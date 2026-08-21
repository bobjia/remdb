//! Worker communication protocol
//! 
//! This module defines the IPC protocol between the main database process
//! and the model worker process.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInput {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOutput {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadataMsg {
    pub name: String,
    pub path: String,
    pub inputs: Vec<ModelInput>,
    pub output: ModelOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelRequest {
    LoadModel {
        name: String,
        path: String,
        inputs: Vec<(String, String)>,
        output: (String, String),
    },
    Execute {
        model_name: String,
        inputs: Vec<Vec<f32>>,
    },
    ExecuteBatch {
        model_name: String,
        inputs: Vec<Vec<f32>>,
    },
    UnloadModel {
        name: String,
    },
    ListModels,
    GetModelInfo {
        name: String,
    },
    Ping,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelResponse {
    Success,
    ModelLoaded {
        metadata: ModelMetadataMsg,
    },
    ExecutionResult {
        output: Vec<f32>,
    },
    BatchExecutionResult {
        outputs: Vec<Vec<f32>>,
    },
    ModelInfo {
        metadata: ModelMetadataMsg,
    },
    ModelList {
        models: Vec<String>,
    },
    Pong,
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    ModelNotFound,
    LoadFailed,
    ExecutionFailed,
    InvalidInput,
    ModelAlreadyExists,
    WorkerError,
    Timeout,
    InternalError,
}

impl From<ErrorCode> for i32 {
    fn from(code: ErrorCode) -> i32 {
        match code {
            ErrorCode::ModelNotFound => 1,
            ErrorCode::LoadFailed => 2,
            ErrorCode::ExecutionFailed => 3,
            ErrorCode::InvalidInput => 4,
            ErrorCode::ModelAlreadyExists => 5,
            ErrorCode::WorkerError => 6,
            ErrorCode::Timeout => 7,
            ErrorCode::InternalError => 8,
        }
    }
}

impl From<i32> for ErrorCode {
    fn from(code: i32) -> Self {
        match code {
            1 => ErrorCode::ModelNotFound,
            2 => ErrorCode::LoadFailed,
            3 => ErrorCode::ExecutionFailed,
            4 => ErrorCode::InvalidInput,
            5 => ErrorCode::ModelAlreadyExists,
            6 => ErrorCode::WorkerError,
            7 => ErrorCode::Timeout,
            _ => ErrorCode::InternalError,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub max_models: usize,
    pub memory_limit_mb: usize,
    pub request_timeout_ms: u64,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_models: 10,
            memory_limit_mb: 2048,
            request_timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub models_loaded: usize,
    pub memory_used_mb: usize,
    pub requests_processed: u64,
    pub uptime_seconds: u64,
}

pub fn serialize_request(request: &ModelRequest) -> Result<Vec<u8>, String> {
    bincode::serialize(request).map_err(|e| format!("Failed to serialize request: {}", e))
}

pub fn deserialize_request(data: &[u8]) -> Result<ModelRequest, String> {
    bincode::deserialize(data).map_err(|e| format!("Failed to deserialize request: {}", e))
}

pub fn serialize_response(response: &ModelResponse) -> Result<Vec<u8>, String> {
    bincode::serialize(response).map_err(|e| format!("Failed to serialize response: {}", e))
}

pub fn deserialize_response(data: &[u8]) -> Result<ModelResponse, String> {
    bincode::deserialize(data).map_err(|e| format!("Failed to deserialize response: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = ModelRequest::Ping;
        let serialized = serialize_request(&request).unwrap();
        let deserialized = deserialize_request(&serialized).unwrap();
        assert!(matches!(deserialized, ModelRequest::Ping));
    }

    #[test]
    fn test_response_serialization() {
        let response = ModelResponse::Pong;
        let serialized = serialize_response(&response).unwrap();
        let deserialized = deserialize_response(&serialized).unwrap();
        assert!(matches!(deserialized, ModelResponse::Pong));
    }

    #[test]
    fn test_execute_request() {
        let request = ModelRequest::Execute {
            model_name: "test_model".to_string(),
            inputs: vec![vec![1.0, 2.0, 3.0]],
        };
        let serialized = serialize_request(&request).unwrap();
        let deserialized = deserialize_request(&serialized).unwrap();
        match deserialized {
            ModelRequest::Execute { model_name, inputs } => {
                assert_eq!(model_name, "test_model");
                assert_eq!(inputs, vec![vec![1.0, 2.0, 3.0]]);
            }
            _ => panic!("Wrong request type"),
        }
    }
}
