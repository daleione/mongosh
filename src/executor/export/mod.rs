//! Export module for streaming data export operations
//!
//! This module provides a comprehensive export system that supports:
//! - Streaming exports to avoid memory issues with large datasets
//! - Multiple query types (Find, Aggregate, etc.)
//! - Progress tracking with real-time feedback
//! - Multiple output formats (JSON Lines, CSV)
//!
//! # Architecture
//!
//! The export system is built on three main components:
//!
//! 1. **StreamingQuery**: Abstracts different query types into a unified streaming interface
//! 2. **ProgressTracker**: Provides real-time progress feedback to users
//! 3. **FormatWriter**: Handles writing documents to different file formats
//!
//! These components are orchestrated by the **ExportCoordinator**, which manages
//! the entire export pipeline.
//!
//! # Example
//!
//! ```no_run
//! // Example usage (requires MongoDB connection)
//! // This shows the general pattern for using the export system
//!
//! // 1. Create a cursor from a MongoDB query
//! // let cursor = collection.find(filter).await?;
//!
//! // 2. Create a streaming query
//! // let query = Box::new(CursorStreamingQuery::new(cursor, 1000, "Find"));
//!
//! // 3. Create progress tracker
//! // let tracker = ProgressTracker::new(None, true);
//!
//! // 4. Create format writer (async)
//! // let writer = Box::new(JsonLWriter::new("output.jsonl").await?);
//!
//! // 5. Create coordinator and execute
//! // let mut coordinator = ExportCoordinator::new(query, tracker, writer);
//! // let result = coordinator.execute().await?;
//! ```

pub mod coordinator;
pub mod progress;
pub mod streaming;
pub mod writers;

pub use coordinator::ExportCoordinator;
pub use progress::ProgressTracker;
pub use streaming::StreamingQuery;
pub use writers::{CsvWriter, FormatWriter, JsonLWriter};




#[cfg(test)]
mod tests {


}
