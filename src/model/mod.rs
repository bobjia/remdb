//! Model management module
//! 
//! This module provides functionality for loading, managing, and executing AI models
//! as user-defined functions (UDFs) in the database.

pub mod model_manager;
pub mod model_udf;
pub mod onnx_runtime;

pub use model_manager::{ModelManager, ModelError};
pub use model_udf::ModelUDF;
pub use onnx_runtime::OnnxModel;
