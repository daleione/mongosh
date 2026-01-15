//! BSON value conversion utilities
//!
//! This module provides unified BSON conversion with multiple strategies:
//! - Plain text conversion for data export
//! - Shell-style formatting (MongoDB shell compatible)
//! - Compact display for table cells
//! - JSON value conversion
//!
//! # Design
//!
//! The module uses a strategy pattern with a common trait `BsonConverter`
//! that allows different conversion strategies to be implemented and used
//! interchangeably.

mod converter;
mod helpers;
mod strategies;

pub use converter::BsonConverter;
pub use strategies::{CompactConverter, JsonConverter, PlainTextConverter, ShellStyleConverter};

#[cfg(test)]
mod tests;
