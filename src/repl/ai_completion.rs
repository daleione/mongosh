//! AI completion service using DeepSeek FIM API
//!
//! This module provides an asynchronous AI completion service that runs
//! in the background and provides inline hints via cache lookup.
//!
//! # Architecture
//!
//! ```text
//! MongoHinter::handle()
//!   ├─► cache.get(line, pos)  [sync, <1μs]
//!   │     ├─ hit  → return AI hint
//!   │     └─ miss → send request via channel
//!   │
//!   └─► background task  [only with feature "ai-completion"]
//!         ├─ debounce (wait for typing pause)
//!         ├─ build FIM prompt (with history + context)
//!         ├─ POST /completions
//!         └─ write result to cache
//! ```

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use crate::config::AiConfig;
use crate::repl::SharedState;
use crate::repl::ai_context::ContextReader;

// ─── Cache ──────────────────────────────────────────────────────────────────

/// A single cache entry.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The original text-before-cursor that produced this completion.
    key_line: String,
    /// The completion text returned by the API (may be empty for negative cache).
    completion: String,
    /// When this entry was written.
    created_at: Instant,
}

/// AI completion cache with TTL, LRU eviction, and prefix-growth matching.
///
/// Uses `IndexMap` to maintain insertion order for deterministic iteration
/// and efficient LRU eviction (remove from front).
pub struct AiCompletionCache {
    /// Ordered map: cache_key → entry. Newest entries are at the back.
    entries: IndexMap<String, CacheEntry>,
    /// Maximum number of entries before LRU eviction.
    #[cfg(any(feature = "ai-completion", test))]
    max_entries: usize,
    /// Time-to-live for each entry.
    ttl: Duration,
}

impl AiCompletionCache {
    /// Create a new cache.
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: IndexMap::with_capacity(max_entries),
            #[cfg(any(feature = "ai-completion", test))]
            max_entries,
            ttl,
        }
    }

    /// Look up a completion for `line_before`.
    ///
    /// Tries, in order:
    /// 1. Exact match on cache key.
    /// 2. Prefix-growth match: a cached key K is a prefix of `line_before`,
    ///    and the cached completion starts with the characters typed since K.
    ///    Returns the remaining (un-typed) portion.
    ///
    /// In case of multiple prefix-growth matches the **longest key** wins
    /// (most specific context).
    pub fn get(&self, cache_key: &str, line_before: &str) -> Option<String> {
        // 1. Exact match
        if let Some(entry) = self.entries.get(cache_key) {
            if entry.created_at.elapsed() < self.ttl {
                return Some(entry.completion.clone());
            }
        }

        // 2. Prefix-growth match — find longest matching cached key
        let mut best: Option<(usize, String)> = None; // (key_len, remaining)

        for (_, entry) in &self.entries {
            if entry.created_at.elapsed() >= self.ttl {
                continue;
            }
            let k = &entry.key_line;
            if line_before.len() > k.len() && line_before.starts_with(k.as_str()) {
                let typed_since = &line_before[k.len()..];
                if entry.completion.starts_with(typed_since) {
                    let remaining = &entry.completion[typed_since.len()..];
                    if !remaining.is_empty() {
                        let key_len = k.len();
                        if best.as_ref().map_or(true, |(bl, _)| key_len > *bl) {
                            best = Some((key_len, remaining.to_string()));
                        }
                    }
                }
            }
        }

        best.map(|(_, r)| r)
    }

    /// Insert a completion result (may be empty for negative caching).
    #[cfg(any(feature = "ai-completion", test))]
    pub fn set(&mut self, cache_key: String, line_before: String, completion: String) {
        // LRU eviction: remove oldest entries (front of IndexMap) until under limit
        while self.entries.len() >= self.max_entries {
            self.entries.shift_remove_index(0);
        }

        self.entries.insert(
            cache_key,
            CacheEntry {
                key_line: line_before,
                completion,
                created_at: Instant::now(),
            },
        );
    }

    /// Remove all entries (e.g. on database switch).
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }
}

