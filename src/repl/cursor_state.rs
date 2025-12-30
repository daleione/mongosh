/// Cursor state for pagination
#[derive(Debug, Clone)]
pub struct CursorState {
    /// Collection name
    pub collection: String,
    /// Query filter
    pub filter: mongodb::bson::Document,
    /// Query options
    pub options: crate::parser::FindOptions,
    /// Number of documents already retrieved
    pub documents_retrieved: usize,
    /// Total documents matched (if known)
    pub total_matched: Option<usize>,
    /// Whether this is the last batch
    pub has_more: bool,
}

impl CursorState {
    /// Create a new cursor state
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `options` - Query options
    /// * `total_matched` - Total matched documents if known
    ///
    /// # Returns
    /// * `Self` - New cursor state
    pub fn new(
        collection: String,
        filter: mongodb::bson::Document,
        options: crate::parser::FindOptions,
        total_matched: Option<usize>,
    ) -> Self {
        Self {
            collection,
            filter,
            options,
            documents_retrieved: 0,
            total_matched,
            has_more: true,
        }
    }

    /// Get the skip value for next batch
    ///
    /// # Returns
    /// * `u64` - Number of documents to skip
    pub fn get_skip(&self) -> u64 {
        self.documents_retrieved as u64
    }

    /// Update after retrieving documents
    ///
    /// # Arguments
    /// * `batch_size` - Number of documents retrieved in this batch
    /// * `total_matched` - Updated total matched documents (if known)
    pub fn update(&mut self, batch_size: usize, total_matched: Option<usize>) {
        self.documents_retrieved += batch_size;
        if let Some(total) = total_matched {
            self.total_matched = Some(total);
            // has_more will be set by the caller based on cursor state
        }
        // Note: has_more field should be set by the caller after this update
        // based on whether the cursor actually has more documents
    }

    /// Check if there are more documents
    ///
    /// # Returns
    /// * `bool` - True if more documents are available
    pub fn has_more(&self) -> bool {
        self.has_more
    }
}
