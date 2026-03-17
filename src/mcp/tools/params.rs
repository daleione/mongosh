//! Parameter structures for MongoDB MCP tools
//!
//! ## BSON Type Handling in Filters and Documents
//!
//! All filter, document, and pipeline parameters support **MongoDB Extended JSON v2** format
//! for expressing BSON-specific types. This is critical for correct operation:
//!
//! ### ObjectId
//! ```json
//! {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
//! ```
//!
//! ### DateTime
//! ```json
//! {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
//! {"create_time": {"$gte": {"$date": "2025-01-01"}}}
//! {"create_time": {"$gte": {"$date": 1735689600000}}}
//! ```
//!
//! ### Other types
//! ```json
//! {"amount": {"$numberDecimal": "3.14"}}
//! {"count":  {"$numberLong":    "9007199254740993"}}
//! ```
//!
//! **Warning:** Using a plain string like "2025-01-01 00:00:00" will NOT match
//! a DateTime field — you must use the {"$date": "..."} wrapper.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

fn default_limit() -> i64 {
    100
}

fn default_verbosity() -> String {
    "queryPlanner".to_string()
}

fn default_scale() -> i64 {
    1
}

// ── Read parameters ───────────────────────────────────────────────────────────

/// Parameters for the `find` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FindParams {
    /// Datasource name to query against (e.g. "shop_prod", "analytics_db").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators
    /// ($eq, $gt, $lt, $in, $and, $or, …).
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    ///   - Date only: {"create_time": {"$gte": {"$date": "2025-01-01"}}} (midnight UTC)
    ///   - Epoch ms:  {"create_time": {"$gte": {"$date": 1735689600000}}}
    ///
    /// Using a plain string for an ObjectId or DateTime field will NOT match any documents.
    #[serde(default)]
    pub filter: Option<serde_json::Value>,

    /// Projection to specify which fields to return (1 = include, 0 = exclude).
    /// Example: {"name": 1, "email": 1, "_id": 0}
    #[serde(default)]
    pub projection: Option<serde_json::Value>,

    /// Sort specification (1 = ascending, -1 = descending).
    /// Example: {"create_time": -1, "name": 1}
    #[serde(default)]
    pub sort: Option<serde_json::Value>,

    /// Maximum number of documents to return.
    #[serde(default = "default_limit")]
    pub limit: i64,

    /// Number of documents to skip.
    #[serde(default)]
    pub skip: Option<i64>,
}

/// Parameters for the `findOne` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FindOneParams {
    /// Datasource name to query against (e.g. "shop_prod", "analytics_db").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    ///   - Date only: {"create_time": {"$gte": {"$date": "2025-01-01"}}} (midnight UTC)
    ///   - Epoch ms:  {"create_time": {"$gte": {"$date": 1735689600000}}}
    ///
    /// Using a plain string for an ObjectId or DateTime field will NOT match any documents.
    #[serde(default)]
    pub filter: Option<serde_json::Value>,

    /// Projection to specify which fields to return (1 = include, 0 = exclude).
    /// Example: {"name": 1, "email": 1, "_id": 0}
    #[serde(default)]
    pub projection: Option<serde_json::Value>,
}

/// Parameters for the `aggregate` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AggregateParams {
    /// Datasource name to query against (e.g. "shop_prod", "analytics_db").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Aggregation pipeline stages as an array of stage objects.
    ///
    /// IMPORTANT: For BSON-typed values inside pipeline stages use Extended JSON v2 wrappers:
    ///   - ObjectId:  {"$match": {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}}
    ///   - DateTime:  {"$match": {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}}
    ///   - Date only: {"$match": {"create_time": {"$gte": {"$date": "2025-01-01"}}}}
    pub pipeline: Vec<serde_json::Value>,
}

/// Parameters for the `count` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CountParams {
    /// Datasource name to query against (e.g. "shop_prod", "analytics_db").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    #[serde(default)]
    pub filter: Option<serde_json::Value>,
}

/// Parameters for the `distinct` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DistinctParams {
    /// Datasource name to query against (e.g. "shop_prod", "analytics_db").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Field name to get distinct values for.
    pub field: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    #[serde(default)]
    pub filter: Option<serde_json::Value>,
}

// ── Write parameters ──────────────────────────────────────────────────────────

/// Parameters for the `insertOne` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertOneParams {
    /// Datasource name to write to (e.g. "myapp_prod").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Document to insert as a JSON object.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"group_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"created_at": {"$date": "2025-01-01T00:00:00Z"}}
    ///   - Date only: {"created_at": {"$date": "2025-01-01"}} (midnight UTC)
    ///   - Epoch ms:  {"created_at": {"$date": 1735689600000}}
    ///
    /// If you omit _id, MongoDB will generate one automatically.
    pub document: serde_json::Value,
}

