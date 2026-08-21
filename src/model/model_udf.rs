//! Model UDF wrapper
//!
//! This module provides a wrapper for using models as user-defined functions (UDFs).

use alloc::string::String;
use alloc::vec::Vec;
use alloc::sync::Arc;

use crate::model::onnx_runtime::OnnxModel;
use crate::types::{DataType, TypedValue, Value};

#[cfg(feature = "model-runtime")]
use crate::model::worker_protocol::{ModelRequest, ModelResponse};

#[cfg(feature = "model-runtime")]
use crate::model::cache::{CacheKey, get_or_init_cache};

#[cfg(feature = "log")]
use crate::log::{debug, error};

#[derive(Debug)]
pub struct ModelUDF {
    name: String,
    model: Option<Arc<OnnxModel>>,
    use_worker: bool,
}

impl ModelUDF {
    pub fn new(name: String, model: Arc<OnnxModel>) -> Self {
        Self {
            name,
            model: Some(model),
            use_worker: false,
        }
    }

    pub fn new_with_worker(name: String) -> Self {
        Self {
            name,
            model: None,
            use_worker: true,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn execute(&self, args: &[TypedValue]) -> core::result::Result<TypedValue, String> {
        #[cfg(feature = "log")]
        debug!("ModelUDF::execute: start for model {}", self.name);

        #[cfg(feature = "model-runtime")]
        if self.use_worker {
            return self.execute_via_worker(args);
        }

        let model = self.model.as_ref()
            .ok_or_else(|| "Model not loaded".to_string())?;

        let mut model_inputs = Vec::new();
        for arg in args {
            #[cfg(feature = "log")]
            debug!("ModelUDF::execute: processing arg with type {:?}", arg.value_type);

            match arg.value_type {
                DataType::VarChar | DataType::Char | DataType::Text => {
                    model_inputs.push(Vec::from_iter((0..768).map(|_| 0.0)));
                }
                DataType::Float32 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.float32]);
                    }
                }
                DataType::Float64 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.float64 as f32]);
                    }
                }
                DataType::Int32 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.i32 as f32]);
                    }
                }
                DataType::Int64 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.i64 as f32]);
                    }
                }
                DataType::UInt32 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.u32 as f32]);
                    }
                }
                DataType::UInt64 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.u64 as f32]);
                    }
                }
                DataType::Vector => {
                    unsafe {
                        if !arg.value.vector.is_null() {
                            let dimension = arg.value.vector_metadata.dimension as usize;
                            let vec_slice = core::slice::from_raw_parts(arg.value.vector, dimension);
                            model_inputs.push(vec_slice.to_vec());
                        } else {
                            model_inputs.push(vec![0.0; 768]);
                        }
                    }
                }
                _ => {
                    return Err("Unsupported input type".to_string());
                }
            }
        }

        #[cfg(feature = "log")]
        debug!("ModelUDF::execute: after processing args, model_inputs len: {}", model_inputs.len());

        #[cfg(feature = "model-runtime")]
        {
            let cache = get_or_init_cache();
            let cache_key = CacheKey::new(self.name.clone(), &model_inputs);
            
            if let Some(_entry) = cache.get(&cache_key) {
                #[cfg(feature = "log")]
                debug!("ModelUDF::execute: cache hit for model {}", self.name);
                
                return Ok(TypedValue {
                    value_type: DataType::Vector,
                    value: Value {
                        vector: core::ptr::null(),
                    },
                });
            }
            
            let output = model.execute(&model_inputs)?;
            
            #[cfg(feature = "log")]
            debug!("ModelUDF::execute: after executing model, output len: {}", output.len());
            
            cache.put(cache_key, output);
            
            #[cfg(feature = "log")]
            debug!("ModelUDF::execute: cache miss, result cached for model {}", self.name);
        }

        #[cfg(not(feature = "model-runtime"))]
        {
            let output = model.execute(&model_inputs)?;
            
            #[cfg(feature = "log")]
            debug!("ModelUDF::execute: after executing model, output len: {}", output.len());
        }

        let typed_value = TypedValue {
            value_type: DataType::Vector,
            value: Value {
                vector: core::ptr::null(),
            },
        };

        #[cfg(feature = "log")]
        debug!("ModelUDF::execute: before returning, typed_value type: {:?}", typed_value.value_type);

        Ok(typed_value)
    }

    #[cfg(feature = "model-runtime")]
    fn execute_via_worker(&self, args: &[TypedValue]) -> core::result::Result<TypedValue, String> {
        use crate::model::worker_manager::get_worker_manager;

        #[cfg(feature = "log")]
        debug!("ModelUDF::execute_via_worker: start for model {}", self.name);

        let mut model_inputs = Vec::new();
        for arg in args {
            match arg.value_type {
                DataType::VarChar | DataType::Char | DataType::Text => {
                    model_inputs.push(Vec::from_iter((0..768).map(|_| 0.0)));
                }
                DataType::Float32 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.float32]);
                    }
                }
                DataType::Float64 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.float64 as f32]);
                    }
                }
                DataType::Int32 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.i32 as f32]);
                    }
                }
                DataType::Int64 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.i64 as f32]);
                    }
                }
                DataType::UInt32 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.u32 as f32]);
                    }
                }
                DataType::UInt64 => {
                    unsafe {
                        model_inputs.push(vec![arg.value.u64 as f32]);
                    }
                }
                DataType::Vector => {
                    unsafe {
                        if !arg.value.vector.is_null() {
                            let dimension = arg.value.vector_metadata.dimension as usize;
                            let vec_slice = core::slice::from_raw_parts(arg.value.vector, dimension);
                            model_inputs.push(vec_slice.to_vec());
                        } else {
                            model_inputs.push(vec![0.0; 768]);
                        }
                    }
                }
                _ => {
                    return Err("Unsupported input type".to_string());
                }
            }
        }

        let cache = get_or_init_cache();
        let cache_key = CacheKey::new(self.name.clone(), &model_inputs);
        
        if let Some(_entry) = cache.get(&cache_key) {
            #[cfg(feature = "log")]
            debug!("ModelUDF::execute_via_worker: cache hit for model {}", self.name);
            
            return Ok(TypedValue {
                value_type: DataType::Vector,
                value: Value {
                    vector: core::ptr::null(),
                },
            });
        }

        let mut manager_guard = get_worker_manager()
            .map_err(|_| "Worker manager unavailable".to_string())?;

        let manager = manager_guard.as_mut()
            .ok_or_else(|| "Worker not initialized".to_string())?;

        let request = ModelRequest::Execute {
            model_name: self.name.clone(),
            inputs: model_inputs,
        };

        let response = manager.send_request(&request)
            .map_err(|e| format!("Worker request failed: {:?}", e))?;

        match response {
            ModelResponse::ExecutionResult { output } => {
                cache.put(cache_key, output);
                
                #[cfg(feature = "log")]
                debug!("ModelUDF::execute_via_worker: cache miss, result cached for model {}", self.name);
                
                Ok(TypedValue {
                    value_type: DataType::Vector,
                    value: Value {
                        vector: core::ptr::null(),
                    },
                })
            }
            ModelResponse::Error { code, message } => {
                Err(format!("Model execution error ({:?}): {}", code, message))
            }
            _ => Err("Unexpected response from worker".to_string()),
        }
    }
}

pub fn execute_model_udf(name: &str, args: &[TypedValue]) -> Result<crate::types::TypedValue, crate::sql::QueryExecutionError> {
    use crate::model::model_manager::get_global_model_manager;

    let model_manager = get_global_model_manager().map_err(|_| crate::sql::QueryExecutionError::InternalError)?;

    if model_manager.is_using_worker() {
        let model_udf = ModelUDF::new_with_worker(name.to_string());
        model_udf.execute(args)
            .map_err(|e| {
                #[cfg(feature = "log")]
                error!("Model UDF execution failed: {}", e);
                crate::sql::QueryExecutionError::InternalError
            })
    } else {
        let model = model_manager.get_model(name)
            .map_err(|_| crate::sql::QueryExecutionError::UnsupportedFunction(name.to_string()))?;

        let model_udf = ModelUDF::new(name.to_string(), model);
        model_udf.execute(args)
            .map_err(|e| {
                #[cfg(feature = "log")]
                error!("Model UDF execution failed: {}", e);
                crate::sql::QueryExecutionError::InternalError
            })
    }
}
