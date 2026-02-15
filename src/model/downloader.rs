//! Model downloader
//! 
//! This module provides functionality for downloading models from HTTP URLs.

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "model-download")]
use std::path::Path;

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percentage: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum DownloadError {
    NetworkError(String),
    IoError(String),
    InvalidUrl(String),
    ChecksumMismatch { expected: String, actual: String },
    Cancelled,
}

impl core::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DownloadError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            DownloadError::IoError(msg) => write!(f, "IO error: {}", msg),
            DownloadError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            DownloadError::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected {}, got {}", expected, actual)
            }
            DownloadError::Cancelled => write!(f, "Download cancelled"),
        }
    }
}

#[cfg(feature = "model-download")]
pub fn is_url(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

#[cfg(not(feature = "model-download"))]
pub fn is_url(_path: &str) -> bool {
    false
}

#[cfg(feature = "model-download")]
pub fn get_cache_dir() -> std::path::PathBuf {
    let cache_dir = std::env::var("REMDB_MODEL_CACHE")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            format!("{}/.remdb/models", home)
        });
    
    std::path::PathBuf::from(cache_dir)
}

#[cfg(feature = "model-download")]
pub fn get_cached_model_path(url: &str) -> std::path::PathBuf {
    use sha2::{Sha256, Digest};
    
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    let hash_str = format!("{:x}", hash);
    
    get_cache_dir().join(&hash_str[..16]).with_extension("onnx")
}

#[cfg(feature = "model-download")]
pub async fn download_model(
    url: &str,
    dest: &Path,
    progress_callback: Option<impl Fn(DownloadProgress) + Send + Sync>,
) -> Result<(), DownloadError> {
    use std::io::Write;

    info!("Downloading model from: {}", url);

    let response = reqwest::get(url)
        .await
        .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(DownloadError::NetworkError(format!(
            "HTTP {}: {}",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown error")
        )));
    }

    let total_bytes = response.content_length();
    let mut downloaded_bytes: u64 = 0;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DownloadError::IoError(e.to_string()))?;
    }

    let mut file = std::fs::File::create(dest)
        .map_err(|e| DownloadError::IoError(e.to_string()))?;

    let mut stream = response.bytes_stream();
    use futures::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DownloadError::NetworkError(e.to_string()))?;
        
        file.write_all(&chunk)
            .map_err(|e| DownloadError::IoError(e.to_string()))?;
        
        downloaded_bytes += chunk.len() as u64;

        if let Some(ref callback) = progress_callback {
            let percentage = total_bytes.map(|total| (downloaded_bytes as f32 / total as f32) * 100.0);
            callback(DownloadProgress {
                downloaded_bytes,
                total_bytes,
                percentage,
            });
        }
    }

    info!("Model downloaded successfully to: {:?}", dest);
    Ok(())
}

#[cfg(feature = "model-download")]
pub fn download_model_sync(
    url: &str,
    dest: &Path,
    progress_callback: Option<impl Fn(DownloadProgress) + Send + Sync>,
) -> Result<(), DownloadError> {
    use std::io::{Read, Write};

    info!("Downloading model from: {}", url);

    let response = ureq::get(url)
        .call()
        .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

    let status = response.status();
    if status != 200 {
        let status_text = response.status_text();
        return Err(DownloadError::NetworkError(format!(
            "HTTP {}: {}",
            status,
            status_text
        )));
    }

    let total_bytes = response.header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok());
    let mut downloaded_bytes: u64 = 0;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DownloadError::IoError(e.to_string()))?;
    }

    let mut file = std::fs::File::create(dest)
        .map_err(|e| DownloadError::IoError(e.to_string()))?;

    let mut reader = response.into_reader();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)
            .map_err(|e| DownloadError::IoError(e.to_string()))?;
        
        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read])
            .map_err(|e| DownloadError::IoError(e.to_string()))?;
        
        downloaded_bytes += bytes_read as u64;

        if let Some(ref callback) = progress_callback {
            let percentage = total_bytes.map(|total| (downloaded_bytes as f32 / total as f32) * 100.0);
            callback(DownloadProgress {
                downloaded_bytes,
                total_bytes,
                percentage,
            });
        }
    }

    info!("Model downloaded successfully to: {:?}", dest);
    Ok(())
}

#[cfg(feature = "model-download")]
pub fn resolve_model_path(path: &str) -> Result<String, DownloadError> {
    if !is_url(path) {
        return Ok(path.to_string());
    }

    let cached_path = get_cached_model_path(path);
    
    if cached_path.exists() {
        debug!("Using cached model: {:?}", cached_path);
        return Ok(cached_path.to_string_lossy().to_string());
    }

    debug!("Downloading model to cache: {:?}", cached_path);
    download_model_sync(path, &cached_path, None::<fn(DownloadProgress)>)?;
    
    Ok(cached_path.to_string_lossy().to_string())
}

#[cfg(not(feature = "model-download"))]
pub fn resolve_model_path(path: &str) -> Result<String, DownloadError> {
    if is_url(path) {
        return Err(DownloadError::NetworkError(
            "model-download feature not enabled".to_string()
        ));
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_url() {
        assert!(is_url("http://example.com/model.onnx"));
        assert!(is_url("https://example.com/model.onnx"));
        assert!(!is_url("/path/to/model.onnx"));
        assert!(!is_url("model.onnx"));
    }
}
