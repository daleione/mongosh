//! Context generator — calls DeepSeek Chat API to produce structured context files.

use std::path::{Path, PathBuf};

use crate::config::AiConfig;
use crate::error::{MongoshError, Result};

use super::sampler::{CollectionSample, DatabaseSample};

/// System prompt for generating per-collection schema + queries.
const SYSTEM_PROMPT_SCHEMA: &str = r#"You are a MongoDB schema analyzer. Given sample documents from a collection, produce a concise context file for use in code completion.

Output format (plain text, NO markdown fences):

Collection: {name}
Documents: {count}
Indexes: {index_list}

Fields:
  {field_name}  {BsonType}  — {one-line description}

For nested objects, indent with two extra spaces:
  metadata       Object     — additional info
    metadata.author  String — creator name

Common Queries:
  // {description}
  db.{collection}.{query}

Rules:
- List ALL fields observed across all sample documents
- Infer the BSON type from values (String, Int32, Int64, Double, Boolean, ObjectId, Date, Array, Object)
- For arrays, note element type: [String], [Object], [ObjectId]
- For enum-like fields (few distinct values), list possible values
- Generate 4-6 common queries covering: basic find, filter, sort, aggregation, index-aligned queries
- Queries must ONLY use fields that appear in the sample documents
- Keep descriptions concise (< 60 chars)
- Output plain text only, no markdown formatting"#;

/// System prompt for generating database overview.
const SYSTEM_PROMPT_OVERVIEW: &str = r#"You are a MongoDB database documenter. Given a list of collections with their document counts and index counts, produce a brief overview.

Output format (plain text):

Database: {name}
Collections: {comma-separated names}

- {name} ({count} docs) — {one-line purpose description}

Rules:
- Infer the purpose of each collection from its name
- Keep descriptions to one line
- Output plain text only"#;

/// System prompt for generating cross-collection queries.
const SYSTEM_PROMPT_QUERIES: &str = r#"You are a MongoDB query expert. Given a database's collections and their top-level fields, generate useful cross-collection queries and common patterns.

Output format (plain text):

Database: {name}

# Cross-collection Queries
  // {description}
  db.{collection}.aggregate([...])

# Common Patterns
  // {description}
  db.{collection}.{operation}(...)

Rules:
- Generate 5-10 practical queries
- Include $lookup for related collections (infer relationships from field names like user_id, order_id)
- Include time-range filters for date fields
- Include pagination patterns
- Queries must ONLY use fields from the provided schema
- Output plain text only"#;

/// Context generator — turns MongoDB samples into structured context files via Chat API.
pub struct ContextGenerator {
    config: AiConfig,
    client: reqwest::Client,
    context_dir: PathBuf,
}

