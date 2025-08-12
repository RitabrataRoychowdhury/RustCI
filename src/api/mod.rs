//! Valkyrie Protocol API Module
//!
//! This module provides the external API for the Valkyrie Protocol,
//! including high-level abstractions and developer-friendly interfaces.

pub mod valkyrie;
pub mod codegen;
pub mod adapters;
pub mod conversions;

pub use valkyrie::ValkyrieEngineAdapter;
pub use codegen::{CodeGenerator, extract_api_definition};