// ─── AI Completion Service ──────────────────────────────────────────────────

/// The public AI completion service.
///
/// Holds a shared cache readable from the hinter (sync) and a channel sender
/// for dispatching requests to the background task.
pub struct AiCompletionService {
    /// Shared cache (hinter reads, background task writes).
    cache: Arc<RwLock<AiCompletionCache>>,
    /// Channel sender to the background debounce/fetch task.
    /// Only present when the `ai-completion` feature is compiled in.
    #[cfg(feature = "ai-completion")]
    request_tx: tokio::sync::mpsc::UnboundedSender<background::AiCompletionRequest>,
    /// Resolved configuration.
    config: AiConfig,
    /// Shared state for building context.
    shared_state: SharedState,
    /// The database name that was active when the cache was last populated.
    /// Used to invalidate the cache on `use <db>`.
    last_database: Arc<RwLock<String>>,
    /// Context reader for pre-generated schema files.
    #[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
    context_reader: ContextReader,
    /// Current datasource name (for context file lookup).
    #[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
    datasource: Arc<RwLock<String>>,
}

impl AiCompletionService {
    /// Create a new service **and** spawn the background task (if the
    /// `ai-completion` cargo feature is enabled).
    ///
    /// Without the feature the service is a no-op: `request_completion`
    /// silently drops requests and `get_cached` always returns `None`.
    pub fn new(config: AiConfig, shared_state: SharedState, datasource: String) -> Self {
        let cache = Arc::new(RwLock::new(AiCompletionCache::new(
            config.max_cache_entries,
            Duration::from_secs(config.cache_ttl_secs),
        )));

        let current_db = shared_state.get_database();
        let last_database = Arc::new(RwLock::new(current_db));
        let context_reader = ContextReader::new(None);
        let datasource = Arc::new(RwLock::new(datasource));

        #[cfg(feature = "ai-completion")]
        let request_tx = {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            background::spawn(rx, Arc::clone(&cache), config.clone());
            tx
        };

        Self {
            cache,
            #[cfg(feature = "ai-completion")]
            request_tx,
            config,
            shared_state,
            last_database,
            context_reader,
            datasource,
        }
    }

    // ── Public API called from MongoHinter ──────────────────────────────

    /// Minimum input length before we bother sending a request.
    pub fn min_trigger_length(&self) -> usize {
        self.config.min_trigger_length
    }

    /// Read pre-generated context for the current database and a target collection.
    ///
    /// Returns `(overview, target_schema)` — either or both may be `None` if
    /// no context files exist.
    #[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
    pub fn get_context(&self, collection_name: Option<&str>) -> (Option<String>, Option<String>) {
        let db = self.shared_state.get_database();
        let ds = self.datasource.read().unwrap().clone();

        let overview = self.context_reader.read_overview(&ds, &db);
        let target_schema = collection_name
            .and_then(|coll| self.context_reader.read_collection_schema(&ds, &db, coll));

        (overview, target_schema)
    }

    /// Look up a cached completion for the current input.
    ///
    /// Returns `Some("")` for a negative-cache hit (API returned nothing).
    /// Returns `None` for a true cache miss.
    pub fn get_cached(&self, line_before: &str) -> Option<String> {
        self.maybe_invalidate_on_db_change();

        let db = self.shared_state.get_database();
        let cache_key = format!("{}@{}", db, line_before);
        let cache = self.cache.read().unwrap();
        cache.get(&cache_key, line_before)
    }

