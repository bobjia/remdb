//! Embedding engine
//!
//! Provides text embedding capabilities using ONNX models and HuggingFace tokenizers.
//! This module is available behind the `model-runtime` feature flag.

#![cfg(feature = "model-runtime")]

use alloc::string::String;
use alloc::vec::Vec;
use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::log::{info, warn};
use crate::model::OnnxModel;

#[cfg(feature = "model-download")]
use crate::model::builtin_models::get_builtin_model;
#[cfg(feature = "model-download")]
use crate::model::downloader::{download_model_sync, DownloadError};

/// Wrapper around Hugging Face tokenizers for embedding models
pub struct EmbeddingTokenizer {
    tokenizer: Mutex<tokenizers::Tokenizer>,
    /// Maximum sequence length for the model
    pub max_input_length: usize,
    /// Whether this model uses token_type_ids (e.g., BERT-based)
    pub has_token_type_ids: bool,
}

impl EmbeddingTokenizer {
    /// Load a tokenizer from `{models_dir}/{model_name}/tokenizer.json`
    pub fn load(models_dir: &str, model_name: &str) -> Result<Self, String> {
        use std::path::PathBuf;

        let tokenizer_path = PathBuf::from(models_dir)
            .join(model_name)
            .join("tokenizer.json");

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            format!(
                "Failed to load tokenizer from {}: {}",
                tokenizer_path.display(),
                e
            )
        })?;

        // Default max length for BGE models is 512
        let max_input_length = 512;

        // Detect if model uses token_type_ids (BERT-style models do)
        let has_token_type_ids = true;

        Ok(Self {
            tokenizer: Mutex::new(tokenizer),
            max_input_length,
            has_token_type_ids,
        })
    }

    /// Encode a single text, returning (input_ids, attention_mask, token_type_ids).
    /// Truncates to max_length silently.
    pub fn encode(
        &self,
        text: &str,
        max_length: usize,
    ) -> Result<(Vec<i64>, Vec<i64>, Vec<i64>), String> {
        let tokenizer = self
            .tokenizer
            .lock()
            .map_err(|_| "tokenizer lock poisoned".to_string())?;

        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| format!("tokenization failed: {}", e))?;

        let input_ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .map(|&id| id as i64)
            .take(max_length)
            .collect();

        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .take(max_length)
            .collect();

        let token_type_ids: Vec<i64> = if self.has_token_type_ids {
            encoding
                .get_type_ids()
                .iter()
                .map(|&t| t as i64)
                .take(max_length)
                .collect()
        } else {
            vec![0i64; input_ids.len()]
        };

        Ok((input_ids, attention_mask, token_type_ids))
    }

    /// Encode a batch of texts
    pub fn encode_batch(
        &self,
        texts: &[&str],
        max_length: usize,
    ) -> Result<Vec<(Vec<i64>, Vec<i64>, Vec<i64>)>, String> {
        texts
            .iter()
            .map(|text| self.encode(text, max_length))
            .collect()
    }
}

/// A loaded model with its tokenizer
struct ModelEntry {
    model: OnnxModel,
    tokenizer: EmbeddingTokenizer,
    dimension: usize,
}

/// Embedding engine managing model loading, caching, and inference
pub struct EmbeddingEngine {
    /// Map of model_name -> (model, tokenizer, dimension)
    models: Mutex<BTreeMap<String, ModelEntry>>,
    /// Default model name
    default_model: Option<String>,
    /// Directory where models are stored
    models_dir: String,
    /// Maximum number of models to cache
    max_models: usize,
    /// HuggingFace mirror URL
    hf_mirror: Option<String>,
    /// Whether to auto-download models
    auto_download: bool,
}

impl EmbeddingEngine {
    /// Create a new embedding engine
    pub fn new(
        default_model: Option<String>,
        models_dir: String,
        max_models: usize,
        auto_download: bool,
        hf_mirror: Option<String>,
    ) -> Self {
        Self {
            models: Mutex::new(BTreeMap::new()),
            default_model,
            models_dir,
            max_models,
            auto_download,
            hf_mirror,
        }
    }

