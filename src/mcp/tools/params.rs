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
//! **Warning:** Using a plain string like `"2025-01-01 00:00:00"` will NOT match
//! a DateTime field — you must use the `{"$date": "..."}` wrapper.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

/// Default limit value for queries
fn default_limit() -> i64 {
    100
}

/// Default verbosity for explain operations
fn default_verbosity() -> String {
    "queryPlanner".to_string()
}

/// Default scale for collection stats
fn default_scale() -> i64 {
    1
}

/// Parameters for the find operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FindParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators ($eq, $gt, $lt, $in, $and, $or, etc.).
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    ///   - Date only: {"create_time": {"$gte": {"$date": "2025-01-01"}}} (midnight UTC)
    ///   - Epoch ms:  {"create_time": {"$gte": {"$date": 1735689600000}}}
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

    /// Maximum number of documents to return
    #[serde(default = "default_limit")]
    pub limit: i64,

    /// Number of documents to skip
    #[serde(default)]
    pub skip: Option<i64>,
}

/// Parameters for the findOne operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FindOneParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    ///   - Date only: {"create_time": {"$gte": {"$date": "2025-01-01"}}} (midnight UTC)
    ///   - Epoch ms:  {"create_time": {"$gte": {"$date": 1735689600000}}}
    /// Using a plain string for an ObjectId or DateTime field will NOT match any documents.
    #[serde(default)]
    pub filter: Option<serde_json::Value>,

    /// Projection to specify which fields to return (1 = include, 0 = exclude).
    /// Example: {"name": 1, "email": 1, "_id": 0}
    #[serde(default)]
    pub projection: Option<serde_json::Value>,
}

/// Parameters for the aggregate operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AggregateParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Aggregation pipeline stages as an array of stage objects.
    /// IMPORTANT: For BSON-typed values inside pipeline stages use Extended JSON v2 wrappers:
    ///   - ObjectId:  {"$match": {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}}
    ///   - DateTime:  {"$match": {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}}
    ///   - Date only: {"$match": {"create_time": {"$gte": {"$date": "2025-01-01"}}}}
    pub pipeline: Vec<serde_json::Value>,
}

/// Parameters for the count operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CountParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    #[serde(default)]
    pub filter: Option<serde_json::Value>,
}

/// Parameters for the distinct operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DistinctParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Field name to get distinct values for
    pub field: String,

    /// Query filter as a JSON object. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    #[serde(default)]
    pub filter: Option<serde_json::Value>,
}

/// Parameters for the insertOne operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertOneParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Document to insert as a JSON object.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"group_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"created_at": {"$date": "2025-01-01T00:00:00Z"}}
    ///   - Date only: {"created_at": {"$date": "2025-01-01"}} (midnight UTC)
    ///   - Epoch ms:  {"created_at": {"$date": 1735689600000}}
    /// If you omit _id, MongoDB will generate one automatically.
    pub document: serde_json::Value,
}

/// Parameters for the insertMany operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertManyParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Array of documents to insert.
    /// IMPORTANT: For BSON-typed fields in each document use Extended JSON v2 wrappers:
    ///   - ObjectId:  {"group_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"created_at": {"$date": "2025-01-01T00:00:00Z"}}
    ///   - Date only: {"created_at": {"$date": "2025-01-01"}} (midnight UTC)
    pub documents: Vec<serde_json::Value>,
}

/// Parameters for the updateOne operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateOneParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter to match document. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    pub filter: serde_json::Value,

    /// Update operations to perform using MongoDB update operators.
    /// IMPORTANT: For BSON-typed values use Extended JSON v2 wrappers:
    ///   - {"$set": {"updated_at": {"$date": "2025-06-01T00:00:00Z"}}}
    ///   - {"$set": {"ref_id": {"$oid": "69297ddcb4c39276cb39b05b"}}}
    pub update: serde_json::Value,
}

/// Parameters for the updateMany operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateManyParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter to match documents. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    pub filter: serde_json::Value,

    /// Update operations to perform using MongoDB update operators.
    /// IMPORTANT: For BSON-typed values use Extended JSON v2 wrappers:
    ///   - {"$set": {"updated_at": {"$date": "2025-06-01T00:00:00Z"}}}
    ///   - {"$set": {"ref_id": {"$oid": "69297ddcb4c39276cb39b05b"}}}
    pub update: serde_json::Value,
}

/// Parameters for the deleteOne operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteOneParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter to match the document to delete. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$lt": {"$date": "2025-01-01T00:00:00Z"}}}
    /// Using a plain string will NOT match ObjectId or DateTime fields.
    pub filter: serde_json::Value,
}

/// Parameters for the deleteMany operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteManyParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter to match documents to delete. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - Multiple:  {"_id": {"$in": [{"$oid": "69b3cd8d..."}, {"$oid": "69b3dcc9..."}]}}
    ///   - DateTime:  {"create_time": {"$lt": {"$date": "2025-01-01T00:00:00Z"}}}
    /// Using a plain string will NOT match ObjectId or DateTime fields.
    pub filter: serde_json::Value,
}

/// Parameters for listing collections in a database
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListCollectionsParams {
    /// Database name
    pub database: String,
}

/// Parameters for listing indexes on a collection
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListIndexesParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,
}

/// Parameters for getting collection statistics
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CollectionStatsParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Scale factor for sizes (1 = bytes, 1024 = KB, etc.)
    #[serde(default = "default_scale")]
    pub scale: i64,
}

