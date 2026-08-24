//! Model Runtime Examples
//!
//! This file demonstrates how to use the model runtime features in remdb.
//!
//! ## Features
//!
//! - `model-runtime`: Enables ONNX model loading and inference
//! - `model-download`: Enables downloading models from HTTP URLs
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example model_runtime --features model-runtime,model-download
//! ```

#[cfg(feature = "model-runtime")]
use remdb::model::builtin_models::{get_builtin_model, list_builtin_models};
#[cfg(feature = "model-download")]
use remdb::model::downloader::is_url;
#[cfg(feature = "model-runtime")]
use remdb::model::{ModelManager, OnnxModel};

const BGE_SMALL_ZH_MODEL_PATH: &str = "models/bge-small-zh-v1.5.onnx";
const BGE_SMALL_ZH_DIMENSION: usize = 512;

#[cfg(feature = "model-runtime")]
fn main() {
    println!("=== remdb Model Runtime Examples ===\n");

    example_builtin_models();
    example_model_manager();
    example_onnx_model();
    example_bge_small_zh_model();

    #[cfg(feature = "model-download")]
    example_model_download();

    println!("\n=== Examples completed ===");
}

#[cfg(feature = "model-runtime")]
fn example_builtin_models() {
    println!("--- Built-in Models ---");

    let models = list_builtin_models();
    println!(
        "Available built-in models: {}",
        models.iter().map(|m| m.name).collect::<Vec<_>>().join(", ")
    );

    if let Some(bge_m3) = get_builtin_model("bge-m3") {
        println!("\nBGE-M3 Model Info:");
        println!("  Name: {}", bge_m3.display_name);
        println!("  Description: {}", bge_m3.description);
        println!("  Dimensions: {}", bge_m3.dimensions);
        println!("  Max Input Length: {}", bge_m3.max_input_length);
        println!("  File: {}", bge_m3.file_name);
        if let Some(url) = bge_m3.download_url {
            println!("  Download URL: {}", url);
        }
    }

    println!();
}

#[cfg(feature = "model-runtime")]
fn example_model_manager() {
    println!("--- Model Manager ---");

    let mut manager = ModelManager::new();
    println!(
        "Created model manager (worker mode: {})",
        manager.is_using_worker()
    );

    println!("\nRegistering models...");

    match manager.register_model(
        "embedding_model".to_string(),
        "models/embedding.onnx".to_string(),
        vec![("text".to_string(), "STRING".to_string())],
        ("embedding".to_string(), "VECTOR(768)".to_string()),
    ) {
        Ok(()) => println!("  Registered: embedding_model"),
        Err(e) => println!(
            "  Failed to register embedding_model: {} (file not found)",
            e
        ),
    }

    match manager.register_model(
        "rerank_model".to_string(),
        "models/rerank.onnx".to_string(),
        vec![
            ("query".to_string(), "STRING".to_string()),
            ("document".to_string(), "STRING".to_string()),
        ],
        ("score".to_string(), "FLOAT".to_string()),
    ) {
        Ok(()) => println!("  Registered: rerank_model"),
        Err(e) => println!("  Failed to register rerank_model: {} (file not found)", e),
    }

    println!("\nRegistered models: {:?}", manager.list_models());

    if let Ok(metadata) = manager.get_metadata("embedding_model") {
        println!("\nEmbedding model metadata:");
        println!("  Path: {}", metadata.path);
        println!("  Inputs: {:?}", metadata.inputs);
        println!("  Output: {:?}", metadata.output);
    }

    println!("\nUnregistering embedding_model...");
    match manager.unregister_model("embedding_model") {
        Ok(()) => println!("Successfully unregistered embedding_model"),
        Err(e) => println!(
            "Failed to unregister embedding_model: {} (may not have been registered)",
            e
        ),
    }
    println!("Remaining models: {:?}", manager.list_models());

    println!();
}

