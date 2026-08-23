//! Model worker manager
//!
//! This module manages the lifecycle of the model worker process,
//! including spawning, monitoring, and restarting.

use alloc::string::String;
use alloc::vec::Vec;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, FromRawSocket};

use crate::model::worker_protocol::{
    deserialize_response, serialize_request, ErrorCode, ModelRequest, ModelResponse, WorkerConfig,
};
use crate::model::ModelError;

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

const DEFAULT_SOCKET_PATH: &str = "/tmp/remdb_model_worker.sock";
const DEFAULT_NAMED_PIPE: &str = r"\\.\pipe\remdb_model_worker";

#[derive(Debug, Clone)]
pub struct WorkerConfigInternal {
    pub enabled: bool,
    pub cpu_cores: usize,
    pub memory_limit_mb: usize,
    pub max_models: usize,
    pub request_timeout_ms: u64,
    pub restart_on_failure: bool,
    pub max_restart_attempts: u32,
}

impl Default for WorkerConfigInternal {
    fn default() -> Self {
        Self {
            enabled: true,
            cpu_cores: 2,
            memory_limit_mb: 2048,
            max_models: 10,
            request_timeout_ms: 5000,
            restart_on_failure: true,
            max_restart_attempts: 3,
        }
    }
}

pub struct WorkerManager {
    config: WorkerConfigInternal,
    worker_process: Option<Child>,
    restart_attempts: u32,
    #[cfg(unix)]
    socket_path: String,
    #[cfg(windows)]
    pipe_name: String,
}

impl WorkerManager {
    pub fn new(config: WorkerConfigInternal) -> Self {
        Self {
            config,
            worker_process: None,
            restart_attempts: 0,
            #[cfg(unix)]
            socket_path: DEFAULT_SOCKET_PATH.to_string(),
            #[cfg(windows)]
            pipe_name: DEFAULT_NAMED_PIPE.to_string(),
        }
    }

