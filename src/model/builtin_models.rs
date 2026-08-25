//! Built-in models
//!
//! This module provides built-in embedding models that are available
//! out of the box without additional configuration.

use alloc::string::String;
use alloc::vec::Vec;

use crate::model::{ModelError, ModelManager};

#[cfg(feature = "log")]
use crate::log::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct BuiltinModel {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub dimensions: usize,
    pub max_input_length: usize,
    pub file_name: &'static str,
    pub download_url: Option<&'static str>,
    pub tokenizer_url: Option<&'static str>,
}

pub const BUILTIN_MODELS: &[BuiltinModel] = &[
    BuiltinModel {
        name: "bge-m3",
        display_name: "BGE-M3",
        description: "BAAI General Embedding - Multi-lingual, Multi-functionality, Multi-granularity",
        dimensions: 1024,
        max_input_length: 8192,
        file_name: "bge-m3.onnx",
        download_url: Some("https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx"),
        tokenizer_url: Some("https://huggingface.co/BAAI/bge-m3/resolve/main/tokenizer.json"),
    },
    BuiltinModel {
        name: "bge-small-zh",
        display_name: "BGE-Small-ZH",
        description: "BAAI General Embedding - Small Chinese model",
        dimensions: 512,
        max_input_length: 512,
        file_name: "bge-small-zh.onnx",
        download_url: Some("https://huggingface.co/BAAI/bge-small-zh/resolve/main/onnx/model.onnx"),
        tokenizer_url: Some("https://huggingface.co/BAAI/bge-small-zh/resolve/main/tokenizer.json"),
    },
    BuiltinModel {
        name: "all-minilm-l6-v2",
        display_name: "all-MiniLM-L6-v2",
        description: "Sentence Transformers - Small but powerful English model",
        dimensions: 384,
        max_input_length: 256,
        file_name: "all-minilm-l6-v2.onnx",
        download_url: Some("https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx"),
        tokenizer_url: Some("https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json"),
    },
];

pub fn get_builtin_model(name: &str) -> Option<&'static BuiltinModel> {
    BUILTIN_MODELS.iter().find(|m| m.name == name)
}

pub fn list_builtin_models() -> &'static [BuiltinModel] {
    BUILTIN_MODELS
}

pub fn get_builtin_model_path(model: &BuiltinModel) -> String {
    #[cfg(feature = "std")]
    {
        let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let exe_dir = exe_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        let models_dir = exe_dir.join("models");
        models_dir
            .join(model.file_name)
            .to_string_lossy()
            .to_string()
    }

    #[cfg(not(feature = "std"))]
    {
        format!("models/{}", model.file_name)
    }
}

pub fn register_builtin_models(manager: &mut ModelManager) -> Result<(), ModelError> {
    for model in BUILTIN_MODELS {
        let model_path = get_builtin_model_path(model);

        #[cfg(feature = "log")]
        debug!(
            "Checking for built-in model: {} at {}",
            model.name, model_path
        );

        #[cfg(feature = "std")]
        {
            if !std::path::Path::new(&model_path).exists() {
                #[cfg(feature = "log")]
                info!(
                    "Built-in model {} not found at {}, skipping",
                    model.name, model_path
                );
                continue;
            }
        }

        let result = manager.register_model(
            model.name.to_string(),
            model_path,
            vec![("text".to_string(), "STRING".to_string())],
            (
                "embedding".to_string(),
                format!("VECTOR({})", model.dimensions),
            ),
        );

        match result {
            Ok(()) => {
                #[cfg(feature = "log")]
                info!(
                    "Registered built-in model: {} ({} dimensions)",
                    model.name, model.dimensions
                );
            }
            Err(ModelError::ModelAlreadyExists) => {
                #[cfg(feature = "log")]
                debug!("Built-in model {} already registered", model.name);
            }
            Err(e) => {
                #[cfg(feature = "log")]
                warn!("Failed to register built-in model {}: {:?}", model.name, e);
            }
        }
    }

    Ok(())
}

pub fn text_embedding(_text: &str, model_name: &str) -> Result<Vec<f32>, ModelError> {
    let model = get_builtin_model(model_name).ok_or(ModelError::ModelNotFound)?;

    let model_path = get_builtin_model_path(model);

    let onnx_model = crate::model::OnnxModel::load(&model_path)?;

    let input = vec![0.0f32; model.dimensions];
    let result = onnx_model.execute(&[input])?;

    Ok(result)
}

pub fn get_model_dimensions(model_name: &str) -> Option<usize> {
    get_builtin_model(model_name).map(|m| m.dimensions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_builtin_models() {
        let models = list_builtin_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.name == "bge-m3"));
    }

    #[test]
    fn test_get_builtin_model() {
        let model = get_builtin_model("bge-m3");
        assert!(model.is_some());
        let model = model.unwrap();
        assert_eq!(model.dimensions, 1024);
        assert_eq!(model.max_input_length, 8192);
    }

    #[test]
    fn test_get_builtin_model_not_found() {
        let model = get_builtin_model("non-existent-model");
        assert!(model.is_none());
    }
}
