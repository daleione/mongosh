//! MongoDB metadata sampler
//!
//! Collects collection names, document counts, indexes, and sample documents
//! from a MongoDB database. This raw data is then fed into the Chat API
//! for structured context generation.

use futures::TryStreamExt;
use mongodb::bson::Document;

use crate::error::{MongoshError, Result};
use crate::executor::ExecutionContext;

/// Information about a single index.
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub keys: Document,
    pub unique: bool,
}

/// Raw sample data for a single collection.
#[derive(Debug, Clone)]
pub struct CollectionSample {
    pub name: String,
    pub document_count: u64,
    pub indexes: Vec<IndexInfo>,
    pub sample_documents: Vec<Document>,
}

/// Raw sample data for an entire database.
#[derive(Debug, Clone)]
pub struct DatabaseSample {
    pub database: String,
    pub datasource: String,
    pub collections: Vec<CollectionSample>,
}

/// Sampler — collects metadata from MongoDB.
pub struct Sampler;

impl Sampler {
    /// Sample all collections in the current database.
    ///
    /// For each collection:
    /// 1. `estimatedDocumentCount()`
    /// 2. `listIndexes()`
    /// 3. `find().limit(sample_size)`
    pub async fn sample_database(
        ctx: &ExecutionContext,
        datasource: &str,
        sample_size: usize,
    ) -> Result<DatabaseSample> {
        let db_name = ctx.get_current_database().await;
        let db = ctx.get_database().await?;
        let collection_names = db
            .list_collection_names()
            .await
            .map_err(|e| MongoshError::Generic(format!("Failed to list collections: {}", e)))?;

        let mut collections = Vec::new();
        for name in &collection_names {
            if name.starts_with("system.") {
                continue;
            }
            match Self::sample_collection(ctx, name, sample_size).await {
                Ok(sample) => collections.push(sample),
                Err(e) => {
                    tracing::warn!("Failed to sample collection '{}': {}", name, e);
                }
            }
        }

        Ok(DatabaseSample {
            database: db_name,
            datasource: datasource.to_string(),
            collections,
        })
    }

    /// Sample a single collection.
    pub async fn sample_collection(
        ctx: &ExecutionContext,
        collection_name: &str,
        sample_size: usize,
    ) -> Result<CollectionSample> {
        let db = ctx.get_database().await?;
        let coll = db.collection::<Document>(collection_name);

        // 1. Document count
        let count = coll.estimated_document_count().await.unwrap_or(0);

        // 2. Indexes
        let mut indexes = Vec::new();
        let mut idx_cursor = coll
            .list_indexes()
            .await
            .map_err(|e| MongoshError::Generic(format!("Failed to list indexes: {}", e)))?;
        while let Some(idx) = idx_cursor
            .try_next()
            .await
            .map_err(|e| MongoshError::Generic(format!("Failed to read index: {}", e)))?
        {
            indexes.push(IndexInfo {
                name: idx
                    .options
                    .as_ref()
                    .and_then(|o| o.name.clone())
                    .unwrap_or_default(),
                keys: idx.keys.clone(),
                unique: idx.options.as_ref().and_then(|o| o.unique).unwrap_or(false),
            });
        }

        // 3. Sample documents
        let mut sample_docs = Vec::new();
        let mut cursor = coll
            .find(mongodb::bson::doc! {})
            .limit(sample_size as i64)
            .await
            .map_err(|e| MongoshError::Generic(format!("Failed to sample documents: {}", e)))?;
        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| MongoshError::Generic(format!("Failed to read document: {}", e)))?
        {
            sample_docs.push(doc);
        }

        Ok(CollectionSample {
            name: collection_name.to_string(),
            document_count: count,
            indexes,
            sample_documents: sample_docs,
        })
    }
}
