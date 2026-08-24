#![allow(static_mut_refs)]
//! Model runtime comprehensive tests
//!
//! Tests for ONNX model loading, execution, worker protocol, and builtin models.

use remdb::model::{ModelError, ModelInfo, ModelManager, ModelUDF, OnnxModel};
use remdb::types::{DataType, TypedValue, Value};

#[cfg(feature = "model-runtime")]
use remdb::model::builtin_models::{get_builtin_model, list_builtin_models, BUILTIN_MODELS};
#[cfg(feature = "model-runtime")]
use remdb::model::worker_protocol::{
    deserialize_request, deserialize_response, serialize_request, serialize_response, ErrorCode,
    ModelRequest, ModelResponse,
};

// Stub model tests (when model-runtime is not enabled, OnnxModel::load returns a stub)
#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_onnx_model_stub_load() {
    let model = OnnxModel::load("test_model.onnx");
    assert!(model.is_ok());

    let model = model.unwrap();
    assert_eq!(model.input_count(), 1);
    assert_eq!(model.output_count(), 1);
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_onnx_model_stub_execute() {
    let model = OnnxModel::load("test_model.onnx").unwrap();

    let inputs = vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]];
    let result = model.execute(&inputs);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(!output.is_empty());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_onnx_model_stub_execute_batch() {
    let model = OnnxModel::load("test_model.onnx").unwrap();

    let inputs = vec![
        vec![1.0, 2.0, 3.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 8.0, 9.0],
    ];
    let result = model.execute_batch(&inputs);

    assert!(result.is_ok());
    let outputs = result.unwrap();
    assert_eq!(outputs.len(), 3);
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_onnx_model_get_info() {
    let model = OnnxModel::load("my_model.onnx").unwrap();

    let info = model.get_info();
    assert_eq!(info.name, "my_model.onnx");
    assert!(!info.input_names.is_empty());
    assert!(!info.output_names.is_empty());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_onnx_model_get_path() {
    let model = OnnxModel::load("/path/to/model.onnx").unwrap();
    assert_eq!(model.get_path(), "/path/to/model.onnx");
}

#[test]
fn test_model_manager_new() {
    let manager = ModelManager::new();
    assert_eq!(manager.model_count(), 0);
    assert!(!manager.is_using_worker());
}

#[test]
fn test_model_manager_with_worker() {
    let manager = ModelManager::with_worker(true);
    assert!(manager.is_using_worker());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_register_model() {
    let mut manager = ModelManager::new();

    let result = manager.register_model(
        "test_model".to_string(),
        "model.onnx".to_string(),
        vec![("text".to_string(), "STRING".to_string())],
        ("embedding".to_string(), "VECTOR(768)".to_string()),
    );

    assert!(result.is_ok());
    assert_eq!(manager.model_count(), 1);
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_register_duplicate() {
    let mut manager = ModelManager::new();

    manager
        .register_model(
            "test_model".to_string(),
            "model.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    let result = manager.register_model(
        "test_model".to_string(),
        "model2.onnx".to_string(),
        vec![("text".to_string(), "STRING".to_string())],
        ("embedding".to_string(), "VECTOR(768)".to_string()),
    );

    assert!(matches!(result, Err(ModelError::ModelAlreadyExists)));
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_get_model() {
    let mut manager = ModelManager::new();

    manager
        .register_model(
            "test_model".to_string(),
            "model.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    let model = manager.get_model("test_model");
    assert!(model.is_ok());

    let not_found = manager.get_model("nonexistent");
    assert!(matches!(not_found, Err(ModelError::ModelNotFound)));
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_get_metadata() {
    let mut manager = ModelManager::new();

    manager
        .register_model(
            "test_model".to_string(),
            "/path/to/model.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    let metadata = manager.get_metadata("test_model").unwrap();
    assert_eq!(metadata.name, "test_model");
    assert_eq!(metadata.path, "/path/to/model.onnx");
    assert_eq!(metadata.inputs.len(), 1);
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_unregister_model() {
    let mut manager = ModelManager::new();

    manager
        .register_model(
            "test_model".to_string(),
            "model.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    assert_eq!(manager.model_count(), 1);

    let result = manager.unregister_model("test_model");
    assert!(result.is_ok());
    assert_eq!(manager.model_count(), 0);

    let not_found = manager.unregister_model("nonexistent");
    assert!(matches!(not_found, Err(ModelError::ModelNotFound)));
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_list_models() {
    let mut manager = ModelManager::new();

    assert!(manager.list_models().is_empty());

    manager
        .register_model(
            "model1".to_string(),
            "model1.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    manager
        .register_model(
            "model2".to_string(),
            "model2.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    let models = manager.list_models();
    assert_eq!(models.len(), 2);
    assert!(models.contains(&"model1".to_string()));
    assert!(models.contains(&"model2".to_string()));
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_manager_clear_all() {
    let mut manager = ModelManager::new();

    manager
        .register_model(
            "model1".to_string(),
            "model1.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    manager
        .register_model(
            "model2".to_string(),
            "model2.onnx".to_string(),
            vec![("text".to_string(), "STRING".to_string())],
            ("embedding".to_string(), "VECTOR(768)".to_string()),
        )
        .unwrap();

    assert_eq!(manager.model_count(), 2);

    manager.clear_all();
    assert_eq!(manager.model_count(), 0);
}

#[cfg(feature = "model-runtime")]
#[test]
fn test_model_request_serialization() {
    let requests = vec![
        ModelRequest::Ping,
        ModelRequest::Shutdown,
        ModelRequest::ListModels,
        ModelRequest::LoadModel {
            name: "test".to_string(),
            path: "/path/to/model.onnx".to_string(),
            inputs: vec![("text".to_string(), "STRING".to_string())],
            output: ("embedding".to_string(), "VECTOR(768)".to_string()),
        },
        ModelRequest::Execute {
            model_name: "test".to_string(),
            inputs: vec![vec![1.0, 2.0, 3.0]],
        },
        ModelRequest::UnloadModel {
            name: "test".to_string(),
        },
    ];

    for request in requests {
        let serialized = serialize_request(&request).unwrap();
        let deserialized = deserialize_request(&serialized).unwrap();
        assert_eq!(request, deserialized);
    }
}

#[cfg(feature = "model-runtime")]
#[test]
fn test_model_response_serialization() {
    let responses = vec![
        ModelResponse::Success,
        ModelResponse::Pong,
        ModelResponse::ModelList {
            models: vec!["model1".to_string(), "model2".to_string()],
        },
        ModelResponse::ExecutionResult {
            output: vec![1.0, 2.0, 3.0],
        },
        ModelResponse::Error {
            code: ErrorCode::ModelNotFound,
            message: "Model not found".to_string(),
        },
    ];

    for response in responses {
        let serialized = serialize_response(&response).unwrap();
        let deserialized = deserialize_response(&serialized).unwrap();
        assert_eq!(response, deserialized);
    }
}

#[cfg(feature = "model-runtime")]
#[test]
fn test_error_code_conversion() {
    let codes = vec![
        ErrorCode::ModelNotFound,
        ErrorCode::LoadFailed,
        ErrorCode::ExecutionFailed,
        ErrorCode::InvalidInput,
        ErrorCode::ModelAlreadyExists,
        ErrorCode::WorkerError,
        ErrorCode::Timeout,
        ErrorCode::InternalError,
    ];

    for code in codes {
        let i: i32 = code.into();
        let back: ErrorCode = i.into();
        assert_eq!(code, back);
    }
}

#[cfg(feature = "model-runtime")]
#[test]
fn test_builtin_models_list() {
    let models = list_builtin_models();
    assert!(!models.is_empty());

    for model in models {
        assert!(!model.name.is_empty());
        assert!(!model.display_name.is_empty());
        assert!(model.dimensions > 0);
        assert!(model.max_input_length > 0);
        assert!(!model.file_name.is_empty());
    }
}

#[cfg(feature = "model-runtime")]
#[test]
fn test_builtin_models_get() {
    let model = get_builtin_model("bge-m3");
    assert!(model.is_some());

    let model = model.unwrap();
    assert_eq!(model.name, "bge-m3");
    assert_eq!(model.dimensions, 1024);
    assert_eq!(model.max_input_length, 8192);
}

#[cfg(feature = "model-runtime")]
#[test]
fn test_builtin_models_get_not_found() {
    let model = get_builtin_model("nonexistent-model");
    assert!(model.is_none());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_udf_new() {
    let onnx_model = OnnxModel::load("test.onnx").unwrap();
    let model_arc = std::sync::Arc::new(onnx_model);

    let udf = ModelUDF::new("test_model".to_string(), model_arc);
    assert_eq!(udf.name(), "test_model");
}

#[test]
fn test_model_udf_new_with_worker() {
    let udf = ModelUDF::new_with_worker("test_model".to_string());
    assert_eq!(udf.name(), "test_model");
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_udf_execute_with_float() {
    let onnx_model = OnnxModel::load("test.onnx").unwrap();
    let model_arc = std::sync::Arc::new(onnx_model);

    let udf = ModelUDF::new("test_model".to_string(), model_arc);

    let arg = TypedValue {
        value_type: DataType::Float32,
        value: Value { float32: 1.0 },
    };

    let result = udf.execute(&[arg]);
    assert!(result.is_ok());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_udf_execute_with_int() {
    let onnx_model = OnnxModel::load("test.onnx").unwrap();
    let model_arc = std::sync::Arc::new(onnx_model);

    let udf = ModelUDF::new("test_model".to_string(), model_arc);

    let arg = TypedValue {
        value_type: DataType::Int32,
        value: Value { i32: 42 },
    };

    let result = udf.execute(&[arg]);
    assert!(result.is_ok());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_udf_execute_with_string() {
    let onnx_model = OnnxModel::load("test.onnx").unwrap();
    let model_arc = std::sync::Arc::new(onnx_model);

    let udf = ModelUDF::new("test_model".to_string(), model_arc);

    let arg = TypedValue {
        value_type: DataType::Text,
        value: Value { string: [0u8; 64] },
    };

    let result = udf.execute(&[arg]);
    assert!(result.is_ok());
}

#[cfg(not(feature = "model-runtime"))]
#[test]
fn test_model_udf_execute_unsupported_type() {
    let onnx_model = OnnxModel::load("test.onnx").unwrap();
    let model_arc = std::sync::Arc::new(onnx_model);

    let udf = ModelUDF::new("test_model".to_string(), model_arc);

    let arg = TypedValue {
        value_type: DataType::Bool,
        value: Value { bool: true },
    };

    let result = udf.execute(&[arg]);
    assert!(result.is_err());
}

#[test]
fn test_model_error_display() {
    assert_eq!(
        format!("{}", ModelError::FileNotFound),
        "Model file not found"
    );
    assert_eq!(
        format!("{}", ModelError::LoadFailed),
        "Failed to load model"
    );
    assert_eq!(
        format!("{}", ModelError::ExecutionFailed),
        "Model execution failed"
    );
    assert_eq!(
        format!("{}", ModelError::InvalidInput),
        "Invalid model input"
    );
    assert_eq!(format!("{}", ModelError::ModelNotFound), "Model not found");
    assert_eq!(
        format!("{}", ModelError::ModelAlreadyExists),
        "Model already exists"
    );
    assert_eq!(
        format!("{}", ModelError::WorkerUnavailable),
        "Model worker unavailable"
    );
    assert_eq!(format!("{}", ModelError::Timeout), "Operation timed out");
    assert_eq!(format!("{}", ModelError::InternalError), "Internal error");
}

#[test]
fn test_model_error_from_string() {
    let error: ModelError = "some error".to_string().into();
    assert!(matches!(error, ModelError::InternalError));
}
