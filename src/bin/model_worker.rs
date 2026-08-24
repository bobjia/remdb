#![allow(static_mut_refs)]
//! Model Worker Binary
//!
//! This is the standalone model worker process that handles model loading
//! and inference requests from the main database process.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(windows)]
use std::fs::OpenOptions;

use remdb::model::worker_protocol::{
    deserialize_request, serialize_response, ErrorCode, ModelInput, ModelMetadataMsg, ModelOutput,
    ModelRequest, ModelResponse, WorkerConfig,
};
use remdb::model::OnnxModel;

struct WorkerState {
    models: HashMap<String, Arc<OnnxModel>>,
    metadata: HashMap<String, ModelMetadataMsg>,
    config: WorkerConfig,
    start_time: Instant,
    requests_processed: u64,
}

impl WorkerState {
    fn new(config: WorkerConfig) -> Self {
        Self {
            models: HashMap::new(),
            metadata: HashMap::new(),
            config,
            start_time: Instant::now(),
            requests_processed: 0,
        }
    }

    fn handle_request(&mut self, request: ModelRequest) -> ModelResponse {
        self.requests_processed += 1;

        match request {
            ModelRequest::LoadModel {
                name,
                path,
                inputs,
                output,
            } => self.load_model(name, path, inputs, output),
            ModelRequest::Execute { model_name, inputs } => self.execute_model(&model_name, inputs),
            ModelRequest::ExecuteBatch { model_name, inputs } => {
                self.execute_batch(&model_name, inputs)
            }
            ModelRequest::UnloadModel { name } => self.unload_model(&name),
            ModelRequest::ListModels => ModelResponse::ModelList {
                models: self.models.keys().cloned().collect(),
            },
            ModelRequest::GetModelInfo { name } => self.get_model_info(&name),
            ModelRequest::Ping => ModelResponse::Pong,
            ModelRequest::Shutdown => ModelResponse::Success,
        }
    }

    fn load_model(
        &mut self,
        name: String,
        path: String,
        inputs: Vec<(String, String)>,
        output: (String, String),
    ) -> ModelResponse {
        if self.models.contains_key(&name) {
            return ModelResponse::Error {
                code: ErrorCode::ModelAlreadyExists,
                message: format!("Model '{}' already exists", name),
            };
        }

        if self.models.len() >= self.config.max_models {
            return ModelResponse::Error {
                code: ErrorCode::LoadFailed,
                message: format!(
                    "Maximum number of models ({}) reached",
                    self.config.max_models
                ),
            };
        }

        match OnnxModel::load(&path) {
            Ok(model) => {
                let _info = model.get_info();
                let metadata = ModelMetadataMsg {
                    name: name.clone(),
                    path: path.clone(),
                    inputs: inputs
                        .iter()
                        .map(|(n, t)| ModelInput {
                            name: n.clone(),
                            data_type: t.clone(),
                        })
                        .collect(),
                    output: ModelOutput {
                        name: output.0.clone(),
                        data_type: output.1.clone(),
                    },
                };

                self.models.insert(name.clone(), Arc::new(model));
                self.metadata.insert(name, metadata.clone());

                ModelResponse::ModelLoaded { metadata }
            }
            Err(e) => ModelResponse::Error {
                code: ErrorCode::LoadFailed,
                message: format!("Failed to load model: {}", e),
            },
        }
    }

    fn execute_model(&mut self, model_name: &str, inputs: Vec<Vec<f32>>) -> ModelResponse {
        match self.models.get(model_name) {
            Some(model) => match model.execute(&inputs) {
                Ok(output) => ModelResponse::ExecutionResult { output },
                Err(e) => ModelResponse::Error {
                    code: ErrorCode::ExecutionFailed,
                    message: format!("Model execution failed: {}", e),
                },
            },
            None => ModelResponse::Error {
                code: ErrorCode::ModelNotFound,
                message: format!("Model '{}' not found", model_name),
            },
        }
    }

    fn execute_batch(&mut self, model_name: &str, inputs: Vec<Vec<f32>>) -> ModelResponse {
        match self.models.get(model_name) {
            Some(model) => match model.execute_batch(&inputs) {
                Ok(outputs) => ModelResponse::BatchExecutionResult { outputs },
                Err(e) => ModelResponse::Error {
                    code: ErrorCode::ExecutionFailed,
                    message: format!("Batch execution failed: {}", e),
                },
            },
            None => ModelResponse::Error {
                code: ErrorCode::ModelNotFound,
                message: format!("Model '{}' not found", model_name),
            },
        }
    }

    fn unload_model(&mut self, name: &str) -> ModelResponse {
        if self.models.remove(name).is_some() {
            self.metadata.remove(name);
            ModelResponse::Success
        } else {
            ModelResponse::Error {
                code: ErrorCode::ModelNotFound,
                message: format!("Model '{}' not found", name),
            }
        }
    }