impl ContextGenerator {
    /// Create a new context generator.
    pub fn new(config: AiConfig, context_dir: Option<PathBuf>) -> Self {
        let context_dir = context_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".mongosh")
                .join("context")
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        Self {
            config,
            client,
            context_dir,
        }
    }

    /// Generate context for an entire database.
    ///
    /// Returns the directory path where context files were written and
    /// a count of successfully generated collection schemas.
    pub async fn generate(&self, sample: &DatabaseSample) -> Result<(PathBuf, usize)> {
        let dir = self.db_context_dir(&sample.datasource, &sample.database);
        std::fs::create_dir_all(dir.join("schemas")).map_err(|e| {
            MongoshError::Generic(format!("Failed to create context directory: {}", e))
        })?;

        // 1. Overview
        let overview = self.generate_overview(sample).await?;
        std::fs::write(dir.join("collections.md"), &overview)
            .map_err(|e| MongoshError::Generic(format!("Failed to write collections.md: {}", e)))?;

        // 2. Per-collection schemas
        let mut success_count = 0;
        for coll in &sample.collections {
            match self
                .generate_collection_schema(&sample.database, coll)
                .await
            {
                Ok(schema) => {
                    let path = dir.join("schemas").join(format!("{}.md", coll.name));
                    if let Err(e) = std::fs::write(&path, &schema) {
                        tracing::warn!("Failed to write schema for '{}': {}", coll.name, e);
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to generate schema for '{}': {}", coll.name, e);
                }
            }
        }

        // 3. Cross-collection queries
        match self.generate_cross_queries(sample).await {
            Ok(queries) => {
                let _ = std::fs::write(dir.join("queries.md"), &queries);
            }
            Err(e) => {
                tracing::warn!("Failed to generate cross-collection queries: {}", e);
            }
        }

        // 4. Meta file
        self.write_meta(&dir, sample)?;

        // 5. Set file permissions on Unix
        #[cfg(unix)]
        Self::set_dir_permissions(&dir);

        Ok((dir, success_count))
    }

    /// Generate / refresh context for a single collection.
    pub async fn generate_single(
        &self,
        datasource: &str,
        database: &str,
        coll: &CollectionSample,
    ) -> Result<PathBuf> {
        let dir = self.db_context_dir(datasource, database);
        std::fs::create_dir_all(dir.join("schemas")).map_err(|e| {
            MongoshError::Generic(format!("Failed to create context directory: {}", e))
        })?;

        let schema = self.generate_collection_schema(database, coll).await?;
        let path = dir.join("schemas").join(format!("{}.md", coll.name));
        std::fs::write(&path, &schema)
            .map_err(|e| MongoshError::Generic(format!("Failed to write schema file: {}", e)))?;

        Ok(path)
    }

    // ── Chat API calls ──────────────────────────────────────────────────

    async fn generate_overview(&self, sample: &DatabaseSample) -> Result<String> {
        let collection_list: Vec<String> = sample
            .collections
            .iter()
            .map(|c| {
                format!(
                    "- {} ({} docs, {} indexes)",
                    c.name,
                    c.document_count,
                    c.indexes.len()
                )
            })
            .collect();

        let user_msg = format!(
            "Database: {}\n\nCollections:\n{}",
            sample.database,
            collection_list.join("\n"),
        );

        self.chat(SYSTEM_PROMPT_OVERVIEW, &user_msg).await
    }

    async fn generate_collection_schema(
        &self,
        database: &str,
        coll: &CollectionSample,
    ) -> Result<String> {
        let docs_json: Vec<String> = coll
            .sample_documents
            .iter()
            .take(20)
            .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
            .collect();

        let indexes_desc: Vec<String> = coll
            .indexes
            .iter()
            .map(|idx| {
                let unique_tag = if idx.unique { " (unique)" } else { "" };
                format!("  {}: {}{}", idx.name, idx.keys, unique_tag)
            })
            .collect();

        let user_msg = format!(
            "Database: {db}\nCollection: {coll}\nDocument count: {count}\n\n\
             Indexes:\n{indexes}\n\n\
             Sample documents ({n} docs):\n{docs}",
            db = database,
            coll = coll.name,
            count = coll.document_count,
            indexes = if indexes_desc.is_empty() {
                "  (none)".to_string()
            } else {
                indexes_desc.join("\n")
            },
            n = docs_json.len(),
            docs = docs_json.join("\n---\n"),
        );

        self.chat(SYSTEM_PROMPT_SCHEMA, &user_msg).await
    }

    async fn generate_cross_queries(&self, sample: &DatabaseSample) -> Result<String> {
        let summary: Vec<String> = sample
            .collections
            .iter()
            .map(|c| {
                let fields: Vec<String> = c
                    .sample_documents
                    .first()
                    .map(|d| d.keys().map(|k| k.to_string()).collect())
                    .unwrap_or_default();
                format!("- {} (fields: {})", c.name, fields.join(", "))
            })
            .collect();

        let user_msg = format!(
            "Database: {}\n\nCollections:\n{}",
            sample.database,
            summary.join("\n"),
        );

        self.chat(SYSTEM_PROMPT_QUERIES, &user_msg).await
    }

    /// Call DeepSeek Chat API.
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let api_key = self.config.resolve_api_key();
        // Chat endpoint: strip /beta suffix if present
        let base = self
            .config
            .base_url
            .trim_end_matches("/beta")
            .trim_end_matches('/');
        let url = format!("{}/chat/completions", base);

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message},
            ],
            "temperature": 0.3,
            "max_tokens": 2048,
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| MongoshError::Generic(format!("Chat API request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(MongoshError::Generic(format!(
                "Chat API error HTTP {}: {}",
                status, text
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| MongoshError::Generic(format!("Chat API JSON parse error: {}", e)))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    fn db_context_dir(&self, datasource: &str, database: &str) -> PathBuf {
        let dir_name = if datasource.is_empty() {
            database.to_string()
        } else {
            format!("{}_{}", datasource, database)
        };
        self.context_dir.join(dir_name)
    }

    fn write_meta(&self, dir: &Path, sample: &DatabaseSample) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut meta = format!(
            "# Auto-generated by mongosh AI context generator\n\
             generated_at = \"{}\"\n\
             database = \"{}\"\n\
             datasource = \"{}\"\n\
             collection_count = {}\n\
             model = \"{}\"\n\n",
            now,
            sample.database,
            sample.datasource,
            sample.collections.len(),
            self.config.model,
        );

        for coll in &sample.collections {
            meta.push_str(&format!(
                "[collections.\"{}\"]\n\
                 document_count = {}\n\
                 sample_size = {}\n\n",
                coll.name,
                coll.document_count,
                coll.sample_documents.len(),
            ));
        }

        std::fs::write(dir.join("_meta.toml"), meta)
            .map_err(|e| MongoshError::Generic(format!("Failed to write _meta.toml: {}", e)))?;
        Ok(())
    }

    #[cfg(unix)]
    fn set_dir_permissions(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        // Set directory and files to owner-only access
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let _ =
                    std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o600));
            }
        }
        let schemas_dir = dir.join("schemas");
        if schemas_dir.exists() {
            let _ = std::fs::set_permissions(&schemas_dir, std::fs::Permissions::from_mode(0o700));
            if let Ok(entries) = std::fs::read_dir(&schemas_dir) {
                for entry in entries.flatten() {
                    let _ = std::fs::set_permissions(
                        entry.path(),
                        std::fs::Permissions::from_mode(0o600),
                    );
                }
            }
        }
    }
}