    /// Send an async completion request to the background task.
    ///
    /// This is non-blocking: it enqueues and returns immediately.
    /// Without the `ai-completion` feature this is a no-op.
    #[cfg(feature = "ai-completion")]
    pub fn request_completion(&self, line_before: &str, suffix: &str, history: Vec<String>) {
        let db = self.shared_state.get_database();
        let cache_key = format!("{}@{}", db, line_before);

        // Don't enqueue if already cached.
        {
            let cache = self.cache.read().unwrap();
            if cache.get(&cache_key, line_before).is_some() {
                return;
            }
        }

        // Read pre-generated context
        let collection_name = extract_collection_name(line_before);
        let (overview, target_schema) = self.get_context(collection_name.as_deref());

        let req = background::AiCompletionRequest {
            line_before: line_before.to_string(),
            suffix: suffix.to_string(),
            cache_key,
            history,
            database: db,
            overview,
            target_schema,
        };

        // Non-blocking send — if channel is closed, silently drop.
        let _ = self.request_tx.send(req);
    }

    /// No-op stub when the `ai-completion` feature is disabled.
    #[cfg(not(feature = "ai-completion"))]
    pub fn request_completion(&self, _line_before: &str, _suffix: &str, _history: Vec<String>) {}

    /// If the active database has changed since our last cache fill,
    /// invalidate the entire cache.
    fn maybe_invalidate_on_db_change(&self) {
        let current = self.shared_state.get_database();
        let mut last = self.last_database.write().unwrap();
        if *last != current {
            *last = current;
            let mut cache = self.cache.write().unwrap();
            cache.invalidate_all();
        }
    }
}

#[cfg_attr(not(feature = "ai-completion"), allow(dead_code))]
/// Extract the target collection name from user input.
///
/// Looks for the pattern `db.<collection>.` or `db.<collection>(` and
/// returns the collection name. Returns the last match if multiple found.
pub fn extract_collection_name(line_before: &str) -> Option<String> {
    let mut best = None;
    let mut search_from = 0;
    while search_from + 3 < line_before.len() {
        if let Some(pos) = line_before[search_from..].find("db.") {
            let abs = search_from + pos;
            let after = &line_before[abs + 3..];
            let end = after
                .find(|c: char| c == '.' || c == '(')
                .unwrap_or(after.len());
            let name = &after[..end];
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                best = Some(name.to_string());
            }
            search_from = abs + 3;
        } else {
            break;
        }
    }
    best
}

// ─── Feature-gated background task and helpers ──────────────────────────────
//
// Everything below is only compiled when `ai-completion` is enabled.
// This avoids `#[allow(dead_code)]` entirely — the items simply don't
// exist in a default build.

#[cfg(feature = "ai-completion")]
mod background {
    use super::*;

    // ── Request type ────────────────────────────────────────────────────

    /// AI completion request sent from hinter to background task.
    #[derive(Debug, Clone)]
    pub(crate) struct AiCompletionRequest {
        pub line_before: String,
        pub suffix: String,
        pub cache_key: String,
        pub history: Vec<String>,
        pub database: String,
        pub overview: Option<String>,
        pub target_schema: Option<String>,
    }

    // ── HTTP types ──────────────────────────────────────────────────────

