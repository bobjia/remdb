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
use crate::log::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Float32,
    Int64,
}

#[derive(Debug, Clone)]
pub struct InputSpec {
    pub name: String,
    pub input_type: InputType,
    pub shape: Vec<Option<usize>>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub input_names: Vec<String>,
    pub input_shapes: Vec<Vec<Option<usize>>>,
    pub input_types: Vec<InputType>,
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
        let session = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?
            .commit_from_file(path)
            .map_err(|e| format!("Failed to load model from {}: {}", path, e))?;

        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        let input_count = input_names.len();
        let input_shapes: Vec<Vec<Option<usize>>> =
            (0..input_count).map(|_| vec![None, Some(512)]).collect();

        let input_types: Vec<InputType> = (0..input_count).map(|_| InputType::Float32).collect();

        let output_names: Vec<String> = session
            .outputs()
            .iter()
            .map(|o| o.name().to_string())
            .collect();
        let output_count = output_names.len();
        let output_shapes: Vec<Vec<Option<usize>>> =
            (0..output_count).map(|_| vec![None, Some(512)]).collect();

        let info = ModelInfo {
            name: path
                .split(['/', '\\'])
                .next_back()
                .unwrap_or("unknown")
                .to_string(),
            input_names,
            input_shapes,
            input_types,
            output_names,
            output_shapes,
        };

        #[cfg(feature = "log")]
        info!(
            "Model loaded successfully: {} inputs, {} outputs",
            info.input_names.len(),
            info.output_names.len()
        );

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
            name: path
                .split(|c| c == '/' || c == '\\')
                .last()
                .unwrap_or("unknown")
                .to_string(),
            input_names: vec!["input".to_string()],
            input_shapes: vec![vec![None, Some(768)]],
            input_types: vec![InputType::Float32],
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
        if inputs_data.is_empty() {
            return Err("No inputs provided".to_string());
        }

