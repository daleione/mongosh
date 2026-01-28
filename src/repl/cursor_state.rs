use mongodb::Cursor;
use mongodb::bson::Document;
use std::time::Instant;
use std::fmt;

/// Active cursor state for pagination
///
/// This struct holds a live MongoDB cursor between pagination calls,
/// eliminating the need for skip() operations and providing optimal performance.
pub struct CursorState {
    /// Collection name (for display purposes)
    pub collection_name: String,

    /// Active MongoDB cursor
    /// The cursor maintains its position on the server side
    pub cursor: Cursor<Document>,

    /// Number of documents retrieved so far
    pub documents_retrieved: usize,

    /// Batch size for pagination
    pub batch_size: u32,

    /// Creation timestamp for timeout detection
    pub created_at: Instant,
}

impl CursorState {
    /// Create a new cursor state
    ///
    /// # Arguments
    /// * `collection_name` - Name of the collection being queried
    /// * `cursor` - Active MongoDB cursor
    /// * `batch_size` - Number of documents to fetch per batch
    ///
    /// # Returns
    /// * `Self` - New cursor state instance
    pub fn new(
        collection_name: String,
        cursor: Cursor<Document>,
        batch_size: u32,
    ) -> Self {
        Self {
            collection_name,
            cursor,
            documents_retrieved: 0,
            batch_size,
            created_at: Instant::now(),
        }
    }

    /// Check if the cursor has expired (10 minute timeout)
    ///
    /// MongoDB cursors timeout after 10 minutes of inactivity by default.
    /// This matches that behavior.
    ///
    /// # Returns
    /// * `bool` - True if cursor has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() > 600
    }

    /// Update the count of retrieved documents
    ///
    /// # Arguments
    /// * `count` - Number of documents retrieved in this batch
    pub fn update_retrieved(&mut self, count: usize) {
        self.documents_retrieved += count;
    }

    /// Get the collection name
    ///
    /// # Returns
    /// * `&str` - Reference to the collection name
    #[allow(dead_code)]
    pub fn collection_name(&self) -> &str {
        &self.collection_name
    }

    /// Get the number of documents retrieved so far
    ///
    /// # Returns
    /// * `usize` - Total number of documents retrieved
    pub fn documents_retrieved(&self) -> usize {
        self.documents_retrieved
    }
}

/// Manual Debug implementation since Cursor doesn't implement Debug
impl fmt::Debug for CursorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CursorState")
            .field("collection_name", &self.collection_name)
            .field("documents_retrieved", &self.documents_retrieved)
            .field("batch_size", &self.batch_size)
            .field("created_at", &self.created_at)
            .field("cursor", &"<MongoDB Cursor>")
            .finish()
    }
}