    mod http {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize)]
        pub struct FimRequest {
            pub model: String,
            pub prompt: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub suffix: Option<String>,
            pub max_tokens: u32,
            pub temperature: f32,
            pub stop: Vec<String>,
            pub stream: bool,
        }

        #[derive(Deserialize)]
        pub struct FimResponse {
            pub choices: Vec<FimChoice>,
        }

        #[derive(Deserialize)]
        pub struct FimChoice {
            pub text: String,
        }
    }

    // ── Service health / back-off ───────────────────────────────────────

    pub(super) struct ServiceHealth {
        pub(super) consecutive_failures: u32,
        next_allowed_at: Instant,
        pub(super) degraded: bool,
    }

    impl ServiceHealth {
        pub fn new() -> Self {
            Self {
                consecutive_failures: 0,
                next_allowed_at: Instant::now(),
                degraded: false,
            }
        }

        pub fn record_failure(&mut self) {
            self.consecutive_failures += 1;
            let backoff_secs = 2u64.pow(self.consecutive_failures.min(6)); // max 64s
            self.next_allowed_at = Instant::now() + Duration::from_secs(backoff_secs);
            if self.consecutive_failures >= 5 {
                self.degraded = true;
            }
        }

        pub fn record_success(&mut self) {
            self.consecutive_failures = 0;
            self.degraded = false;
        }

        pub fn is_allowed(&self) -> bool {
            !self.degraded && Instant::now() >= self.next_allowed_at
        }
    }

    // ── Prompt builder ──────────────────────────────────────────────────

    /// Build the FIM prompt by prepending session context and history.
    pub(super) fn build_fim_prompt(
        database: &str,
        overview: Option<&str>,
        target_schema: Option<&str>,
        history: &[String],
        line_before: &str,
    ) -> String {
        let mut prompt = String::with_capacity(2048);

        // 1. Session metadata
        prompt.push_str("// MongoDB Shell session\n");
        prompt.push_str(&format!("// Database: {}\n", database));

        // 2. Collections overview (from pre-generated context)
        if let Some(overview) = overview {
            for line in overview.lines().take(20) {
                prompt.push_str("// ");
                prompt.push_str(line);
                prompt.push('\n');
            }
        }
        prompt.push('\n');

        // 3. Target collection schema (from pre-generated context — most critical)
        if let Some(schema) = target_schema {
            prompt.push_str("// === Target Collection Schema ===\n");
            for line in schema.lines() {
                prompt.push_str("// ");
                prompt.push_str(line);
                prompt.push('\n');
            }
            prompt.push('\n');
        }

        // 4. Recent history
        if !history.is_empty() {
            prompt.push_str("// Recent commands:\n");
            for cmd in history {
                prompt.push_str("// ");
                prompt.push_str(cmd);
                prompt.push('\n');
            }
            prompt.push('\n');
        }

        // 5. Current input (text before cursor)
        prompt.push_str(line_before);
        prompt
    }

    /// Build context-aware stop tokens.
    ///
    /// Always includes `"\n"`. If `suffix` starts with a non-whitespace
    /// character, that character is added as an extra stop token to prevent
    /// the model from generating text that overlaps with the existing suffix.
    pub(super) fn build_stop_tokens(suffix: &str) -> Vec<String> {
        let mut stops = vec!["\n".to_string()];
        if let Some(ch) = suffix.trim_start().chars().next() {
            let s = ch.to_string();
            if !stops.contains(&s) {
                stops.push(s);
            }
        }
        stops
    }

    /// Trim trailing overlap between `completion` and `suffix`.
    ///
    /// If the tail of the completion duplicates the head of the suffix
    /// (e.g. completion=`{$gt: 18}})` and suffix=`})`), the duplicate
    /// portion is removed.
    pub(super) fn trim_suffix_overlap(completion: &str, suffix: &str) -> String {
        if suffix.is_empty() || completion.is_empty() {
            return completion.to_string();
        }
        let max_overlap = completion.len().min(suffix.len());
        for overlap_len in (1..=max_overlap).rev() {
            if completion.ends_with(&suffix[..overlap_len]) {
                return completion[..completion.len() - overlap_len].to_string();
            }
        }
        completion.to_string()
    }

    // ── Background task ─────────────────────────────────────────────────

    pub(super) fn spawn(
        mut rx: tokio::sync::mpsc::UnboundedReceiver<background::AiCompletionRequest>,
        cache: Arc<RwLock<AiCompletionCache>>,
        config: AiConfig,
    ) {
        use tokio::time::timeout;

        let debounce = Duration::from_millis(config.debounce_ms);
        let api_key = config.resolve_api_key();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        tokio::spawn(async move {
            let mut health = ServiceHealth::new();

            loop {
                // Wait for the first request.
                let Some(mut latest) = rx.recv().await else {
                    break; // channel closed
                };

                // Debounce: keep replacing with newer requests until a pause.
                loop {
                    match timeout(debounce, rx.recv()).await {
                        Ok(Some(newer)) => latest = newer,
                        Ok(None) => return, // channel closed
                        Err(_) => break,    // timeout → pause detected, fire request
                    }
                }

                // Skip if already cached.
                {
                    let c = cache.read().unwrap();
                    if c.get(&latest.cache_key, &latest.line_before).is_some() {
                        continue;
                    }
                }

                // Skip if health is bad.
                if !health.is_allowed() {
                    continue;
                }

                // Build FIM prompt.
                let prompt = build_fim_prompt(
                    &latest.database,
                    latest.overview.as_deref(),
                    latest.target_schema.as_deref(),
                    &latest.history,
                    &latest.line_before,
                );
                let stop_tokens = build_stop_tokens(&latest.suffix);

                let suffix_opt = if latest.suffix.is_empty() {
                    None
                } else {
                    Some(latest.suffix.clone())
                };

                let body = http::FimRequest {
                    model: config.model.clone(),
                    prompt,
                    suffix: suffix_opt,
                    max_tokens: config.max_tokens,
                    temperature: config.temperature,
                    stop: stop_tokens,
                    stream: false,
                };

                let url = format!("{}/completions", config.base_url);
                let resp = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        match r.json::<http::FimResponse>().await {
                            Ok(fim) => {
                                health.record_success();
                                let raw = fim
                                    .choices
                                    .first()
                                    .map(|c| c.text.clone())
                                    .unwrap_or_default();

                                // Post-process: trim whitespace, limit length,
                                // remove suffix overlap.
                                let trimmed = raw.trim_end().to_string();
                                let completion = if trimmed.len() > 200 {
                                    String::new() // discard unreasonably long completions
                                } else {
                                    trim_suffix_overlap(&trimmed, &latest.suffix)
                                };

                                // Write to cache (empty string = negative cache).
                                let mut c = cache.write().unwrap();
                                c.set(latest.cache_key, latest.line_before.clone(), completion);
                            }
                            Err(e) => {
                                tracing::debug!("AI completion JSON parse error: {}", e);
                                health.record_failure();
                            }
                        }
                    }
                    Ok(r) => {
                        let status = r.status().as_u16();
                        tracing::debug!("AI completion API error: HTTP {}", status);
                        health.record_failure();
                    }
                    Err(e) => {
                        tracing::debug!("AI completion request failed: {}", e);
                        health.record_failure();
                    }
                }
            }
        });
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Cache tests ─────────────────────────────────────────────────────

    #[test]
    fn test_cache_exact_match() {
        let mut cache = AiCompletionCache::new(256, Duration::from_secs(60));
        cache.set(
            "test@db.users.find({".to_string(),
            "db.users.find({".to_string(),
            "age: {$gt: 18}}".to_string(),
        );

        let result = cache.get("test@db.users.find({", "db.users.find({");
        assert_eq!(result, Some("age: {$gt: 18}}".to_string()));
    }

    #[test]
    fn test_cache_prefix_growth() {
        let mut cache = AiCompletionCache::new(256, Duration::from_secs(60));
        cache.set(
            "test@db.users.find({a".to_string(),
            "db.users.find({a".to_string(),
            "ge: {$gt: 18}}".to_string(),
        );

        // User typed one more char 'g' — completion starts with 'g' ✓
        let result = cache.get("test@db.users.find({ag", "db.users.find({ag");
        assert_eq!(result, Some("e: {$gt: 18}}".to_string()));

        // Two more chars 'ge'
        let result = cache.get("test@db.users.find({age", "db.users.find({age");
        assert_eq!(result, Some(": {$gt: 18}}".to_string()));
    }

    #[test]
    fn test_cache_prefix_growth_mismatch() {
        let mut cache = AiCompletionCache::new(256, Duration::from_secs(60));
        cache.set(
            "test@db.users.find({a".to_string(),
            "db.users.find({a".to_string(),
            "ge: {$gt: 18}}".to_string(),
        );

        // User typed 'n' — 'n' != 'g' → no match
        let result = cache.get("test@db.users.find({an", "db.users.find({an");
        assert_eq!(result, None);
    }

    #[test]
    fn test_cache_prefix_growth_longest_key_wins() {
        let mut cache = AiCompletionCache::new(256, Duration::from_secs(60));
        cache.set(
            "test@db.u".to_string(),
            "db.u".to_string(),
            "sers.find({name: 'alice'})".to_string(),
        );
        cache.set(
            "test@db.users".to_string(),
            "db.users".to_string(),
            ".findOne({_id: 1})".to_string(),
        );

        // "db.users." — both keys are prefixes, but "db.users" is longer → wins
        let result = cache.get("test@db.users.", "db.users.");
        assert_eq!(result, Some("findOne({_id: 1})".to_string()));
    }

    #[test]
    fn test_cache_ttl_expiry() {
        let mut cache = AiCompletionCache::new(256, Duration::from_millis(1));
        cache.set(
            "k".to_string(),
            "line".to_string(),
            "completion".to_string(),
        );

        std::thread::sleep(Duration::from_millis(10));
        let result = cache.get("k", "line");
        assert_eq!(result, None);
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = AiCompletionCache::new(2, Duration::from_secs(60));
        cache.set("k1".to_string(), "l1".to_string(), "v1".to_string());
        cache.set("k2".to_string(), "l2".to_string(), "v2".to_string());
        cache.set("k3".to_string(), "l3".to_string(), "v3".to_string()); // evicts k1

        assert_eq!(cache.get("k1", "l1"), None);
        assert_eq!(cache.get("k2", "l2"), Some("v2".to_string()));
        assert_eq!(cache.get("k3", "l3"), Some("v3".to_string()));
    }

    #[test]
    fn test_cache_negative_hit() {
        let mut cache = AiCompletionCache::new(256, Duration::from_secs(60));
        cache.set(
            "test@help".to_string(),
            "help".to_string(),
            String::new(), // empty = negative cache
        );

        let result = cache.get("test@help", "help");
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn test_cache_invalidate_all() {
        let mut cache = AiCompletionCache::new(256, Duration::from_secs(60));
        cache.set("k1".to_string(), "l1".to_string(), "v1".to_string());
        cache.set("k2".to_string(), "l2".to_string(), "v2".to_string());

        cache.invalidate_all();

        assert_eq!(cache.get("k1", "l1"), None);
        assert_eq!(cache.get("k2", "l2"), None);
    }

    // ── Feature-gated helper tests ──────────────────────────────────────
    //
    // build_fim_prompt, build_stop_tokens, trim_suffix_overlap, and
    // ServiceHealth live inside the `background` module which only exists
    // when the `ai-completion` feature is enabled.

    #[cfg(feature = "ai-completion")]
    mod background_tests {
        use super::super::background::*;

        // ── Prompt builder ──────────────────────────────────────────

        #[test]
        fn test_build_fim_prompt_minimal() {
            let prompt = build_fim_prompt("test", None, None, &[], "db.users.find(");
            assert!(prompt.contains("// Database: test"));
            assert!(prompt.ends_with("db.users.find("));
            assert!(!prompt.contains("Recent commands"));
        }

        #[test]
        fn test_build_fim_prompt_with_history() {
            let history = vec![
                "db.users.find({name: \"alice\"})".to_string(),
                "db.orders.countDocuments()".to_string(),
            ];
            let prompt = build_fim_prompt(
                "myapp",
                Some("Collections: users, orders"),
                None,
                &history,
                "db.",
            );

            assert!(prompt.contains("// Database: myapp"));
            assert!(prompt.contains("// Collections: users, orders"));
            assert!(prompt.contains("// Recent commands:"));
            assert!(prompt.contains("db.users.find"));
            assert!(prompt.ends_with("db."));
        }

        #[test]
        fn test_build_fim_prompt_with_schema() {
            let schema = "Collection: users\nFields:\n  _id ObjectId\n  name String\n  age Int32";
            let result = build_fim_prompt("testdb", None, Some(schema), &[], "db.users.find({");
            assert!(result.contains("Target Collection Schema"));
            assert!(result.contains("name String"));
            assert!(result.contains("db.users.find({"));
        }

        #[test]
        fn test_extract_collection_name() {
            use super::super::extract_collection_name;
            assert_eq!(
                extract_collection_name("db.users.find({"),
                Some("users".to_string())
            );
            assert_eq!(
                extract_collection_name("db.my_coll.aggregate(["),
                Some("my_coll".to_string())
            );
            assert_eq!(extract_collection_name("db."), None);
            assert_eq!(extract_collection_name("hello"), None);
            assert_eq!(
                extract_collection_name("db.a.find().sort(); db.b.find({"),
                Some("b".to_string())
            );
        }

        // ── Stop tokens ─────────────────────────────────────────────

        #[test]
        fn test_build_stop_tokens_empty_suffix() {
            let stops = build_stop_tokens("");
            assert_eq!(stops, vec!["\n".to_string()]);
        }

        #[test]
        fn test_build_stop_tokens_with_suffix() {
            let stops = build_stop_tokens("})");
            assert_eq!(stops, vec!["\n".to_string(), "}".to_string()]);
        }

        #[test]
        fn test_build_stop_tokens_suffix_starts_with_whitespace() {
            let stops = build_stop_tokens("  })");
            assert_eq!(stops, vec!["\n".to_string(), "}".to_string()]);
        }

        #[test]
        fn test_build_stop_tokens_suffix_newline_bracket() {
            // "\n])" — trim_start() skips \n as whitespace,
            // first non-whitespace char is ']', which gets added.
            let stops = build_stop_tokens("\n])");
            assert_eq!(stops, vec!["\n".to_string(), "]".to_string()]);
        }

        // ── Suffix overlap ──────────────────────────────────────────

        #[test]
        fn test_trim_suffix_overlap_no_overlap() {
            assert_eq!(trim_suffix_overlap("{$gt: 18}", "abc"), "{$gt: 18}");
        }

        #[test]
        fn test_trim_suffix_overlap_single_char() {
            // Completion ends with "}" which matches suffix "}" → trimmed.
            assert_eq!(trim_suffix_overlap("{$gt: 18}", "}"), "{$gt: 18");
        }

        #[test]
        fn test_trim_suffix_overlap_has_overlap() {
            assert_eq!(trim_suffix_overlap("{$gt: 18}})", "})"), "{$gt: 18}");
        }

        #[test]
        fn test_trim_suffix_overlap_full_overlap() {
            assert_eq!(trim_suffix_overlap("})", "})"), "");
        }

        #[test]
        fn test_trim_suffix_overlap_empty_suffix() {
            assert_eq!(trim_suffix_overlap("hello", ""), "hello");
        }

        #[test]
        fn test_trim_suffix_overlap_empty_completion() {
            assert_eq!(trim_suffix_overlap("", "})"), "");
        }

        // ── Service health ──────────────────────────────────────────

        #[test]
        fn test_health_initial_state() {
            let health = ServiceHealth::new();
            assert!(health.is_allowed());
            assert!(!health.degraded);
        }

        #[test]
        fn test_health_degrades_after_failures() {
            let mut health = ServiceHealth::new();
            for _ in 0..5 {
                health.record_failure();
            }
            assert!(health.degraded);
            assert!(!health.is_allowed());
        }

        #[test]
        fn test_health_recovers_on_success() {
            let mut health = ServiceHealth::new();
            for _ in 0..5 {
                health.record_failure();
            }
            assert!(health.degraded);

            health.record_success();
            assert!(!health.degraded);
            assert_eq!(health.consecutive_failures, 0);
        }
    }
}
