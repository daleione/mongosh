//! AI query generation from natural language descriptions.
//!
//! This module converts natural language into MongoDB shell queries by calling
//! an LLM chat completion API (OpenAI-compatible, e.g. DeepSeek).
//!
//! It fully reuses the pre-generated context from the `ai_context` module
//! (`:ai-gen`), injecting the database overview, per-collection schemas,
//! and cross-collection query examples into the system prompt so the LLM
//! can produce accurate, schema-aware queries.
//!
//! ## Multi-step query execution
//!
//! When the user's intent spans multiple collections (e.g. "what are Xiaoming's scores"),
//! the planner decomposes it into sequential single-collection steps.  Each
//! step is presented to the user for editing, executed, and its result is
//! injected into the next step's prompt so the LLM can reference concrete
//! values (ObjectIds, etc.).

use crate::config::AiConfig;
use crate::executor::{ExecutionResult, ResultData};
use crate::repl::ai_context::ContextReader;

// ═══════════════════════════════════════════════════════════════════════════
//  Data structures
// ═══════════════════════════════════════════════════════════════════════════

/// AI-generated execution plan — one or more sequential steps.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExecutionPlan {
    /// The user's original natural-language intent.
    pub intent: String,
    /// Ordered list of steps to execute.
    pub steps: Vec<PlannedStep>,
}

/// A single planned step targeting one collection.
#[derive(Debug, Clone)]
pub struct PlannedStep {
    /// 1-based step number.
    pub step_number: usize,
    /// Human-readable description of what this step does.
    pub description: String,
    /// Target collection name (inferred by the LLM).
    pub collection: String,
    /// Operation hint: find, findOne, aggregate, count, …
    pub operation_hint: String,
}

/// Result summary for one executed step — carried forward into subsequent
/// steps' prompts so the LLM can reference concrete values.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// 1-based step number.
    pub step_number: usize,
    /// The query string that was actually executed (may differ from the
    /// generated one if the user edited it).
    pub executed_query: String,
    /// A compact textual summary of the execution result (JSON docs,
    /// counts, messages, …).  Truncated to a reasonable size.
    pub result_summary: String,
    /// Number of documents in the result (0 for non-document results).
    pub document_count: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
//  AiQueryGenerator
// ═══════════════════════════════════════════════════════════════════════════

/// AI query generator that converts natural language to MongoDB queries.
#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
pub struct AiQueryGenerator {
    config: AiConfig,
    context_reader: ContextReader,
    datasource: String,
}

impl AiQueryGenerator {
    /// Create a new generator with the given AI configuration.
    pub fn new(config: AiConfig, datasource: String) -> Self {
        let context_reader = ContextReader::new(None);
        Self {
            config,
            context_reader,
            datasource,
        }
    }

    // ─── plan ───────────────────────────────────────────────────────────