/// Parameters for the `insertMany` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertManyParams {
    /// Datasource name to write to (e.g. "myapp_prod").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Array of documents to insert.
    ///
    /// IMPORTANT: For BSON-typed fields in each document use Extended JSON v2 wrappers:
    ///   - ObjectId:  {"group_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"created_at": {"$date": "2025-01-01T00:00:00Z"}}
    ///   - Date only: {"created_at": {"$date": "2025-01-01"}} (midnight UTC)
    pub documents: Vec<serde_json::Value>,
}

/// Parameters for the `updateOne` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateOneParams {
    /// Datasource name to write to (e.g. "myapp_prod").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter to match the document. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    pub filter: serde_json::Value,

    /// Update operations to perform using MongoDB update operators.
    ///
    /// IMPORTANT: For BSON-typed values use Extended JSON v2 wrappers:
    ///   - {"$set": {"updated_at": {"$date": "2025-06-01T00:00:00Z"}}}
    ///   - {"$set": {"ref_id": {"$oid": "69297ddcb4c39276cb39b05b"}}}
    pub update: serde_json::Value,
}

/// Parameters for the `updateMany` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateManyParams {
    /// Datasource name to write to (e.g. "myapp_prod").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter to match documents. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    pub filter: serde_json::Value,

    /// Update operations to perform using MongoDB update operators.
    ///
    /// IMPORTANT: For BSON-typed values use Extended JSON v2 wrappers:
    ///   - {"$set": {"updated_at": {"$date": "2025-06-01T00:00:00Z"}}}
    ///   - {"$set": {"ref_id": {"$oid": "69297ddcb4c39276cb39b05b"}}}
    pub update: serde_json::Value,
}

// ── Delete parameters ─────────────────────────────────────────────────────────

/// Parameters for the `deleteOne` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteOneParams {
    /// Datasource name to delete from (e.g. "myapp_prod").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter to match the document to delete. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$lt": {"$date": "2025-01-01T00:00:00Z"}}}
    ///
    /// Using a plain string will NOT match ObjectId or DateTime fields.
    pub filter: serde_json::Value,
}

/// Parameters for the `deleteMany` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteManyParams {
    /// Datasource name to delete from (e.g. "myapp_prod").
    /// If provided, the server switches to that datasource before executing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter to match documents to delete. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - Multiple:  {"_id": {"$in": [{"$oid": "69b3cd8d..."}, {"$oid": "69b3dcc9..."}]}}
    ///   - DateTime:  {"create_time": {"$lt": {"$date": "2025-01-01T00:00:00Z"}}}
    ///
    /// Using a plain string will NOT match ObjectId or DateTime fields.
    pub filter: serde_json::Value,
}

// ── Admin parameters ──────────────────────────────────────────────────────────

/// Parameters for listing collections in a database.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListCollectionsParams {
    /// Datasource name (e.g. "shop_prod"). Switches datasource before listing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,
}

/// Parameters for listing indexes on a collection.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListIndexesParams {
    /// Datasource name (e.g. "shop_prod"). Switches datasource before listing.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,
}

/// Parameters for getting collection statistics.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CollectionStatsParams {
    /// Datasource name (e.g. "shop_prod"). Switches datasource before querying.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Scale factor for sizes (1 = bytes, 1024 = KB, 1048576 = MB, …).
    #[serde(default = "default_scale")]
    pub scale: i64,
}

/// Parameters for the `explain` operation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExplainParams {
    /// Datasource name (e.g. "shop_prod"). Switches datasource before explaining.
    /// If omitted, uses the currently active datasource.
    #[serde(default)]
    pub datasource: Option<String>,

    /// Database name. If not provided, uses the current active database.
    #[serde(default)]
    pub database: Option<String>,

    /// Collection name.
    pub collection: String,

    /// Query filter to explain. Supports all MongoDB query operators.
    ///
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    pub filter: serde_json::Value,

    /// Verbosity level: "queryPlanner", "executionStats", or "allPlansExecution".
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
}

// ── Context / datasource parameters ──────────────────────────────────────────