    pub fn spawn_worker(&mut self) -> Result<(), ModelError> {
        if !self.config.enabled {
            #[cfg(feature = "log")]
            info!("Model worker is disabled by config");
            return Ok(());
        }

        #[cfg(feature = "log")]
        info!("Spawning model worker process...");

        let executable = std::env::current_exe().map_err(|_| ModelError::LoadFailed)?;

        let mut cmd = Command::new(&executable);
        cmd.arg("--model-worker");
        cmd.arg("--socket");

        #[cfg(unix)]
        cmd.arg(&self.socket_path);
        #[cfg(windows)]
        cmd.arg(&self.pipe_name);

        cmd.arg("--max-models");
        cmd.arg(self.config.max_models.to_string());

        cmd.arg("--memory-limit");
        cmd.arg(format!("{}m", self.config.memory_limit_mb));

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            #[cfg(feature = "log")]
            error!("Failed to spawn model worker: {}", e);
            ModelError::LoadFailed
        })?;

        self.worker_process = Some(child);
        self.restart_attempts = 0;

        std::thread::sleep(Duration::from_millis(100));

        #[cfg(feature = "log")]
        info!("Model worker spawned successfully");

        Ok(())
    }

    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.worker_process {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    #[cfg(feature = "log")]
                    warn!("Model worker process has exited");
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub fn restart_worker(&mut self) -> Result<(), ModelError> {
        if self.restart_attempts >= self.config.max_restart_attempts {
            #[cfg(feature = "log")]
            error!(
                "Max restart attempts ({}) reached",
                self.config.max_restart_attempts
            );
            return Err(ModelError::LoadFailed);
        }

        self.kill_worker();
        self.restart_attempts += 1;

        #[cfg(feature = "log")]
        info!(
            "Restarting model worker (attempt {}/{})",
            self.restart_attempts, self.config.max_restart_attempts
        );

        self.spawn_worker()
    }

    pub fn kill_worker(&mut self) {
        if let Some(mut child) = self.worker_process.take() {
            let _ = child.kill();
            let _ = child.wait();
            #[cfg(feature = "log")]
            info!("Model worker process killed");
        }
    }

    pub fn send_request(&mut self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        if !self.is_alive() {
            if self.config.restart_on_failure {
                self.restart_worker()?;
            } else {
                return Err(ModelError::LoadFailed);
            }
        }

        let serialized = serialize_request(request).map_err(|_| ModelError::LoadFailed)?;

        #[cfg(unix)]
        {
            self.send_request_unix(&serialized)
        }

        #[cfg(windows)]
        {
            self.send_request_windows(&serialized)
        }
    }

    #[cfg(unix)]
    fn send_request_unix(&mut self, data: &[u8]) -> Result<ModelResponse, ModelError> {
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|e| {
            #[cfg(feature = "log")]
            error!("Failed to connect to worker socket: {}", e);
            ModelError::LoadFailed
        })?;

        let len = data.len() as u32;
        stream
            .write_all(&len.to_be_bytes())
            .map_err(|_| ModelError::LoadFailed)?;
        stream.write_all(data).map_err(|_| ModelError::LoadFailed)?;

        let timeout = Duration::from_millis(self.config.request_timeout_ms);
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|_| ModelError::LoadFailed)?;

        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .map_err(|_| ModelError::LoadFailed)?;
        let response_len = u32::from_be_bytes(len_buf) as usize;

        let mut response_buf = vec![0u8; response_len];
        stream
            .read_exact(&mut response_buf)
            .map_err(|_| ModelError::LoadFailed)?;

        deserialize_response(&response_buf).map_err(|_| ModelError::LoadFailed)
    }

    #[cfg(windows)]
    fn send_request_windows(&mut self, data: &[u8]) -> Result<ModelResponse, ModelError> {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        let mut pipe = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x00000080)
            .open(&self.pipe_name)
            .map_err(|e| {
                #[cfg(feature = "log")]
                error!("Failed to connect to named pipe: {}", e);
                ModelError::LoadFailed
            })?;

        let len = data.len() as u32;
        pipe.write_all(&len.to_be_bytes())
            .map_err(|_| ModelError::LoadFailed)?;
        pipe.write_all(data).map_err(|_| ModelError::LoadFailed)?;

        let mut len_buf = [0u8; 4];
        pipe.read_exact(&mut len_buf)
            .map_err(|_| ModelError::LoadFailed)?;
        let response_len = u32::from_be_bytes(len_buf) as usize;

        let mut response_buf = vec![0u8; response_len];
        pipe.read_exact(&mut response_buf)
            .map_err(|_| ModelError::LoadFailed)?;

        deserialize_response(&response_buf).map_err(|_| ModelError::LoadFailed)
    }

    pub fn get_config(&self) -> &WorkerConfigInternal {
        &self.config
    }
}

impl Drop for WorkerManager {
    fn drop(&mut self) {
        self.kill_worker();
    }
}

lazy_static::lazy_static! {
    pub(crate) static ref GLOBAL_WORKER_MANAGER: Mutex<Option<WorkerManager>> = Mutex::new(None);
}

pub fn init_worker_manager(config: WorkerConfigInternal) -> Result<(), ModelError> {
    let mut manager = GLOBAL_WORKER_MANAGER
        .lock()
        .map_err(|_| ModelError::LoadFailed)?;

    let mut worker = WorkerManager::new(config);
    worker.spawn_worker()?;

    *manager = Some(worker);
    Ok(())
}

pub fn get_worker_manager(
) -> Result<std::sync::MutexGuard<'static, Option<WorkerManager>>, ModelError> {
    GLOBAL_WORKER_MANAGER
        .lock()
        .map_err(|_| ModelError::LoadFailed)
}

pub fn shutdown_worker() -> Result<(), ModelError> {
    let mut manager = GLOBAL_WORKER_MANAGER
        .lock()
        .map_err(|_| ModelError::LoadFailed)?;

    if let Some(ref mut worker) = *manager {
        worker.kill_worker();
    }
    *manager = None;

    Ok(())
}