        let input_dim = inputs_data[0].len();
        let input_vec: Vec<f32> = inputs_data[0].clone();
        let shape: [usize; 2] = [1, input_dim];
        let input_tensor = Tensor::from_array((shape, input_vec.into_boxed_slice()))
            .map_err(|e| format!("Failed to create input tensor: {}", e))?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| "Failed to lock session".to_string())?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Model execution failed: {}", e))?;

        let output_name = self.info.output_names.first().ok_or("No output names")?;

        let first_output = outputs
            .get(output_name.as_str())
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
    pub fn execute_int64(&self, inputs_data: &[Vec<i64>]) -> Result<Vec<f32>, String> {
        if inputs_data.is_empty() {
            return Err("No inputs provided".to_string());
        }

        let mut session = self
            .session
            .lock()
            .map_err(|_| "Failed to lock session".to_string())?;

        let input_count = self.info.input_names.len();

        if input_count == 1 {
            let input_dim = inputs_data[0].len();
            let input_vec: Vec<i64> = inputs_data[0].clone();
            let shape: [usize; 2] = [1, input_dim];
            let input_tensor = Tensor::from_array((shape, input_vec.into_boxed_slice()))
                .map_err(|e| format!("Failed to create input tensor: {}", e))?;

            let outputs = session
                .run(ort::inputs![input_tensor])
                .map_err(|e| format!("Model execution failed: {}", e))?;

            self.extract_sentence_embedding(&outputs)
        } else if input_count >= 3 {
            let _seq_len = inputs_data.first().map(|v| v.len()).unwrap_or(512);

            let input_ids = inputs_data.first().ok_or("Missing input_ids")?;
            let attention_mask = inputs_data.get(1).ok_or("Missing attention_mask")?;
            let token_type_ids = inputs_data.get(2).ok_or("Missing token_type_ids")?;

            let input_ids_tensor =
                Tensor::from_array(([1, input_ids.len()], input_ids.clone().into_boxed_slice()))
                    .map_err(|e| format!("Failed to create input_ids tensor: {}", e))?;

            let attention_mask_tensor = Tensor::from_array((
                [1, attention_mask.len()],
                attention_mask.clone().into_boxed_slice(),
            ))
            .map_err(|e| format!("Failed to create attention_mask tensor: {}", e))?;

            let token_type_ids_tensor = Tensor::from_array((
                [1, token_type_ids.len()],
                token_type_ids.clone().into_boxed_slice(),
            ))
            .map_err(|e| format!("Failed to create token_type_ids tensor: {}", e))?;

            let outputs = session
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor,
                    "token_type_ids" => token_type_ids_tensor,
                ])
                .map_err(|e| format!("Model execution failed: {}", e))?;

            self.extract_sentence_embedding(&outputs)
        } else {
            Err(format!("Unsupported input count: {}", input_count))
        }
    }

    #[cfg(feature = "model-runtime")]
    fn extract_sentence_embedding(
        &self,
        outputs: &ort::session::SessionOutputs,
    ) -> Result<Vec<f32>, String> {
        if let Some(embedding_name) = self
            .info
            .output_names
            .iter()
            .find(|n| n.contains("embedding"))
        {
            let output = outputs
                .get(embedding_name.as_str())
                .ok_or("Failed to get embedding output")?;

            let (_shape, data) = output
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract embedding tensor: {}", e))?;

            return Ok(data.to_vec());
        }

        let output_name = self.info.output_names.first().ok_or("No output names")?;

        let first_output = outputs
            .get(output_name.as_str())
            .ok_or("Failed to get first output")?;

        let (shape, data) = first_output
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output tensor: {}", e))?;

        let shape_dims: Vec<usize> = shape.iter().map(|d| *d as usize).collect();

        if shape_dims.len() == 2 {
            Ok(data.to_vec())
        } else if shape_dims.len() == 3 {
            let embedding_dim = shape_dims[2];
            let mut result = Vec::with_capacity(embedding_dim);
            for i in 0..embedding_dim {
                result.push(data[i]);
            }
            Ok(result)
        } else {
            Ok(data.to_vec())
        }
    }

    #[cfg(feature = "model-runtime")]
    pub fn execute_batch(&self, inputs_data: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, String> {
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

        let mut session = self
            .session
            .lock()
            .map_err(|_| "Failed to lock session".to_string())?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Model execution failed: {}", e))?;

        let output_name = self.info.output_names.first().ok_or("No output names")?;

        let first_output = outputs
            .get(output_name.as_str())
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

    #[cfg(feature = "model-runtime")]
    pub fn execute_int64_batch(
        &self,
        inputs_data: &[Vec<Vec<i64>>],
    ) -> Result<Vec<Vec<f32>>, String> {
        #[cfg(feature = "log")]
        debug!("Executing model with {} int64 batch(es)", inputs_data.len());

        if inputs_data.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = inputs_data.len();
        let mut results = Vec::with_capacity(batch_size);

        for batch_inputs in inputs_data {
            let result = self.execute_int64(batch_inputs)?;
            results.push(result);
        }

        Ok(results)
    }

    pub fn get_input_type(&self, index: usize) -> Option<InputType> {
        self.info.input_types.get(index).copied()
    }

    pub fn is_bert_style(&self) -> bool {
        let has_input_ids = self.info.input_names.iter().any(|n| n == "input_ids");
        let has_attention_mask = self.info.input_names.iter().any(|n| n == "attention_mask");
        has_input_ids && has_attention_mask
    }

    #[cfg(not(feature = "model-runtime"))]
    pub fn execute(&self, _inputs_data: &[Vec<f32>]) -> Result<Vec<f32>, String> {
        #[cfg(feature = "log")]
        debug!("Using stub model execution");

        let output_dim = self
            .info
            .output_shapes
            .first()
            .and_then(|shape| shape.last().copied())
            .flatten()
            .unwrap_or(768);

        Ok(vec![0.0; output_dim])
    }

    #[cfg(not(feature = "model-runtime"))]
    pub fn execute_batch(&self, inputs_data: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, String> {
        let output_dim = self
            .info
            .output_shapes
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

#[cfg(feature = "model-runtime")]
impl Clone for OnnxModel {
    fn clone(&self) -> Self {
        Self::load(&self.model_path).expect("Failed to clone model by reloading")
    }
}

#[cfg(not(feature = "model-runtime"))]
impl Clone for OnnxModel {
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
        let model = match OnnxModel::load("test.onnx") {
            Ok(m) => m,
            Err(_) => {
                // When model-runtime feature is enabled, loading a non-existent file fails
                return;
            }
        };
        assert_eq!(model.input_count(), 1);
        assert_eq!(model.output_count(), 1);
    }

    #[test]
    fn test_stub_model_execute() {
        let model = match OnnxModel::load("test.onnx") {
            Ok(m) => m,
            Err(_) => {
                // When model-runtime feature is enabled, loading a non-existent file fails
                return;
            }
        };
        let input = vec![0.0; 768];
        let output = model.execute(&[input]).unwrap();
        assert_eq!(output.len(), 768);
    }
}