    fn get_model_info(&self, name: &str) -> ModelResponse {
        match self.metadata.get(name) {
            Some(metadata) => ModelResponse::ModelInfo {
                metadata: metadata.clone(),
            },
            None => ModelResponse::Error {
                code: ErrorCode::ModelNotFound,
                message: format!("Model '{}' not found", name),
            },
        }
    }
}

fn parse_args() -> (String, WorkerConfig) {
    let args: Vec<String> = std::env::args().collect();
    let mut socket_path = String::new();
    let mut config = WorkerConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--socket" | "-s" if i + 1 < args.len() => {
                socket_path = args[i + 1].clone();
                i += 1;
            }
            "--max-models" | "-m" if i + 1 < args.len() => {
                if let Ok(v) = args[i + 1].parse() {
                    config.max_models = v;
                }
                i += 1;
            }
            "--memory-limit" if i + 1 < args.len() => {
                let limit_str = &args[i + 1];
                if limit_str.ends_with('m') || limit_str.ends_with('M') {
                    if let Ok(v) = limit_str[..limit_str.len() - 1].parse() {
                        config.memory_limit_mb = v;
                    }
                }
                i += 1;
            }
            "--timeout" | "-t" if i + 1 < args.len() => {
                if let Ok(v) = args[i + 1].parse() {
                    config.request_timeout_ms = v;
                }
                i += 1;
            }
            "--help" | "-h" => {
                println!("Model Worker - Standalone model inference process");
                println!();
                println!("Usage: model_worker [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -s, --socket <PATH>    Socket path for IPC");
                println!("  -m, --max-models <N>   Maximum number of models to load (default: 10)");
                println!("  --memory-limit <MB>    Memory limit in MB (default: 2048)");
                println!("  -t, --timeout <MS>     Request timeout in ms (default: 5000)");
                println!("  -h, --help             Show this help message");
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    (socket_path, config)
}

#[cfg(unix)]
fn run_server(socket_path: &str, config: WorkerConfig) {
    use std::os::unix::net::UnixListener;

    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path).expect("Failed to bind to socket");

    println!("Model worker listening on {}", socket_path);

    let mut state = WorkerState::new(config);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(e) = handle_connection(&mut stream, &mut state) {
                    eprintln!("Connection error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}

#[cfg(unix)]
fn handle_connection(stream: &mut UnixStream, state: &mut WorkerState) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(state.config.request_timeout_ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(state.config.request_timeout_ms)))?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let request_len = u32::from_be_bytes(len_buf) as usize;

    let mut request_buf = vec![0u8; request_len];
    stream.read_exact(&mut request_buf)?;

    let request = deserialize_request(&request_buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let is_shutdown = matches!(request, ModelRequest::Shutdown);

    let response = state.handle_request(request);
    let response_data = serialize_response(&response).map_err(|e| std::io::Error::other(e))?;

    let len = response_data.len() as u32;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&response_data)?;

    if is_shutdown {
        std::process::exit(0);
    }

    Ok(())
}

#[cfg(windows)]
fn run_server(pipe_name: &str, config: WorkerConfig) {
    use std::os::windows::fs::OpenOptionsExt;

    println!("Model worker listening on {}", pipe_name);

    let mut state = WorkerState::new(config);

    loop {
        let pipe = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x00000080)
            .open(pipe_name);

        match pipe {
            Ok(mut pipe) => {
                if let Err(e) = handle_connection_pipe(&mut pipe, &mut state) {
                    eprintln!("Connection error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Pipe open error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[cfg(windows)]
fn handle_connection_pipe(
    pipe: &mut std::fs::File,
    state: &mut WorkerState,
) -> std::io::Result<()> {
    let mut len_buf = [0u8; 4];
    pipe.read_exact(&mut len_buf)?;
    let request_len = u32::from_be_bytes(len_buf) as usize;

    let mut request_buf = vec![0u8; request_len];
    pipe.read_exact(&mut request_buf)?;

    let request = deserialize_request(&request_buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let is_shutdown = matches!(request, ModelRequest::Shutdown);

    let response = state.handle_request(request);
    let response_data = serialize_response(&response)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let len = response_data.len() as u32;
    pipe.write_all(&len.to_be_bytes())?;
    pipe.write_all(&response_data)?;

    if is_shutdown {
        std::process::exit(0);
    }

    Ok(())
}

fn main() {
    let (socket_path, config) = parse_args();

    if socket_path.is_empty() {
        eprintln!("Error: --socket argument is required");
        std::process::exit(1);
    }

    println!("Model Worker starting...");
    println!("  Max models: {}", config.max_models);
    println!("  Memory limit: {}MB", config.memory_limit_mb);
    println!("  Request timeout: {}ms", config.request_timeout_ms);

    #[cfg(unix)]
    run_server(&socket_path, config);

    #[cfg(windows)]
    run_server(&socket_path, config);
}