    /// Analyse the user's intent and produce an [`ExecutionPlan`].
    ///
    /// * Single-collection tasks → `plan.steps.len() == 1`
    /// * Multi-collection tasks  → 2–5 steps, executed sequentially
    ///
    /// The planner calls the Chat API once with a JSON-output prompt.
    /// If the response cannot be parsed the method falls back to a
    /// single-step plan so the existing single-shot path still works.
    #[cfg(feature = "ai-completion")]
    pub async fn plan(&self, description: &str, database: &str) -> Result<ExecutionPlan, String> {
        let api_key = self.config.resolve_api_key();
        if api_key.is_empty() {
            return Err("AI API key not configured. Set DEEPSEEK_API_KEY env var \
                 or configure ai.api_key in config."
                .to_string());
        }

        let overview = self
            .context_reader
            .read_overview(&self.datasource, database);
        let all_schemas = self
            .context_reader
            .read_all_schemas(&self.datasource, database);

        let system_prompt =
            self.build_plan_system_prompt(database, overview.as_deref(), &all_schemas);

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": description   }
            ],
            "max_tokens": 512,
            "temperature": 0.1,
            "stream": false
        });

        let raw = self.call_chat_api(&api_key, &body).await?;

        // Try to parse the structured JSON plan.
        match parse_plan_response(&raw, description) {
            Ok(plan) => Ok(plan),
            Err(e) => {
                tracing::debug!(
                    "Plan JSON parse failed ({}), falling back to single step",
                    e
                );
                // Fallback: treat the entire intent as a single step.
                Ok(ExecutionPlan {
                    intent: description.to_string(),
                    steps: vec![PlannedStep {
                        step_number: 1,
                        description: description.to_string(),
                        collection: String::new(),
                        operation_hint: String::new(),
                    }],
                })
            }
        }
    }

    #[cfg(not(feature = "ai-completion"))]
    pub async fn plan(&self, _description: &str, _database: &str) -> Result<ExecutionPlan, String> {
        Err("AI query generation requires the 'ai-completion' feature. \
             Rebuild with: cargo build --features ai-completion"
            .to_string())
    }

    // ─── generate_step ──────────────────────────────────────────────────

    /// Generate the concrete MongoDB shell query for one step of a plan.
    ///
    /// `previous_results` carries the summaries of all already-executed
    /// steps so the LLM can reference real values (ObjectIds, field
    /// values, …).
    #[cfg(feature = "ai-completion")]
    pub async fn generate_step(
        &self,
        plan: &ExecutionPlan,
        step: &PlannedStep,
        previous_results: &[StepResult],
        database: &str,
    ) -> Result<String, String> {
        let api_key = self.config.resolve_api_key();
        if api_key.is_empty() {
            return Err("AI API key not configured.".to_string());
        }

        let system_prompt = self.build_step_system_prompt(plan, step, previous_results, database);

        let user_msg = format!(
            "Original intent: {}\n\nNow generate the query for step {}: {}",
            plan.intent, step.step_number, step.description,
        );

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": user_msg      }
            ],
            "max_tokens": 1024,
            "temperature": 0.1,
            "stream": false
        });

        let content = self.call_chat_api(&api_key, &body).await?;
        let query = extract_query_from_response(&content);

        if query.is_empty() {
            return Err("AI returned an empty response".to_string());
        }

        Ok(query)
    }

    #[cfg(not(feature = "ai-completion"))]
    pub async fn generate_step(
        &self,
        _plan: &ExecutionPlan,
        _step: &PlannedStep,
        _previous_results: &[StepResult],
        _database: &str,
    ) -> Result<String, String> {
        Err("AI query generation requires the 'ai-completion' feature. \
             Rebuild with: cargo build --features ai-completion"
            .to_string())
    }

    // ─── generate (convenience, kept for backward compat) ───────────────

    /// One-shot convenience wrapper: `plan()` then `generate_step()` for
    /// the first (and presumably only) step.
    ///
    /// Kept for simple call-sites that only need a single query.
    #[allow(dead_code)]
    #[cfg(feature = "ai-completion")]
    pub async fn generate(&self, description: &str, database: &str) -> Result<String, String> {
        let plan = self.plan(description, database).await?;
        let step = plan
            .steps
            .first()
            .ok_or_else(|| "Plan contained no steps".to_string())?;
        self.generate_step(&plan, step, &[], database).await
    }

    #[allow(dead_code)]
    #[cfg(not(feature = "ai-completion"))]
    pub async fn generate(&self, _description: &str, _database: &str) -> Result<String, String> {
        Err("AI query generation requires the 'ai-completion' feature. \
             Rebuild with: cargo build --features ai-completion"
            .to_string())
    }

    // ─── HTTP helper ────────────────────────────────────────────────────

    /// Send a chat completion request and return the assistant message text.
    #[cfg(feature = "ai-completion")]
    async fn call_chat_api(
        &self,
        api_key: &str,
        body: &serde_json::Value,
    ) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let chat_url = derive_chat_url(&self.config.base_url);

        let resp = client
            .post(&chat_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("AI API request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("AI API error (HTTP {}): {}", status, text));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse AI response: {}", e))?;

        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    // ─── Prompt builders ────────────────────────────────────────────────

    /// System prompt for the **planning** phase.
    #[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
    fn build_plan_system_prompt(
        &self,
        database: &str,
        overview: Option<&str>,
        all_schemas: &[(String, String)],
    ) -> String {
        let mut p = String::with_capacity(8192);

        p.push_str(
            "You are a MongoDB query planner. Given a natural language task, decide \
             whether it can be fulfilled with a SINGLE collection query or requires \
             MULTIPLE sequential steps across different collections.\n\n",
        );

        p.push_str("Output valid JSON only (no markdown fences, no explanation).\n\n");

        p.push_str("If the task needs only ONE collection:\n");
        p.push_str(
            r#"{"steps":[{"step":1,"description":"...","collection":"...","operation":"find"}]}"#,
        );
        p.push_str("\n\n");

        p.push_str("If the task needs MULTIPLE steps:\n");
        p.push_str(r#"{"steps":[{"step":1,"description":"...","collection":"...","operation":"findOne"},{"step":2,"description":"...","collection":"...","operation":"find"}]}"#);
        p.push_str("\n\n");

        p.push_str("Rules:\n");
        p.push_str("1. Each step targets exactly ONE collection.\n");
        p.push_str("2. Later steps may reference results from earlier steps.\n");
        p.push_str("3. Maximum 5 steps. Simplify if more are needed.\n");
        p.push_str("4. Use collection names and field names from the schema below.\n");
        p.push_str("5. operation is one of: find, findOne, aggregate, countDocuments, distinct, updateOne, updateMany, deleteOne, deleteMany.\n");
        p.push_str("6. Output ONLY the JSON object.\n\n");

        p.push_str(&format!("Current database: {}\n\n", database));

        if let Some(ov) = overview {
            p.push_str("=== Database Overview ===\n");
            p.push_str(ov);
            p.push_str("\n\n");
        }

        if !all_schemas.is_empty() {
            p.push_str("=== Collection Schemas ===\n\n");
            for (name, schema) in all_schemas {
                p.push_str(&format!("--- {} ---\n", name));
                p.push_str(&truncate_lines(schema, 60));
                p.push_str("\n\n");
            }
        }

        if overview.is_none() && all_schemas.is_empty() {
            p.push_str(
                "Note: No schema context available. Infer collection names from the task.\n\n",
            );
        }

        p
    }

    /// System prompt for the **step generation** phase.
    #[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
    fn build_step_system_prompt(
        &self,
        plan: &ExecutionPlan,
        step: &PlannedStep,
        previous_results: &[StepResult],
        database: &str,
    ) -> String {
        let mut p = String::with_capacity(8192);

        p.push_str(
            "You are a MongoDB query generator. Generate the query for ONE step \
             of a multi-step plan.\n\n",
        );

        p.push_str("Rules:\n");
        p.push_str("1. Output ONLY the MongoDB shell command (db.collection.operation(…)).\n");
        p.push_str(
            "2. Use ACTUAL values from previous step results (real ObjectIds, not placeholders).\n",
        );
        p.push_str("3. No $lookup, $graphLookup, or $unionWith.\n");
        p.push_str("4. Do NOT wrap in markdown code blocks.\n");
        p.push_str("5. ONLY use field names from the schema.\n");
        p.push_str("6. Use correct BSON types (ObjectId for _id, Date for timestamps, etc.).\n\n");

        p.push_str(&format!("Current database: {}\n", database));
        p.push_str(&format!("Plan: {} total step(s)\n\n", plan.steps.len()));

        // Inject only the target collection's schema (not all schemas).
        if !step.collection.is_empty() {
            if let Some(schema) = self.context_reader.read_collection_schema(
                &self.datasource,
                database,
                &step.collection,
            ) {
                p.push_str(&format!(
                    "=== Target Collection Schema: {} ===\n",
                    step.collection
                ));
                p.push_str(&truncate_lines(&schema, 80));
                p.push_str("\n\n");
            }
        } else {
            // Collection unknown — inject all schemas (fallback).
            let all = self
                .context_reader
                .read_all_schemas(&self.datasource, database);
            if !all.is_empty() {
                p.push_str("=== Collection Schemas ===\n\n");
                for (name, schema) in &all {
                    p.push_str(&format!("--- {} ---\n", name));
                    p.push_str(&truncate_lines(schema, 60));
                    p.push_str("\n\n");
                }
            }
        }

        // Inject previous step results.
        if !previous_results.is_empty() {
            p.push_str("=== Previous Steps ===\n\n");
            let mut budget = MAX_TOTAL_RESULT_CHARS;
            for sr in previous_results {
                let block = format_step_result_block(sr);
                if block.len() > budget {
                    p.push_str("(earlier results truncated for brevity)\n\n");
                    break;
                }
                budget -= block.len();
                p.push_str(&block);
            }
        }

        // Current step description.
        p.push_str(&format!(
            "=== Current Step {}/{} ===\n{}\nTarget collection: {}\nOperation: {}\n\n",
            step.step_number,
            plan.steps.len(),
            step.description,
            if step.collection.is_empty() {
                "(infer from context)"
            } else {
                &step.collection
            },
            if step.operation_hint.is_empty() {
                "auto"
            } else {
                &step.operation_hint
            },
        ));

        p
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  summarize_result — ExecutionResult → StepResult
// ═══════════════════════════════════════════════════════════════════════════

/// Maximum characters for a single step result summary.
const MAX_STEP_RESULT_CHARS: usize = 2000;
/// Maximum combined characters for all previous results in a prompt.
const MAX_TOTAL_RESULT_CHARS: usize = 6000;

/// Convert an [`ExecutionResult`] into a compact [`StepResult`] suitable
/// for injection into the next step's prompt.
pub fn summarize_result(
    step_number: usize,
    executed_query: &str,
    result: &ExecutionResult,
) -> StepResult {
    let (summary, doc_count) = match &result.data {
        ResultData::Documents(docs) => summarize_docs(docs),
        ResultData::DocumentsWithPagination { documents, .. } => summarize_docs(documents),
        ResultData::Document(doc) => {
            let json = serde_json::to_string_pretty(doc).unwrap_or_default();
            (truncate_chars(&json, MAX_STEP_RESULT_CHARS), 1)
        }
        ResultData::InsertOne { inserted_id } => (format!("Inserted: {}", inserted_id), 1),
        ResultData::InsertMany { inserted_ids } => (
            format!("Inserted {} documents", inserted_ids.len()),
            inserted_ids.len(),
        ),
        ResultData::Update { matched, modified } => {
            (format!("matched: {}, modified: {}", matched, modified), 0)
        }
        ResultData::Delete { deleted } => (format!("deleted: {}", deleted), 0),
        ResultData::Count(n) => (format!("count: {}", n), 0),
        ResultData::Message(msg) => (truncate_chars(msg, MAX_STEP_RESULT_CHARS), 0),
        ResultData::List(items) => {
            let text = items.join("\n");
            (truncate_chars(&text, MAX_STEP_RESULT_CHARS), items.len())
        }
        ResultData::None => ("(no data)".to_string(), 0),
        ResultData::Stream(_) => ("(streaming result)".to_string(), 0),
    };

    StepResult {
        step_number,
        executed_query: executed_query.to_string(),
        result_summary: summary,
        document_count: doc_count,
    }
}

/// Summarize a vec of BSON documents into compact text.
fn summarize_docs(docs: &[mongodb::bson::Document]) -> (String, usize) {
    let count = docs.len();
    if count == 0 {
        return ("(no documents)".to_string(), 0);
    }

    let show = if count <= 5 { count } else { 3 };
    let mut parts: Vec<String> = Vec::with_capacity(show + 1);
    for doc in docs.iter().take(show) {
        let json = serde_json::to_string(doc).unwrap_or_else(|_| format!("{:?}", doc));
        parts.push(json);
    }
    if count > 5 {
        parts.push(format!("... ({} more documents)", count - 3));
    }

    let text = parts.join("\n");
    (truncate_chars(&text, MAX_STEP_RESULT_CHARS), count)
}

// ═══════════════════════════════════════════════════════════════════════════
//  Helper functions
// ═══════════════════════════════════════════════════════════════════════════

/// Derive the chat completion URL from the configured base URL.
#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
fn derive_chat_url(base_url: &str) -> String {
    if base_url.ends_with("/chat/completions") {
        return base_url.to_string();
    }
    let base = base_url
        .trim_end_matches('/')
        .trim_end_matches("/beta")
        .trim_end_matches("/completions");
    format!("{}/chat/completions", base)
}

/// Extract the actual MongoDB query from the AI response (strip markdown
/// code fences if present).
#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
fn extract_query_from_response(response: &str) -> String {
    let trimmed = response.trim();

    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        let code_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let code = &after[code_start..];
        if let Some(end) = code.find("```") {
            return code[..end].trim().to_string();
        }
    }

    trimmed.to_string()
}

/// Truncate a multi-line string to at most `max_lines` lines.
#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
fn truncate_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let mut result: String = lines[..max_lines].join("\n");
    result.push_str(&format!(
        "\n... ({} more lines truncated)",
        lines.len() - max_lines
    ));
    result
}

