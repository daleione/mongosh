//! Context reader — reads pre-generated context files for FIM prompt injection.

use std::path::PathBuf;

/// Reads pre-generated AI context files from disk.
///
/// Used by the FIM completion service to inject collection schema and
/// common queries into the prompt, enabling accurate field completion.
pub struct ContextReader {
    context_dir: PathBuf,
}

impl ContextReader {
    /// Create a new context reader with the default context directory.
    pub fn new(context_dir: Option<PathBuf>) -> Self {
        let context_dir = context_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".mongosh")
                .join("context")
        });
        Self { context_dir }
    }

    /// Read the schema context for a specific collection.
    ///
    /// Returns the file content (plain text) for injection into the FIM prompt prefix.
    pub fn read_collection_schema(
        &self,
        datasource: &str,
        database: &str,
        collection: &str,
    ) -> Option<String> {
        let path = self
            .db_dir(datasource, database)
            .join("schemas")
            .join(format!("{}.md", collection));
        std::fs::read_to_string(path).ok()
    }

    /// Read the collections overview.
    pub fn read_overview(&self, datasource: &str, database: &str) -> Option<String> {
        let path = self.db_dir(datasource, database).join("collections.md");
        std::fs::read_to_string(path).ok()
    }

    /// Read the cross-collection queries context.
    #[allow(dead_code)]
    pub fn read_queries(&self, datasource: &str, database: &str) -> Option<String> {
        let path = self.db_dir(datasource, database).join("queries.md");
        std::fs::read_to_string(path).ok()
    }

    /// Read all per-collection schema files and return them as a vec of
    /// `(collection_name, schema_content)` pairs.
    ///
    /// Scans the `schemas/` subdirectory for `*.md` files. The collection
    /// name is derived from the file stem (e.g. `users.md` → `"users"`).
    pub fn read_all_schemas(&self, datasource: &str, database: &str) -> Vec<(String, String)> {
        let schemas_dir = self.db_dir(datasource, database).join("schemas");
        let mut results = Vec::new();

        let entries = match std::fs::read_dir(&schemas_dir) {
            Ok(entries) => entries,
            Err(_) => return results,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        results.push((stem.to_string(), content));
                    }
                }
            }
        }

        // Sort by collection name for deterministic prompt ordering
        results.sort_by(|a, b| a.0.cmp(&b.0));
        results
    }

    /// Check whether context files exist for a given database.
    pub fn has_context(&self, datasource: &str, database: &str) -> bool {
        self.db_dir(datasource, database)
            .join("_meta.toml")
            .exists()
    }

    fn db_dir(&self, datasource: &str, database: &str) -> PathBuf {
        let dir_name = if datasource.is_empty() {
            database.to_string()
        } else {
            format!("{}_{}", datasource, database)
        };
        self.context_dir.join(dir_name)
    }
}
