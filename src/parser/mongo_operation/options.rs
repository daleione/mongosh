//! Options parsing for MongoDB operations

use crate::error::{ParseError, Result};
use crate::parser::command::{AggregateOptions, FindAndModifyOptions, UpdateOptions};
use crate::parser::mongo_ast::*;
use crate::parser::mongo_converter::ExpressionConverter;

/// Options parsing utilities
pub struct OptionsParser;

impl OptionsParser {
    /// Parse find and modify options from expression
    pub fn parse_find_and_modify_options(expr: &Expr) -> Result<FindAndModifyOptions> {
        let doc = if let Expr::Object(obj) = expr {
            ExpressionConverter::object_to_bson(obj)?
        } else {
            return Err(ParseError::InvalidQuery("Options must be an object".to_string()).into());
        };

        let mut options = FindAndModifyOptions::default();

        if let Ok(sort) = doc.get_document("sort") {
            options.sort = Some(sort.clone());
        }

        if let Ok(projection) = doc.get_document("projection") {
            options.projection = Some(projection.clone());
        }

        if let Ok(return_new) = doc.get_bool("returnNew") {
            options.return_new = return_new;
        } else if let Ok(return_document) = doc.get_str("returnDocument") {
            options.return_new = match return_document {
                "after" => true,
                "before" => false,
                _ => {
                    return Err(ParseError::InvalidQuery(
                        "returnDocument must be 'before' or 'after'".to_string(),
                    )
                    .into());
                }
            };
        }

        if let Ok(upsert) = doc.get_bool("upsert") {
            options.upsert = upsert;
        }

        Ok(options)
    }

    /// Parse update options from expression
    pub fn parse_update_options(expr: &Expr) -> Result<UpdateOptions> {
        let doc = if let Expr::Object(obj) = expr {
            ExpressionConverter::object_to_bson(obj)?
        } else {
            return Err(ParseError::InvalidQuery("Options must be an object".to_string()).into());
        };

        let mut options = UpdateOptions::default();

        if let Ok(upsert) = doc.get_bool("upsert") {
            options.upsert = upsert;
        }

        if let Ok(array_filters) = doc.get_array("arrayFilters") {
            let mut filters = Vec::new();
            for filter in array_filters {
                if let mongodb::bson::Bson::Document(doc) = filter {
                    filters.push(doc.clone());
                }
            }
            options.array_filters = Some(filters);
        }

        Ok(options)
    }

    /// Parse aggregate options from expression
    pub fn parse_aggregate_options(expr: &Expr) -> Result<AggregateOptions> {
        let doc = if let Expr::Object(obj) = expr {
            ExpressionConverter::object_to_bson(obj)?
        } else {
            return Err(ParseError::InvalidQuery("Options must be an object".to_string()).into());
        };

        let mut options = AggregateOptions::default();

        if let Ok(batch_size) = doc.get_i32("batchSize") {
            if batch_size <= 0 {
                return Err(ParseError::InvalidQuery(
                    "batchSize must be a positive integer".to_string(),
                )
                .into());
            }
            options.batch_size = Some(batch_size as u32);
        } else if let Ok(batch_size) = doc.get_i64("batchSize") {
            if batch_size <= 0 {
                return Err(ParseError::InvalidQuery(
                    "batchSize must be a positive integer".to_string(),
                )
                .into());
            }
            options.batch_size = Some(batch_size as u32);
        }

        if let Ok(allow_disk_use) = doc.get_bool("allowDiskUse") {
            options.allow_disk_use = allow_disk_use;
        }

        if let Ok(max_time_ms) = doc.get_i64("maxTimeMS") {
            options.max_time_ms = Some(max_time_ms as u64);
        } else if let Ok(max_time_ms) = doc.get_i32("maxTimeMS") {
            options.max_time_ms = Some(max_time_ms as u64);
        }

        Ok(options)
    }
}
