//! Model management module
//! 
//! This module provides functionality for loading, managing, and executing AI models
//! as user-defined functions (UDFs) in the database.

pub mod model_manager;
pub mod model_udf;
pub mod onnx_runtime;

#[cfg(feature = "model-runtime")]
pub mod worker_protocol;

#[cfg(feature = "model-runtime")]
pub mod worker_manager;

#[cfg(feature = "model-runtime")]
pub mod builtin_models;

#[cfg(feature = "model-runtime")]
pub mod cache;

#[cfg(feature = "model-download")]
pub mod downloader;

pub use model_manager::{ModelManager, ModelError, ModelMetadata};
pub use model_udf::ModelUDF;
pub use onnx_runtime::{OnnxModel, ModelInfo, InputType};

#[cfg(feature = "model-runtime")]
pub use builtin_models::{BuiltinModel, BUILTIN_MODELS, get_builtin_model, list_builtin_models, register_builtin_models};

#[cfg(feature = "model-runtime")]
pub use cache::{CacheConfig, CacheStats, CacheKey, CacheEntry, InferenceCache, ThreadSafeCache, cached_inference, init_cache, get_cache, clear_cache, cache_stats};

#[cfg(feature = "model-download")]
pub use downloader::{DownloadError, DownloadProgress, download_model, download_model_sync, resolve_model_path, is_url};