#[cfg(feature = "model-runtime")]
fn example_onnx_model() {
    println!("--- ONNX Model ---");

    println!("Loading model (stub mode)...");
    match OnnxModel::load("example_model.onnx") {
        Ok(model) => {
            let info = model.get_info();
            println!("Model info:");
            println!("  Name: {}", info.name);
            println!("  Input count: {}", model.input_count());
            println!("  Output count: {}", model.output_count());

            println!("\nExecuting single inference...");
            let input = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
            match model.execute(&[input]) {
                Ok(output) => println!("  Output dimension: {}", output.len()),
                Err(e) => println!("  Failed to execute inference: {}", e),
            }

            println!("\nExecuting batch inference...");
            let batch_inputs = vec![
                vec![1.0f32, 2.0, 3.0],
                vec![4.0f32, 5.0, 6.0],
                vec![7.0f32, 8.0, 9.0],
            ];
            match model.execute_batch(&batch_inputs) {
                Ok(outputs) => println!("  Batch size: {}", outputs.len()),
                Err(e) => println!("  Failed to execute batch inference: {}", e),
            }
        }
        Err(e) => {
            println!(
                "  Failed to load model: {} (example_model.onnx not found)",
                e
            );
            println!("  This is expected - the example model file doesn't exist.");
        }
    }

    println!();
}

#[cfg(feature = "model-runtime")]
fn example_bge_small_zh_model() {
    println!("--- BGE-Small-ZH-v1.5 Model (Real ONNX) ---");

    let model_path = std::path::Path::new(BGE_SMALL_ZH_MODEL_PATH);

    if !model_path.exists() {
        println!(
            "BGE-Small-ZH-v1.5 model file not found at '{}'",
            BGE_SMALL_ZH_MODEL_PATH
        );
        println!("To use this model, download it from:");
        println!("  https://huggingface.co/BAAI/bge-small-zh-v1.5/tree/main/onnx");
        println!("\nUsing stub mode for demonstration...");

        demo_stub_model();
        return;
    }

    println!(
        "Loading BGE-Small-ZH-v1.5 model from '{}'...",
        BGE_SMALL_ZH_MODEL_PATH
    );

    match OnnxModel::load(BGE_SMALL_ZH_MODEL_PATH) {
        Ok(model) => {
            let info = model.get_info();
            println!("Model loaded successfully!");
            println!("  Name: {}", info.name);
            println!("  Input count: {}", model.input_count());
            println!("  Output count: {}", model.output_count());
            println!("  Input names: {:?}", info.input_names);
            println!("  Input types: {:?}", info.input_types);
            println!("  Output names: {:?}", info.output_names);
            println!("  Is BERT-style: {}", model.is_bert_style());

            if model.is_bert_style() {
                println!("\nExecuting BERT-style inference with int64 token IDs...");
                let seq_len = 64;
                let input_ids: Vec<i64> = (0..seq_len as i64).collect();
                let attention_mask: Vec<i64> = vec![1; seq_len];
                let token_type_ids: Vec<i64> = vec![0; seq_len];

                let start = std::time::Instant::now();
                match model.execute_int64(&[input_ids, attention_mask, token_type_ids]) {
                    Ok(output) => {
                        let elapsed = start.elapsed();
                        println!("  Output dimension: {}", output.len());
                        println!("  First 5 values: {:?}", &output[..5.min(output.len())]);
                        println!("  Inference time: {:?}", elapsed);
                    }
                    Err(e) => println!("  Failed to execute inference: {}", e),
                }

                println!("\nExecuting batch inference (batch_size=3)...");
                let _batch_inputs: Vec<Vec<i64>> = (0..3)
                    .flat_map(|i| {
                        let offset = i * 10;
                        vec![
                            (offset..offset + seq_len as i64).collect::<Vec<i64>>(),
                            vec![1; seq_len],
                            vec![0; seq_len],
                        ]
                    })
                    .collect();

                let batch_data: Vec<Vec<Vec<i64>>> = (0..3)
                    .map(|i| {
                        let offset = i * 10;
                        vec![
                            (offset..offset + seq_len as i64).collect(),
                            vec![1; seq_len],
                            vec![0; seq_len],
                        ]
                    })
                    .collect();

                let start = std::time::Instant::now();
                match model.execute_int64_batch(&batch_data) {
                    Ok(outputs) => {
                        let elapsed = start.elapsed();
                        println!("  Batch size: {}", outputs.len());
                        println!(
                            "  Output dimensions: {:?}",
                            outputs.iter().map(|o| o.len()).collect::<Vec<_>>()
                        );
                        println!("  Batch inference time: {:?}", elapsed);
                    }
                    Err(e) => println!("  Failed to execute batch inference: {}", e),
                }
            } else {
                println!(
                    "\nExecuting single inference with {}-dim input...",
                    BGE_SMALL_ZH_DIMENSION
                );
                let input: Vec<f32> = vec![0.1; BGE_SMALL_ZH_DIMENSION];
                let start = std::time::Instant::now();
                match model.execute(&[input]) {
                    Ok(output) => {
                        let elapsed = start.elapsed();
                        println!("  Output dimension: {}", output.len());
                        println!("  First 5 values: {:?}", &output[..5.min(output.len())]);
                        println!("  Inference time: {:?}", elapsed);
                    }
                    Err(e) => println!("  Failed to execute inference: {}", e),
                }
            }

            println!("\nRegistering BGE-Small-ZH in ModelManager...");
            let mut manager = ModelManager::new();
            match manager.register_model(
                "bge-small-zh".to_string(),
                BGE_SMALL_ZH_MODEL_PATH.to_string(),
                vec![("text".to_string(), "STRING".to_string())],
                (
                    "embedding".to_string(),
                    format!("VECTOR({})", BGE_SMALL_ZH_DIMENSION),
                ),
            ) {
                Ok(()) => {
                    println!("  Successfully registered bge-small-zh model");
                    println!("  Registered models: {:?}", manager.list_models());
                }
                Err(e) => println!("  Failed to register: {}", e),
            }
        }
        Err(e) => {
            println!("  Failed to load BGE-Small-ZH-v1.5 model: {}", e);
        }
    }

    println!();
}