/// Truncate a string to at most `max_chars` characters.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        let mut s = text[..max_chars].to_string();
        s.push_str("\n... (truncated)");
        s
    }
}

/// Parse the JSON output from the planner LLM into an [`ExecutionPlan`].
#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
fn parse_plan_response(raw: &str, intent: &str) -> Result<ExecutionPlan, String> {
    // The LLM might wrap in code fences despite our instructions.
    let json_str = extract_json_from_response(raw);

    let val: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("JSON parse error: {}", e))?;

    let steps_arr = val
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing 'steps' array".to_string())?;

    if steps_arr.is_empty() {
        return Err("Empty steps array".to_string());
    }

    let mut steps = Vec::with_capacity(steps_arr.len());
    for (i, item) in steps_arr.iter().enumerate() {
        steps.push(PlannedStep {
            step_number: item
                .get("step")
                .and_then(|v| v.as_u64())
                .unwrap_or((i + 1) as u64) as usize,
            description: item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            collection: item
                .get("collection")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            operation_hint: item
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }

    // Enforce maximum 5 steps.
    if steps.len() > 5 {
        steps.truncate(5);
    }

    Ok(ExecutionPlan {
        intent: intent.to_string(),
        steps,
    })
}

/// Extract a JSON object from a response that may be wrapped in markdown
/// code fences or have surrounding prose.
#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
fn extract_json_from_response(raw: &str) -> String {
    let trimmed = raw.trim();

    // Try code-fence extraction first.
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        let code_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let code = &after[code_start..];
        if let Some(end) = code.find("```") {
            return code[..end].trim().to_string();
        }
    }

    // Find the first '{' and last '}'.
    if let (Some(open), Some(close)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if close > open {
            return trimmed[open..=close].to_string();
        }
    }

    trimmed.to_string()
}

