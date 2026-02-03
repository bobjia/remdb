//! Model manager
//! 
//! This module manages the lifecycle of AI models, including loading, unloading, and caching.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;

use crate::model::onnx_runtime::OnnxModel;

/// Model error types
#[derive(Debug, Clone, PartialEq)]
pub enum ModelError {
    /// Model file not found
    FileNotFound,
    /// Model loading failed
    LoadFailed,
    /// Model execution failed
    ExecutionFailed,
    /// Invalid model input
    InvalidInput,
    /// Model not found
    ModelNotFound,
    /// Model already exists
    ModelAlreadyExists,
}

impl core::fmt::Display for ModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ModelError::FileNotFound => write!(f, "Model file not found"),
            ModelError::LoadFailed => write!(f, "Failed to load model"),
            ModelError::ExecutionFailed => write!(f, "Model execution failed"),
            ModelError::InvalidInput => write!(f, "Invalid model input"),
            ModelError::ModelNotFound => write!(f, "Model not found"),
            ModelError::ModelAlreadyExists => write!(f, "Model already exists"),
        }
    }
}

impl core::error::Error for ModelError {}

/// Model metadata
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// Model name
    pub name: String,
    /// Model file path
    pub path: String,
    /// Input parameters: (name, type)
    pub inputs: Vec<(String, String)>,
    /// Output: (name, type)
    pub output: (String, String),
}

use lazy_static::lazy_static;
use std::sync::Mutex;

/// Model manager
#[derive(Debug)]
pub struct ModelManager {
    /// Loaded models
    models: BTreeMap<String, Arc<OnnxModel>>,
    /// Model metadata
    metadata: BTreeMap<String, ModelMetadata>,
}

/// Global model manager
lazy_static! {
    pub(crate) static ref GLOBAL_MODEL_MANAGER: Mutex<ModelManager> = Mutex::new(ModelManager::new());
}

/// Get the global model manager
pub fn get_global_model_manager() -> Result<std::sync::MutexGuard<'static, ModelManager>, ModelError> {
    GLOBAL_MODEL_MANAGER.lock().map_err(|_| ModelError::LoadFailed)
}

impl Default for ModelManager {
    fn default() -> Self {
        Self {
            models: BTreeMap::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl ModelManager {
    /// Create a new model manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a model
    pub fn register_model(
        &mut self,
        name: String,
        path: String,
        inputs: Vec<(String, String)>,
        output: (String, String),
    ) -> Result<(), ModelError> {
        // Check if model already exists
        if self.models.contains_key(&name) {
            return Err(ModelError::ModelAlreadyExists);
        }

        // Load the model
        let model = OnnxModel::load(&path)
            .map_err(|_| ModelError::LoadFailed)?;

        // Store the model and metadata
        let name_clone = name.clone();
        self.models.insert(name.clone(), Arc::new(model));
        self.metadata.insert(name, ModelMetadata {
            name: name_clone,
            path,
            inputs,
            output,
        });

        Ok(())
    }

    /// Get a model by name
    pub fn get_model(&self, name: &str) -> Result<Arc<OnnxModel>, ModelError> {
        self.models
            .get(name)
            .cloned()
            .ok_or(ModelError::ModelNotFound)
    }

    /// Get model metadata
    pub fn get_metadata(&self, name: &str) -> Result<&ModelMetadata, ModelError> {
        self.metadata
            .get(name)
            .ok_or(ModelError::ModelNotFound)
    }

    /// Unregister a model
    pub fn unregister_model(&mut self, name: &str) -> Result<(), ModelError> {
        if self.models.remove(name).is_some() {
            self.metadata.remove(name);
            Ok(())
        } else {
            Err(ModelError::ModelNotFound)
        }
    }

    /// List all registered models
    pub fn list_models(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }
}
