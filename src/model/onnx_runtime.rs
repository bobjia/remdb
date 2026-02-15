//! ONNX runtime wrapper
//! 
//! This module provides a wrapper for loading and executing ONNX models.
//! Supports both real ONNX Runtime (with feature flag) and stub implementation.

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "model-runtime")]
use std::sync::Mutex;

#[cfg(feature = "model-runtime")]
use ort::session::Session;

#[cfg(feature = "model-runtime")]
use ort::value::Tensor;

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub input_names: Vec<String>,
    pub input_shapes: Vec<Vec<Option<usize>>>,
    pub output_names: Vec<String>,
    pub output_shapes: Vec<Vec<Option<usize>>>,
}

#[derive(Debug)]
pub struct OnnxModel {
    #[cfg(feature = "model-runtime")]
    session: Mutex<Session>,
    model_path: String,
    info: ModelInfo,
}

impl OnnxModel {
    #[cfg(feature = "model-runtime")]
    pub fn load(path: &str) -> Result<Self, String> {
        #[cfg(feature = "log")]
        info!("Loading ONNX model from: {}", path);

        let session = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?
            .commit_from_file(path)
            .map_err(|e| format!("Failed to load model from {}: {}", path, e))?;

        let input_names: Vec<String> = session.inputs().iter().map(|i| i.name().to_string()).collect();
        let input_shapes: Vec<Vec<Option<usize>>> = vec![vec![None, Some(768)]];

        let output_names: Vec<String> = session.outputs().iter().map(|o| o.name().to_string()).collect();
        let output_shapes: Vec<Vec<Option<usize>>> = vec![vec![None, Some(768)]];

        let info = ModelInfo {
            name: path.split(|c| c == '/' || c == '\\')
                .last()
                .unwrap_or("unknown")
                .to_string(),
            input_names,
            input_shapes,
            output_names,
            output_shapes,
        };

        #[cfg(feature = "log")]
        info!("Model loaded successfully: {} inputs, {} outputs", 
              info.input_names.len(), info.output_names.len());

        Ok(Self {
            session: Mutex::new(session),
            model_path: path.to_string(),
            info,
        })
    }

    #[cfg(not(feature = "model-runtime"))]
    pub fn load(path: &str) -> Result<Self, String> {
        #[cfg(feature = "log")]
        warn!("model-runtime feature not enabled, using stub implementation");

        let info = ModelInfo {
            name: path.split(|c| c == '/' || c == '\\')
                .last()
                .unwrap_or("unknown")
                .to_string(),
            input_names: vec!["input".to_string()],
            input_shapes: vec![vec![None, Some(768)]],
            output_names: vec!["output".to_string()],
            output_shapes: vec![vec![None, Some(768)]],
        };

        Ok(Self {
            model_path: path.to_string(),
            info,
        })
    }

    #[cfg(feature = "model-runtime")]
    pub fn execute(&self, inputs_data: &[Vec<f32>]) -> Result<Vec<f32>, String> {
        #[cfg(feature = "log")]
        debug!("Executing model with {} input(s)", inputs_data.len());

        if inputs_data.is_empty() {
            return Err("No inputs provided".to_string());
        }

        let input_dim = inputs_data[0].len();
        let input_vec: Vec<f32> = inputs_data[0].clone();
        let shape: [usize; 2] = [1, input_dim];
        let input_tensor = Tensor::from_array((shape, input_vec.into_boxed_slice()))
            .map_err(|e| format!("Failed to create input tensor: {}", e))?;

        let mut session = self.session.lock()
            .map_err(|_| "Failed to lock session".to_string())?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Model execution failed: {}", e))?;

        let output_name = self.info.output_names.first()
            .ok_or("No output names")?;
        
        let first_output = outputs.get(output_name.as_str())
            .ok_or("Failed to get first output")?;
        
        let (_shape, data) = first_output
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output tensor: {}", e))?;

        let result: Vec<f32> = data.to_vec();

        #[cfg(feature = "log")]
        debug!("Model execution complete, output size: {}", result.len());

        Ok(result)
    }