/// Parameters for the `prepare_context` operation.
///
/// Use this tool ONLY when the datasource is ambiguous or you need to
/// explore what collections are available.  If the user has already
/// mentioned a datasource name, pass it directly to the query tool
/// via its `datasource` field instead.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PrepareContextParams {
    /// Datasource name to switch to, as defined in the config file
    /// (e.g. "myapp_prod", "analytics_db").
    /// Must be provided — do NOT call this tool with an empty payload.
    pub datasource: String,

    /// Optional database override. If provided, the active database is set
    /// to this value after the datasource switch (useful when the URI does
    /// not embed a database name or you want to target a different database
    /// on the same cluster).
    #[serde(default)]
    pub database: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── FindParams ────────────────────────────────────────────────────────────

    #[test]
    fn find_params_all_optional_fields_default_to_none() {
        let p: FindParams = serde_json::from_value(json!({
            "collection": "users"
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.collection, "users");
        assert_eq!(p.limit, 100);
    }

    #[test]
    fn find_params_with_datasource_and_database() {
        let p: FindParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "database": "mydb",
            "collection": "users"
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert_eq!(p.database, Some("mydb".to_string()));
    }

    #[test]
    fn find_params_datasource_only() {
        let p: FindParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "collection": "templates"
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert!(p.database.is_none());
    }

    #[test]
    fn find_params_full() {
        let p: FindParams = serde_json::from_value(json!({
            "database": "mydb",
            "collection": "users",
            "filter": {"age": {"$gt": 18}},
            "projection": {"name": 1},
            "sort": {"name": 1},
            "limit": 50,
            "skip": 10
        }))
        .unwrap();
        assert_eq!(p.database, Some("mydb".to_string()));
        assert_eq!(p.limit, 50);
        assert_eq!(p.skip, Some(10));
        assert!(p.filter.is_some());
        assert!(p.projection.is_some());
        assert!(p.sort.is_some());
    }

    #[test]
    fn find_params_default_limit() {
        let p: FindParams = serde_json::from_value(json!({ "collection": "c" })).unwrap();
        assert_eq!(p.limit, 100);
    }

    // ── FindOneParams ─────────────────────────────────────────────────────────

    #[test]
    fn find_one_params_minimal() {
        let p: FindOneParams = serde_json::from_value(json!({ "collection": "users" })).unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert!(p.filter.is_none());
        assert!(p.projection.is_none());
    }

    #[test]
    fn find_one_params_with_datasource() {
        let p: FindOneParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "collection": "templates",
            "filter": {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert!(p.database.is_none());
        assert!(p.filter.is_some());
    }

    // ── AggregateParams ───────────────────────────────────────────────────────

    #[test]
    fn aggregate_params_minimal() {
        let p: AggregateParams = serde_json::from_value(json!({
            "collection": "orders",
            "pipeline": [{"$match": {"status": "active"}}, {"$count": "total"}]
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.pipeline.len(), 2);
    }

    #[test]
    fn aggregate_params_with_datasource() {
        let p: AggregateParams = serde_json::from_value(json!({
            "datasource": "analytics_db",
            "collection": "orders",
            "pipeline": [{"$match": {}}]
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("analytics_db".to_string()));
        assert!(p.database.is_none());
    }

    // ── CountParams ───────────────────────────────────────────────────────────

    #[test]
    fn count_params_minimal() {
        let p: CountParams = serde_json::from_value(json!({ "collection": "users" })).unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert!(p.filter.is_none());
    }

    #[test]
    fn count_params_with_datasource_and_filter() {
        let p: CountParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "collection": "templates",
            "filter": {"status": "active"}
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert!(p.database.is_none());
        assert!(p.filter.is_some());
    }

    // ── DistinctParams ────────────────────────────────────────────────────────

    #[test]
    fn distinct_params_minimal() {
        let p: DistinctParams = serde_json::from_value(json!({
            "collection": "users",
            "field": "status"
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.field, "status");
    }

    #[test]
    fn distinct_params_with_datasource() {
        let p: DistinctParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "collection": "templates",
            "field": "category"
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
    }

    // ── InsertOneParams ───────────────────────────────────────────────────────

    #[test]
    fn insert_one_params_minimal() {
        let p: InsertOneParams = serde_json::from_value(json!({
            "collection": "users",
            "document": {"name": "Alice", "age": 30}
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
    }

    #[test]
    fn insert_one_params_with_datasource() {
        let p: InsertOneParams = serde_json::from_value(json!({
            "datasource": "myapp_prod",
            "collection": "users",
            "document": {"name": "Alice"}
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("myapp_prod".to_string()));
    }

    // ── InsertManyParams ──────────────────────────────────────────────────────

    #[test]
    fn insert_many_params_minimal() {
        let p: InsertManyParams = serde_json::from_value(json!({
            "collection": "users",
            "documents": [{"name": "Alice"}, {"name": "Bob"}]
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.documents.len(), 2);
    }

    // ── UpdateOneParams ───────────────────────────────────────────────────────

    #[test]
    fn update_one_params_minimal() {
        let p: UpdateOneParams = serde_json::from_value(json!({
            "collection": "users",
            "filter": {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}},
            "update": {"$set": {"status": "active"}}
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
    }

    #[test]
    fn update_one_params_with_datasource() {
        let p: UpdateOneParams = serde_json::from_value(json!({
            "datasource": "myapp_prod",
            "collection": "users",
            "filter": {"name": "Alice"},
            "update": {"$set": {"age": 31}}
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("myapp_prod".to_string()));
    }

    // ── UpdateManyParams ──────────────────────────────────────────────────────

    #[test]
    fn update_many_params_minimal() {
        let p: UpdateManyParams = serde_json::from_value(json!({
            "collection": "users",
            "filter": {"status": "inactive"},
            "update": {"$set": {"archived": true}}
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
    }

    // ── DeleteOneParams ───────────────────────────────────────────────────────

    #[test]
    fn delete_one_params_minimal() {
        let p: DeleteOneParams = serde_json::from_value(json!({
            "collection": "users",
            "filter": {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
    }

    #[test]
    fn delete_one_params_with_datasource() {
        let p: DeleteOneParams = serde_json::from_value(json!({
            "datasource": "myapp_prod",
            "collection": "users",
            "filter": {"name": "Alice"}
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("myapp_prod".to_string()));
    }

    // ── DeleteManyParams ──────────────────────────────────────────────────────

    #[test]
    fn delete_many_params_minimal() {
        let p: DeleteManyParams = serde_json::from_value(json!({
            "collection": "logs",
            "filter": {"level": "debug"}
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
    }

    // ── ListCollectionsParams ─────────────────────────────────────────────────

    #[test]
    fn list_collections_params_minimal() {
        let p: ListCollectionsParams = serde_json::from_value(json!({})).unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
    }

    #[test]
    fn list_collections_params_with_datasource() {
        let p: ListCollectionsParams =
            serde_json::from_value(json!({ "datasource": "shop_prod" })).unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert!(p.database.is_none());
    }

    // ── ListIndexesParams ─────────────────────────────────────────────────────

    #[test]
    fn list_indexes_params_minimal() {
        let p: ListIndexesParams =
            serde_json::from_value(json!({ "collection": "users" })).unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.collection, "users");
    }

    // ── CollectionStatsParams ─────────────────────────────────────────────────

    #[test]
    fn collection_stats_params_default_scale() {
        let p: CollectionStatsParams = serde_json::from_value(json!({
            "collection": "users"
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.scale, 1);
    }

    #[test]
    fn collection_stats_params_with_datasource() {
        let p: CollectionStatsParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "collection": "templates",
            "scale": 1024
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert!(p.database.is_none());
        assert_eq!(p.scale, 1024);
    }

    // ── ExplainParams ─────────────────────────────────────────────────────────

    #[test]
    fn explain_params_default_verbosity() {
        let p: ExplainParams = serde_json::from_value(json!({
            "collection": "users",
            "filter": {"age": {"$gt": 18}}
        }))
        .unwrap();
        assert!(p.datasource.is_none());
        assert!(p.database.is_none());
        assert_eq!(p.verbosity, "queryPlanner");
    }

    #[test]
    fn explain_params_with_datasource() {
        let p: ExplainParams = serde_json::from_value(json!({
            "datasource": "shop_prod",
            "collection": "templates",
            "filter": {},
            "verbosity": "executionStats"
        }))
        .unwrap();
        assert_eq!(p.datasource, Some("shop_prod".to_string()));
        assert_eq!(p.verbosity, "executionStats");
    }

    // ── PrepareContextParams ──────────────────────────────────────────────────

    #[test]
    fn prepare_context_params_datasource_only() {
        let p: PrepareContextParams =
            serde_json::from_value(json!({ "datasource": "myapp_prod" })).unwrap();
        assert_eq!(p.datasource, "myapp_prod");
        assert!(p.database.is_none());
    }

    #[test]
    fn prepare_context_params_both_fields() {
        let p: PrepareContextParams = serde_json::from_value(json!({
            "datasource": "myapp_prod",
            "database": "myapp"
        }))
        .unwrap();
        assert_eq!(p.datasource, "myapp_prod");
        assert_eq!(p.database, Some("myapp".to_string()));
    }
}
