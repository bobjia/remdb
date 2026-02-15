//! Model manager
//! 
//! This module manages the lifecycle of AI models, including loading, unloading, and caching.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;

use crate::model::onnx_runtime::OnnxModel;

#[cfg(feature = "model-runtime")]
use crate::model::worker_protocol::{ModelRequest, ModelResponse, ErrorCode};

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum ModelError {
    FileNotFound,
    LoadFailed,
    ExecutionFailed,
    InvalidInput,
    ModelNotFound,
    ModelAlreadyExists,
    WorkerUnavailable,
    Timeout,
    InternalError,
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
            ModelError::WorkerUnavailable => write!(f, "Model worker unavailable"),
            ModelError::Timeout => write!(f, "Operation timed out"),
            ModelError::InternalError => write!(f, "Internal error"),
        }
    }
}

impl core::error::Error for ModelError {}

impl From<String> for ModelError {
    fn from(_: String) -> Self {
        ModelError::InternalError
    }
}

#[derive(Debug, Clone)]
pub struct ModelMetadata {
    pub name: String,
    pub path: String,
    pub inputs: Vec<(String, String)>,
    pub output: (String, String),
}

use lazy_static::lazy_static;
use std::sync::Mutex;

#[derive(Debug)]
pub struct ModelManager {
    models: BTreeMap<String, Arc<OnnxModel>>,
    metadata: BTreeMap<String, ModelMetadata>,
    use_worker: bool,
}

lazy_static! {
    pub(crate) static ref GLOBAL_MODEL_MANAGER: Mutex<ModelManager> = Mutex::new(ModelManager::new());
}

pub fn get_global_model_manager() -> Result<std::sync::MutexGuard<'static, ModelManager>, ModelError> {
    GLOBAL_MODEL_MANAGER.lock().map_err(|_| ModelError::InternalError)
}

pub fn reset_global_model_manager() -> Result<(), ModelError> {
    let mut model_manager = get_global_model_manager()?;
    model_manager.clear_all();
    Ok(())
}

impl Default for ModelManager {
    fn default() -> Self {
        Self {
            models: BTreeMap::new(),
            metadata: BTreeMap::new(),
            use_worker: false,
        }
    }
}

impl ModelManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_worker(use_worker: bool) -> Self {
        Self {
            models: BTreeMap::new(),
            metadata: BTreeMap::new(),
            use_worker,
        }
    }

    pub fn register_model(
        &mut self,
        name: String,
        path: String,
        inputs: Vec<(String, String)>,
        output: (String, String),
    ) -> Result<(), ModelError> {
        if self.models.contains_key(&name) {
            return Err(ModelError::ModelAlreadyExists);
        }

        #[cfg(feature = "model-runtime")]
        if self.use_worker {
            return self.register_model_via_worker(&name, &path, &inputs, &output);
        }

        #[cfg(feature = "log")]
        info!("Registering model: {} from {}", name, path);

        let model = OnnxModel::load(&path)
            .map_err(|e| {
                #[cfg(feature = "log")]
                error!("Failed to load model {}: {}", name, e);
                ModelError::LoadFailed
            })?;

        let name_clone = name.clone();
        self.models.insert(name.clone(), Arc::new(model));
        self.metadata.insert(name, ModelMetadata {
            name: name_clone.clone(),
            path,
            inputs,
            output,
        });

        #[cfg(feature = "log")]
        info!("Model {} registered successfully", name_clone);

        Ok(())
    }

    #[cfg(feature = "model-runtime")]
    fn register_model_via_worker(
        &mut self,
        name: &str,
        path: &str,
        inputs: &[(String, String)],
        output: &(String, String),
    ) -> Result<(), ModelError> {
        use crate::model::worker_manager::get_worker_manager;

        let mut manager_guard = get_worker_manager()
            .map_err(|_| ModelError::WorkerUnavailable)?;

        let manager = manager_guard.as_mut()
            .ok_or(ModelError::WorkerUnavailable)?;

        let request = ModelRequest::LoadModel {
            name: name.to_string(),
            path: path.to_string(),
            inputs: inputs.to_vec(),
            output: output.clone(),
        };

        let response = manager.send_request(&request)
            .map_err(|_| ModelError::WorkerUnavailable)?;

        match response {
            ModelResponse::ModelLoaded { metadata: _ } => {
                self.metadata.insert(name.to_string(), ModelMetadata {
                    name: name.to_string(),
                    path: path.to_string(),
                    inputs: inputs.to_vec(),
                    output: output.clone(),
                });
                Ok(())
            }
            ModelResponse::Error { code, message } => {
                #[cfg(feature = "log")]
                error!("Failed to register model via worker: {} ({:?})", message, code);
                Err(match code {
                    ErrorCode::ModelAlreadyExists => ModelError::ModelAlreadyExists,
                    ErrorCode::LoadFailed => ModelError::LoadFailed,
                    _ => ModelError::InternalError,
                })
            }
            _ => Err(ModelError::InternalError),
        }
    }

    pub fn get_model(&self, name: &str) -> Result<Arc<OnnxModel>, ModelError> {
        self.models
            .get(name)
            .cloned()
            .ok_or(ModelError::ModelNotFound)
    }

    pub fn get_metadata(&self, name: &str) -> Result<&ModelMetadata, ModelError> {
        self.metadata
            .get(name)
            .ok_or(ModelError::ModelNotFound)
    }

    pub fn unregister_model(&mut self, name: &str) -> Result<(), ModelError> {
        #[cfg(feature = "model-runtime")]
        if self.use_worker {
            return self.unregister_model_via_worker(name);
        }

        if self.models.remove(name).is_some() {
            self.metadata.remove(name);
            Ok(())
        } else {
            Err(ModelError::ModelNotFound)
        }
    }

    #[cfg(feature = "model-runtime")]
    fn unregister_model_via_worker(&mut self, name: &str) -> Result<(), ModelError> {
        use crate::model::worker_manager::get_worker_manager;

        let mut manager_guard = get_worker_manager()
            .map_err(|_| ModelError::WorkerUnavailable)?;

        let manager = manager_guard.as_mut()
            .ok_or(ModelError::WorkerUnavailable)?;

        let request = ModelRequest::UnloadModel {
            name: name.to_string(),
        };

        let response = manager.send_request(&request)
            .map_err(|_| ModelError::WorkerUnavailable)?;

        match response {
            ModelResponse::Success => {
                self.metadata.remove(name);
                Ok(())
            }
            ModelResponse::Error { code, .. } => {
                Err(match code {
                    ErrorCode::ModelNotFound => ModelError::ModelNotFound,
                    _ => ModelError::InternalError,
                })
            }
            _ => Err(ModelError::InternalError),
        }
    }

    pub fn list_models(&self) -> Vec<String> {
        self.metadata.keys().cloned().collect()
    }

    pub fn clear_all(&mut self) {
        self.models.clear();
        self.metadata.clear();
    }

    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    pub fn is_using_worker(&self) -> bool {
        self.use_worker
    }
}
