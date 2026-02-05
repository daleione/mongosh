//! Argument extraction utilities for parsing MongoDB operation arguments

use mongodb::bson::Document;

use crate::error::{ParseError, Result};
use crate::parser::command::{AggregateOptions, FindAndModifyOptions, UpdateOptions};
use crate::parser::mongo_ast::*;
use crate::parser::mongo_converter::ExpressionConverter;

/// Argument extraction utilities
pub struct ArgParser;

impl ArgParser {
    /// Extract collection and operation from db.collection.operation
    /// Returns (collection_name, operation_name)
    pub fn extract_db_call_target(callee: &Expr) -> Result<(String, String)> {
        // Must be a member expression: db.collection.operation
        if let Expr::Member(member) = callee {
            let operation = match &member.property {
                MemberProperty::Ident(name) => name.clone(),
                MemberProperty::Computed(expr) => {
                    // Handle computed property like db["collection"]["operation"]
                    if let Expr::String(s) = expr {
                        s.clone()
                    } else {
                        return Err(ParseError::InvalidCommand(
                            "Computed member expression must use string literal".to_string(),
                        )
                        .into());
                    }
                }
            };

            // The object should be db.collection
            if let Expr::Member(inner_member) = member.object.as_ref() {
                let collection = match &inner_member.property {
                    MemberProperty::Ident(name) => name.clone(),
                    MemberProperty::Computed(expr) => {
                        if let Expr::String(s) = expr {
                            s.clone()
                        } else {
                            return Err(ParseError::InvalidCommand(
                                "Collection name must be a string".to_string(),
                            )
                            .into());
                        }
                    }
                };

                // The inner object should be 'db'
                if let Expr::Ident(id) = inner_member.object.as_ref() {
                    if id == "db" {
                        return Ok((collection, operation));
                    }
                }
            }
        }

        Err(
            ParseError::InvalidCommand("Expected db.collection.operation() syntax".to_string())
                .into(),
        )
    }

    /// Get argument at index as BSON document
    pub fn get_doc_arg(args: &[Expr], index: usize) -> Result<Document> {
        if let Some(expr) = args.get(index) {
            let bson = ExpressionConverter::expr_to_bson(expr)?;
            if let mongodb::bson::Bson::Document(doc) = bson {
                Ok(doc)
            } else {
                Err(
                    ParseError::InvalidQuery(format!("Argument {} must be an object", index))
                        .into(),
                )
            }
        } else {
            Ok(Document::new())
        }
    }

    /// Get argument at index as BSON array of documents
    pub fn get_doc_array_arg(args: &[Expr], index: usize) -> Result<Vec<Document>> {
        if let Some(expr) = args.get(index) {
            let bson = ExpressionConverter::expr_to_bson(expr)?;
            if let mongodb::bson::Bson::Array(arr) = bson {
                let mut docs = Vec::new();
                for item in arr {
                    if let mongodb::bson::Bson::Document(doc) = item {
                        docs.push(doc);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "Array must contain only documents".to_string(),
                        )
                        .into());
                    }
                }
                Ok(docs)
            } else {
                Err(ParseError::InvalidQuery(format!("Argument {} must be an array", index)).into())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Get argument at index as string
    pub fn get_string_arg(args: &[Expr], index: usize) -> Result<String> {
        if let Some(expr) = args.get(index) {
            if let Expr::String(s) = expr {
                Ok(s.clone())
            } else {
                Err(ParseError::InvalidQuery(format!("Argument {} must be a string", index)).into())
            }
        } else {
            Err(ParseError::InvalidQuery(format!("Missing argument at index {}", index)).into())
        }
    }

    /// Get argument at index as number
    pub fn get_number_arg(args: &[Expr], index: usize) -> Result<i64> {
        if let Some(expr) = args.get(index) {
            if let Expr::Number(n) = expr {
                Ok(*n as i64)
            } else {
                Err(ParseError::InvalidQuery(format!("Argument {} must be a number", index)).into())
            }
        } else {
            Err(ParseError::InvalidQuery(format!("Missing argument at index {}", index)).into())
        }
    }

    /// Get update options from arguments
    pub fn get_update_options(args: &[Expr], index: usize) -> Result<UpdateOptions> {
        if let Some(expr) = args.get(index) {
            super::options::OptionsParser::parse_update_options(expr)
        } else {
            Ok(UpdateOptions::default())
        }
    }

    /// Get find and modify options from arguments
    pub fn get_find_and_modify_options(args: &[Expr], index: usize) -> Result<FindAndModifyOptions> {
        if let Some(expr) = args.get(index) {
            super::options::OptionsParser::parse_find_and_modify_options(expr)
        } else {
            Ok(FindAndModifyOptions::default())
        }
    }

    /// Get aggregate options from arguments
    pub fn get_aggregate_options(args: &[Expr], index: usize) -> Result<AggregateOptions> {
        if let Some(expr) = args.get(index) {
            super::options::OptionsParser::parse_aggregate_options(expr)
        } else {
            Ok(AggregateOptions::default())
        }
    }

    /// Get projection from arguments
    pub fn get_projection(args: &[Expr], index: usize) -> Result<Option<Document>> {
        if let Some(_expr) = args.get(index) {
            Ok(Some(Self::get_doc_arg(args, index)?))
        } else {
            Ok(None)
        }
    }
}