    #[cfg(feature = "model-runtime")]
    pub fn execute_batch(&self, inputs_data: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, String> {
        #[cfg(feature = "log")]
        debug!("Executing model with batch size {}", inputs_data.len());

        if inputs_data.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = inputs_data.len();
        let feature_dim = inputs_data[0].len();

        let mut flat_input: Vec<f32> = Vec::with_capacity(batch_size * feature_dim);
        for input in inputs_data {
            if input.len() != feature_dim {
                return Err(format!(
                    "Inconsistent input dimensions: expected {}, got {}",
                    feature_dim,
                    input.len()
                ));
            }
            flat_input.extend_from_slice(input.as_slice());
        }

        let shape: [usize; 2] = [batch_size, feature_dim];
        let input_tensor = Tensor::from_array((shape, flat_input.into_boxed_slice()))
            .map_err(|e| format!("Failed to create input tensor: {}", e))?;

        let mut session = self.session.lock()
            .map_err(|_| "Failed to lock session".to_string())?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Model execution failed: {}", e))?;

        let output_name = self.info.output_names.first()
            .ok_or("No output names")?;
        
        let first_output = outputs.get(output_name.as_str())
            .ok_or("Failed to get first output")?;
        
        let (out_shape, data) = first_output
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output tensor: {}", e))?;

        let shape_dims: Vec<usize> = out_shape.iter().map(|d| *d as usize).collect();
        
        if shape_dims.len() < 2 {
            let result: Vec<f32> = data.to_vec();
            return Ok(vec![result]);
        }

        let output_dim = shape_dims[shape_dims.len() - 1];
        let mut results = Vec::with_capacity(batch_size);
        let output_vec: Vec<f32> = data.to_vec();

        for i in 0..batch_size {
            let start = i * output_dim;
            let end = start + output_dim;
            results.push(output_vec[start..end].to_vec());
        }

        #[cfg(feature = "log")]
        debug!("Batch execution complete, {} results", results.len());

        Ok(results)
    }

    #[cfg(not(feature = "model-runtime"))]
    pub fn execute(&self, inputs_data: &[Vec<f32>]) -> Result<Vec<f32>, String> {
        #[cfg(feature = "log")]
        debug!("Using stub model execution");

        let output_dim = self.info.output_shapes
            .first()
            .and_then(|shape| shape.last().copied())
            .flatten()
            .unwrap_or(768);

        Ok(vec![0.0; output_dim])
    }

    #[cfg(not(feature = "model-runtime"))]
    pub fn execute_batch(&self, inputs_data: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, String> {
        let output_dim = self.info.output_shapes
            .first()
            .and_then(|shape| shape.last().copied())
            .flatten()
            .unwrap_or(768);

        Ok(inputs_data.iter().map(|_| vec![0.0; output_dim]).collect())
    }

    pub fn input_count(&self) -> usize {
        self.info.input_names.len()
    }

    pub fn output_count(&self) -> usize {
        self.info.output_names.len()
    }

    pub fn get_info(&self) -> &ModelInfo {
        &self.info
    }

    pub fn get_path(&self) -> &str {
        &self.model_path
    }
}

impl Clone for OnnxModel {
    #[cfg(feature = "model-runtime")]
    fn clone(&self) -> Self {
        Self::load(&self.model_path)
            .expect("Failed to clone model by reloading")
    }

    #[cfg(not(feature = "model-runtime"))]
    fn clone(&self) -> Self {
        Self {
            model_path: self.model_path.clone(),
            info: self.info.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_model_load() {
        let model = OnnxModel::load("test.onnx").unwrap();
        assert_eq!(model.input_count(), 1);
        assert_eq!(model.output_count(), 1);
    }

    #[test]
    fn test_stub_model_execute() {
        let model = OnnxModel::load("test.onnx").unwrap();
        let input = vec![0.0; 768];
        let output = model.execute(&[input]).unwrap();
        assert_eq!(output.len(), 768);
    }
}
