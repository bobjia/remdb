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

pub use model_manager::{ModelError, ModelManager, ModelMetadata};
pub use model_udf::ModelUDF;
pub use onnx_runtime::{InputType, ModelInfo, OnnxModel};

#[cfg(feature = "model-runtime")]
pub use builtin_models::{
    get_builtin_model, list_builtin_models, register_builtin_models, BuiltinModel, BUILTIN_MODELS,
};

#[cfg(feature = "model-runtime")]
pub use cache::{
    cache_stats, cached_inference, clear_cache, get_cache, init_cache, CacheConfig, CacheEntry,
    CacheKey, CacheStats, InferenceCache, ThreadSafeCache,
};

#[cfg(feature = "model-download")]
pub use downloader::{
    download_model, download_model_sync, is_url, resolve_model_path, DownloadError,
    DownloadProgress,
};