    /// Pre-load the default model if configured
    pub fn preload_default(&self) -> Result<(), String> {
        if let Some(ref default_model) = self.default_model {
            info!("Pre-loading default embedding model: {}", default_model);
            self.load_model_internal(default_model)?;
            info!("Default embedding model loaded: {}", default_model);
        }
        Ok(())
    }

    /// Apply HuggingFace mirror URL to a download URL
    fn apply_mirror(&self, url: &str) -> String {
        if let Some(ref mirror) = self.hf_mirror {
            // Replace the HuggingFace base URL with the mirror
            if url.starts_with("https://huggingface.co/") {
                return url.replacen("https://huggingface.co", mirror, 1);
            }
        }
        url.to_string()
    }

    /// Try to download a model file and its tokenizer
    #[cfg(feature = "model-download")]
    fn download_model_files(&self, name: &str) -> Result<(), String> {
        use std::path::PathBuf;

        let model_dir = PathBuf::from(&self.models_dir).join(name);
        let model_path = model_dir.join(format!("{}.onnx", name));
        let tokenizer_path = model_dir.join("tokenizer.json");

        // Check if files already exist
        if model_path.exists() && tokenizer_path.exists() {
            return Ok(());
        }

        // Look up the model in built-in models
        let builtin = get_builtin_model(name).ok_or_else(|| {
            format!(
                "Model '{}' not found in built-in models and no local file at {}",
                name,
                model_path.display()
            )
        })?;

        // Get download URLs and apply mirror if configured
        let model_url = builtin
            .download_url
            .ok_or_else(|| format!("No download URL for model '{}'", name))?;
        let tokenizer_url = builtin
            .tokenizer_url
            .ok_or_else(|| format!("No tokenizer URL for model '{}'", name))?;

        let model_url = self.apply_mirror(model_url);
        let tokenizer_url = self.apply_mirror(tokenizer_url);

        // Create the model directory
        std::fs::create_dir_all(&model_dir).map_err(|e| {
            format!(
                "Failed to create model directory '{}': {}",
                model_dir.display(),
                e
            )
        })?;

        // Download the ONNX model file
        info!(
            "Downloading embedding model '{}' from {} ...",
            name, model_url
        );
        download_model_sync(&model_url, &model_path, None::<fn(_)>)
            .map_err(|e| format!("Failed to download model '{}': {}", name, e))?;

        // Download the tokenizer file
        info!(
            "Downloading tokenizer for '{}' from {} ...",
            name, tokenizer_url
        );
        download_model_sync(&tokenizer_url, &tokenizer_path, None::<fn(_)>)
            .map_err(|e| format!("Failed to download tokenizer for '{}': {}", name, e))?;

        info!("Model '{}' downloaded successfully", name);
        Ok(())
    }

    #[cfg(not(feature = "model-download"))]
    fn download_model_files(&self, name: &str) -> Result<(), String> {
        Err(format!(
            "Auto-download is not supported because 'model-download' feature is not enabled. \
             Please manually download model '{}' to {}/{}",
            name, self.models_dir, name
        ))
    }