/// Parameters for the explain operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExplainParams {
    /// Database name
    pub database: String,

    /// Collection name
    pub collection: String,

    /// Query filter to explain. Supports all MongoDB query operators.
    /// IMPORTANT: For BSON-typed fields use Extended JSON v2 wrappers, NOT plain strings:
    ///   - ObjectId:  {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
    ///   - DateTime:  {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
    pub filter: serde_json::Value,

    /// Verbosity level: "queryPlanner", "executionStats", or "allPlansExecution"
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
}

/// Parameters for switching the active database context
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UseDatabaseParams {
    /// Name of the database to switch to.
    /// After calling this tool, subsequent operations that omit the "database" field
    /// will use this database by default. You can call mongo_get_current_database
    /// to verify the switch was successful.
    pub database: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_find_params_serialization() {
        let params = FindParams {
            database: "testdb".to_string(),
            collection: "users".to_string(),
            filter: Some(json!({"age": {"$gt": 18}})),
            projection: Some(json!({"name": 1, "age": 1})),
            sort: Some(json!({"age": -1})),
            limit: 50,
            skip: Some(10),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("testdb"));
        assert!(json.contains("users"));
    }

    #[test]
    fn test_find_params_deserialization() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "filter": {"status": "active"},
            "limit": 25
        }"#;

        let params: FindParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert_eq!(params.limit, 25);
        assert!(params.filter.is_some());
    }

    #[test]
    fn test_find_params_default_limit() {
        let json = r#"{
            "database": "testdb",
            "collection": "users"
        }"#;

        let params: FindParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 100);
    }

    #[test]
    fn test_find_params_minimal() {
        let json = r#"{
            "database": "testdb",
            "collection": "users"
        }"#;

        let params: FindParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert!(params.filter.is_none());
        assert!(params.projection.is_none());
        assert!(params.sort.is_none());
        assert!(params.skip.is_none());
        assert_eq!(params.limit, 100);
    }

    #[test]
    fn test_find_one_params_minimal() {
        let json = r#"{
            "database": "testdb",
            "collection": "users"
        }"#;

        let params: FindOneParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert!(params.filter.is_none());
        assert!(params.projection.is_none());
    }

    #[test]
    fn test_aggregate_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "orders",
            "pipeline": [
                {"$match": {"status": "completed"}},
                {"$group": {"_id": "$customerId", "total": {"$sum": "$amount"}}}
            ]
        }"#;

        let params: AggregateParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "orders");
        assert_eq!(params.pipeline.len(), 2);
    }

    #[test]
    fn test_count_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "products",
            "filter": {"inStock": true}
        }"#;

        let params: CountParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "products");
        assert!(params.filter.is_some());
    }

    #[test]
    fn test_distinct_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "field": "country",
            "filter": {"active": true}
        }"#;

        let params: DistinctParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert_eq!(params.field, "country");
        assert!(params.filter.is_some());
    }

    #[test]
    fn test_insert_one_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "document": {"name": "John", "age": 30}
        }"#;

        let params: InsertOneParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert!(params.document.is_object());
    }

    #[test]
    fn test_insert_many_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "documents": [
                {"name": "John", "age": 30},
                {"name": "Jane", "age": 25}
            ]
        }"#;

        let params: InsertManyParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert_eq!(params.documents.len(), 2);
    }

    #[test]
    fn test_update_one_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "filter": {"_id": "123"},
            "update": {"$set": {"status": "active"}}
        }"#;

        let params: UpdateOneParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert!(params.filter.is_object());
        assert!(params.update.is_object());
    }

    #[test]
    fn test_update_many_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "filter": {"status": "inactive"},
            "update": {"$set": {"lastChecked": "2024-01-01"}}
        }"#;

        let params: UpdateManyParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
    }

    #[test]
    fn test_delete_one_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "filter": {"_id": "123"}
        }"#;

        let params: DeleteOneParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert!(params.filter.is_object());
    }

    #[test]
    fn test_delete_many_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "logs",
            "filter": {"timestamp": {"$lt": "2024-01-01"}}
        }"#;

        let params: DeleteManyParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "logs");
    }

    #[test]
    fn test_list_collections_params() {
        let json = r#"{
            "database": "testdb"
        }"#;

        let params: ListCollectionsParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
    }

    #[test]
    fn test_list_indexes_params() {
        let json = r#"{
            "database": "testdb",
            "collection": "users"
        }"#;

        let params: ListIndexesParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
    }

    #[test]
    fn test_collection_stats_params_default_scale() {
        let json = r#"{
            "database": "testdb",
            "collection": "users"
        }"#;

        let params: CollectionStatsParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert_eq!(params.scale, 1);
    }

    #[test]
    fn test_collection_stats_params_custom_scale() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "scale": 1024
        }"#;

        let params: CollectionStatsParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.scale, 1024);
    }

    #[test]
    fn test_explain_params_default_verbosity() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "filter": {"age": {"$gt": 18}}
        }"#;

        let params: ExplainParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.database, "testdb");
        assert_eq!(params.collection, "users");
        assert_eq!(params.verbosity, "queryPlanner");
    }

    #[test]
    fn test_explain_params_custom_verbosity() {
        let json = r#"{
            "database": "testdb",
            "collection": "users",
            "filter": {"age": {"$gt": 18}},
            "verbosity": "executionStats"
        }"#;

        let params: ExplainParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.verbosity, "executionStats");
    }
}