#[cfg(feature = "model-runtime")]
fn demo_stub_model() {
    println!("\n--- Stub Model Demo ---");

    match OnnxModel::load("stub_model.onnx") {
        Ok(model) => {
            let info = model.get_info();
            println!("Stub model created:");
            println!("  Name: {}", info.name);
            println!("  Input count: {}", model.input_count());
            println!("  Output count: {}", model.output_count());

            println!("\nExecuting stub inference...");
            let input: Vec<f32> = vec![0.1; 768];
            match model.execute(&[input]) {
                Ok(output) => {
                    println!("  Output dimension: {}", output.len());
                    println!("  (Stub mode returns zeros)");
                }
                Err(e) => println!("  Failed: {}", e),
            }
        }
        Err(e) => println!("Stub model error: {}", e),
    }

    println!();
}

#[cfg(feature = "model-download")]
fn example_model_download() {
    println!("--- Model Download ---");

    let local_path = "models/local.onnx";
    let http_url = "https://example.com/model.onnx";
    let https_url = "https://huggingface.co/model.onnx";

    println!("URL detection:");
    println!("  '{}' is URL: {}", local_path, is_url(local_path));
    println!("  '{}' is URL: {}", http_url, is_url(http_url));
    println!("  '{}' is URL: {}", https_url, is_url(https_url));

    println!("\nNote: Actual download requires network access and valid URLs.");
    println!("Use resolve_model_path() to download and cache models from URLs.");

    println!();
}

#[cfg(not(feature = "model-runtime"))]
fn main() {
    println!("This example requires the 'model-runtime' feature.");
    println!("Run with: cargo run --example model_runtime --features model-runtime,model-download");
}