    /// Load a model (and its tokenizer) from disk, caching it.
    fn load_model_internal(&self, name: &str) -> Result<(), String> {
        use std::path::PathBuf;

        let mut models = self
            .models
            .lock()
            .map_err(|_| "models lock poisoned".to_string())?;

        // Check cache first
        if models.contains_key(name) {
            return Ok(());
        }

        // Evict if over max_models
        if models.len() >= self.max_models {
            if let Some(key) = models.keys().next().cloned() {
                warn!(
                    "Evicting model '{}' from cache (max {})",
                    key, self.max_models
                );
                models.remove(&key);
            }
        }

        // Build model path: {models_dir}/{name}/{name}.onnx
        let model_path = PathBuf::from(&self.models_dir)
            .join(name)
            .join(format!("{}.onnx", name));

        // If the model file doesn't exist and auto-download is enabled, try to download
        if !model_path.exists() {
            if self.auto_download {
                // Drop the lock before downloading to avoid blocking other operations
                drop(models);
                self.download_model_files(name)?;
                // Re-acquire the lock after download
                models = self
                    .models
                    .lock()
                    .map_err(|_| "models lock poisoned".to_string())?;
                // Check again if another thread loaded the model while we were downloading
                if models.contains_key(name) {
                    return Ok(());
                }
            } else {
                return Err(format!(
                    "Model '{}' not found at {}. Set auto_download=true to enable automatic download.",
                    name,
                    model_path.display()
                ));
            }
        }

        let model_path_str = model_path
            .to_str()
            .ok_or_else(|| "invalid model path".to_string())?;

        // Load ONNX model
        let model =
            OnnxModel::load(model_path_str).map_err(|e| format!("load model '{}': {}", name, e))?;

        // Load tokenizer
        let tokenizer = EmbeddingTokenizer::load(&self.models_dir, name)?;

        // Determine embedding dimension from model info
        let dimension = model
            .get_info()
            .output_shapes
            .first()
            .and_then(|shape| shape.last().copied())
            .flatten()
            .unwrap_or(768);

        models.insert(
            name.to_string(),
            ModelEntry {
                model,
                tokenizer,
                dimension,
            },
        );

        Ok(())
    }

    /// Embed a batch of texts, returning a vector of embeddings.
    /// Each embedding is L2-normalized.
    pub fn embed(&self, model_name: &str, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        // Ensure model is loaded
        self.load_model_internal(model_name)?;

        let models = self
            .models
            .lock()
            .map_err(|_| "models lock poisoned".to_string())?;

        let entry = models
            .get(model_name)
            .ok_or_else(|| format!("Model '{}' not found", model_name))?;

        let max_length = entry.tokenizer.max_input_length;

        // Tokenize all texts
        let tokenized = entry.tokenizer.encode_batch(texts, max_length)?;

        // Run inference for each text
        let mut results = Vec::with_capacity(texts.len());
        for (input_ids, attention_mask, token_type_ids) in &tokenized {
            // Use execute_int64 which handles BERT-style multi-input models
            let embedding = entry
                .model
                .execute_int64(&[
                    input_ids.clone(),
                    attention_mask.clone(),
                    token_type_ids.clone(),
                ])
                .map_err(|e| format!("inference failed: {}", e))?;

            // L2-normalize
            let normalized = Self::l2_normalize(&embedding);
            results.push(normalized);
        }

        Ok(results)
    }

    /// L2-normalize a vector
    pub fn l2_normalize(vec: &[f32]) -> Vec<f32> {
        let sum_sq: f32 = vec.iter().map(|&v| v * v).sum();
        if sum_sq <= core::f32::EPSILON {
            return vec.to_vec();
        }
        let norm = sum_sq.sqrt();
        vec.iter().map(|&v| v / norm).collect()
    }

    /// Get the default model name, if any
    pub fn default_model(&self) -> Option<&str> {
        self.default_model.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize_non_zero() {
        let vec = vec![3.0, 4.0];
        let normalized = EmbeddingEngine::l2_normalize(&vec);
        assert!((normalized[0] - 0.6).abs() < 1e-6);
        assert!((normalized[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let vec = vec![0.0, 0.0, 0.0];
        let normalized = EmbeddingEngine::l2_normalize(&vec);
        assert_eq!(normalized, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_l2_normalize_unit_vector() {
        let vec = vec![1.0, 0.0, 0.0];
        let normalized = EmbeddingEngine::l2_normalize(&vec);
        assert!((normalized[0] - 1.0).abs() < 1e-6);
        assert!((normalized[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_embedding_engine_new() {
        let engine = EmbeddingEngine::new(None, "./models".to_string(), 5, false, None);
        assert!(engine.default_model().is_none());
    }

    #[test]
    fn test_embedding_engine_default_model() {
        let engine = EmbeddingEngine::new(
            Some("bge-m3".to_string()),
            "./models".to_string(),
            5,
            false,
            None,
        );
        assert_eq!(engine.default_model(), Some("bge-m3"));
    }
}
