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
use remdb::model::{OnnxModel, ModelManager, ModelUDF};
#[cfg(feature = "model-runtime")]
use remdb::model::builtin_models::{list_builtin_models, get_builtin_model};
#[cfg(feature = "model-download")]
use remdb::model::downloader::{is_url, resolve_model_path};

#[cfg(feature = "model-runtime")]
fn main() {
    println!("=== remdb Model Runtime Examples ===\n");

    example_builtin_models();
    example_model_manager();
    example_onnx_model();
    
    #[cfg(feature = "model-download")]
    example_model_download();
    
    println!("\n=== Examples completed ===");
}

#[cfg(feature = "model-runtime")]
fn example_builtin_models() {
    println!("--- Built-in Models ---");
    
    let models = list_builtin_models();
    println!("Available built-in models: {}",
        models.iter().map(|m| m.name).collect::<Vec<_>>().join(", "));
    
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
    println!("Created model manager (worker mode: {})", manager.is_using_worker());
    
    println!("\nRegistering models...");
    
    match manager.register_model(
        "embedding_model".to_string(),
        "models/embedding.onnx".to_string(),
        vec![("text".to_string(), "STRING".to_string())],
        ("embedding".to_string(), "VECTOR(768)".to_string()),
    ) {
        Ok(()) => println!("  Registered: embedding_model"),
        Err(e) => println!("  Failed to register embedding_model: {} (file not found)", e),
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
        Err(e) => println!("Failed to unregister embedding_model: {} (may not have been registered)", e),
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
            println!("  Failed to load model: {} (example_model.onnx not found)", e);
            println!("  This is expected - the example model file doesn't exist.");
        }
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