/// Format a [`StepResult`] into a text block for prompt injection.
fn format_step_result_block(sr: &StepResult) -> String {
    format!(
        "Step {}: {}\nResult ({} doc(s)):\n{}\n\n",
        sr.step_number, sr.executed_query, sr.document_count, sr.result_summary,
    )
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── derive_chat_url ─────────────────────────────────────────────────

    #[test]
    fn test_derive_chat_url_deepseek_beta() {
        assert_eq!(
            derive_chat_url("https://api.deepseek.com/beta"),
            "https://api.deepseek.com/chat/completions"
        );
    }

    #[test]
    fn test_derive_chat_url_openai_v1() {
        assert_eq!(
            derive_chat_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_derive_chat_url_already_chat() {
        assert_eq!(
            derive_chat_url("https://api.example.com/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    // ── extract_query_from_response ─────────────────────────────────────

    #[test]
    fn test_extract_query_plain() {
        assert_eq!(
            extract_query_from_response("db.users.find({ age: { $gt: 18 } })"),
            "db.users.find({ age: { $gt: 18 } })"
        );
    }

    #[test]
    fn test_extract_query_from_code_block() {
        let r = "```javascript\ndb.users.find({ age: { $gt: 18 } })\n```";
        assert_eq!(
            extract_query_from_response(r),
            "db.users.find({ age: { $gt: 18 } })"
        );
    }

    #[test]
    fn test_extract_query_from_bare_code_block() {
        let r = "```\ndb.users.find({})\n```";
        assert_eq!(extract_query_from_response(r), "db.users.find({})");
    }

    #[test]
    fn test_extract_query_with_surrounding_text() {
        let r = "Here is the query:\n```\ndb.users.find({})\n```\nThis finds all users.";
        assert_eq!(extract_query_from_response(r), "db.users.find({})");
    }

    // ── truncate_lines ──────────────────────────────────────────────────

    #[test]
    fn test_truncate_lines_short() {
        let text = "line1\nline2\nline3";
        assert_eq!(truncate_lines(text, 5), text);
    }

    #[test]
    fn test_truncate_lines_exact() {
        let text = "a\nb\nc";
        assert_eq!(truncate_lines(text, 3), text);
    }

    #[test]
    fn test_truncate_lines_over() {
        let text = "a\nb\nc\nd\ne";
        assert_eq!(
            truncate_lines(text, 2),
            "a\nb\n... (3 more lines truncated)"
        );
    }

    // ── truncate_chars ──────────────────────────────────────────────────

    #[test]
    fn test_truncate_chars_short() {
        assert_eq!(truncate_chars("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_chars_over() {
        let result = truncate_chars("hello world", 5);
        assert!(result.starts_with("hello"));
        assert!(result.contains("truncated"));
    }

    // ── parse_plan_response ─────────────────────────────────────────────

    #[test]
    fn test_parse_plan_single_step() {
        let raw = r#"{"steps":[{"step":1,"description":"query users","collection":"users","operation":"find"}]}"#;
        let plan = parse_plan_response(raw, "query users").unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].collection, "users");
        assert_eq!(plan.steps[0].operation_hint, "find");
    }

    #[test]
    fn test_parse_plan_multi_step() {
        let raw = r#"{
            "steps": [
                {"step":1,"description":"find Xiaoming's user_id","collection":"users","operation":"findOne"},
                {"step":2,"description":"query scores","collection":"scores","operation":"find"}
            ]
        }"#;
        let plan = parse_plan_response(raw, "what are Xiaoming's scores").unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].step_number, 1);
        assert_eq!(plan.steps[0].collection, "users");
        assert_eq!(plan.steps[1].step_number, 2);
        assert_eq!(plan.steps[1].collection, "scores");
        assert_eq!(plan.intent, "what are Xiaoming's scores");
    }

    #[test]
    fn test_parse_plan_with_code_fence() {
        let raw = "```json\n{\"steps\":[{\"step\":1,\"description\":\"test\",\"collection\":\"c\",\"operation\":\"find\"}]}\n```";
        let plan = parse_plan_response(raw, "test").unwrap();
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_parse_plan_truncates_to_5() {
        let steps: Vec<serde_json::Value> = (1..=8)
            .map(|i| {
                serde_json::json!({
                    "step": i,
                    "description": format!("step {}", i),
                    "collection": "c",
                    "operation": "find"
                })
            })
            .collect();
        let raw = serde_json::json!({"steps": steps}).to_string();
        let plan = parse_plan_response(&raw, "test").unwrap();
        assert_eq!(plan.steps.len(), 5);
    }

    #[test]
    fn test_parse_plan_invalid_json() {
        assert!(parse_plan_response("not json", "test").is_err());
    }

    #[test]
    fn test_parse_plan_empty_steps() {
        assert!(parse_plan_response(r#"{"steps":[]}"#, "test").is_err());
    }

    // ── extract_json_from_response ──────────────────────────────────────

    #[test]
    fn test_extract_json_bare() {
        let raw = r#"{"steps":[{"step":1}]}"#;
        assert_eq!(extract_json_from_response(raw), raw);
    }

    #[test]
    fn test_extract_json_with_prose() {
        let raw = "Here is the plan:\n{\"steps\":[{\"step\":1}]}\nDone.";
        assert_eq!(
            extract_json_from_response(raw),
            "{\"steps\":[{\"step\":1}]}"
        );
    }

    #[test]
    fn test_extract_json_with_code_fence() {
        let raw = "```json\n{\"steps\":[{\"step\":1}]}\n```";
        assert_eq!(
            extract_json_from_response(raw),
            "{\"steps\":[{\"step\":1}]}"
        );
    }

    // ── summarize_result ────────────────────────────────────────────────

    #[test]
    fn test_summarize_count() {
        let result = ExecutionResult {
            success: true,
            data: ResultData::Count(42),
            stats: Default::default(),
            error: None,
        };
        let sr = summarize_result(1, "db.users.countDocuments({})", &result);
        assert_eq!(sr.step_number, 1);
        assert!(sr.result_summary.contains("42"));
        assert_eq!(sr.document_count, 0);
    }

    #[test]
    fn test_summarize_message() {
        let result = ExecutionResult {
            success: true,
            data: ResultData::Message("hello".to_string()),
            stats: Default::default(),
            error: None,
        };
        let sr = summarize_result(2, "some query", &result);
        assert_eq!(sr.result_summary, "hello");
    }

    #[test]
    fn test_summarize_empty_docs() {
        let result = ExecutionResult {
            success: true,
            data: ResultData::Documents(vec![]),
            stats: Default::default(),
            error: None,
        };
        let sr = summarize_result(1, "q", &result);
        assert!(sr.result_summary.contains("no documents"));
        assert_eq!(sr.document_count, 0);
    }

    #[test]
    fn test_summarize_many_docs() {
        let docs: Vec<mongodb::bson::Document> =
            (0..10).map(|i| mongodb::bson::doc! { "i": i }).collect();
        let result = ExecutionResult {
            success: true,
            data: ResultData::Documents(docs),
            stats: Default::default(),
            error: None,
        };
        let sr = summarize_result(1, "q", &result);
        assert_eq!(sr.document_count, 10);
        // Should show first 3 + "... (7 more documents)"
        assert!(sr.result_summary.contains("7 more documents"));
    }

    // ── format_step_result_block ────────────────────────────────────────

    #[test]
    fn test_format_step_result_block() {
        let sr = StepResult {
            step_number: 1,
            executed_query: "db.users.findOne({})".to_string(),
            result_summary: "{\"_id\": \"abc\"}".to_string(),
            document_count: 1,
        };
        let block = format_step_result_block(&sr);
        assert!(block.contains("Step 1:"));
        assert!(block.contains("db.users.findOne({})"));
        assert!(block.contains("1 doc(s)"));
        assert!(block.contains("\"_id\": \"abc\""));
    }

    // ── build_plan_system_prompt ────────────────────────────────────────

    #[test]
    fn test_plan_prompt_no_context() {
        let g = AiQueryGenerator::new(AiConfig::default(), String::new());
        let p = g.build_plan_system_prompt("testdb", None, &[]);
        assert!(p.contains("query planner"));
        assert!(p.contains("Current database: testdb"));
        assert!(p.contains("No schema context"));
    }

    #[test]
    fn test_plan_prompt_with_schemas() {
        let g = AiQueryGenerator::new(AiConfig::default(), String::new());
        let schemas = vec![("users".to_string(), "fields: _id, name".to_string())];
        let p = g.build_plan_system_prompt("mydb", Some("overview"), &schemas);
        assert!(p.contains("=== Database Overview ==="));
        assert!(p.contains("--- users ---"));
        assert!(!p.contains("No schema context"));
    }

    // ── build_step_system_prompt ────────────────────────────────────────

    #[test]
    fn test_step_prompt_with_previous_results() {
        let g = AiQueryGenerator::new(AiConfig::default(), String::new());
        let plan = ExecutionPlan {
            intent: "test".to_string(),
            steps: vec![
                PlannedStep {
                    step_number: 1,
                    description: "find user".to_string(),
                    collection: "users".to_string(),
                    operation_hint: "findOne".to_string(),
                },
                PlannedStep {
                    step_number: 2,
                    description: "find scores".to_string(),
                    collection: "scores".to_string(),
                    operation_hint: "find".to_string(),
                },
            ],
        };
        let prev = vec![StepResult {
            step_number: 1,
            executed_query: "db.users.findOne({name:\"x\"})".to_string(),
            result_summary: "{\"_id\":\"abc\"}".to_string(),
            document_count: 1,
        }];

        let p = g.build_step_system_prompt(&plan, &plan.steps[1], &prev, "testdb");
        assert!(p.contains("=== Previous Steps ==="));
        assert!(p.contains("Step 1:"));
        assert!(p.contains("\"_id\":\"abc\""));
        assert!(p.contains("Current Step 2/2"));
        assert!(p.contains("find scores"));
    }
}
