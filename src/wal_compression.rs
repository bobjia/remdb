extern crate alloc;
use crate::config::WALCompressionType;
use crate::types::RemDbError;
use crate::types::Result;
use alloc::vec::Vec;

pub fn compress_wal_data(
    data: &[u8],
    compression_type: WALCompressionType,
    level: u8,
) -> Result<Vec<u8>> {
    match compression_type {
        WALCompressionType::None => Ok(data.to_vec()),
        #[cfg(feature = "wal-compression-lz4")]
        WALCompressionType::LZ4 => compress_lz4(data, level),
        #[cfg(not(feature = "wal-compression-lz4"))]
        WALCompressionType::LZ4 => Err(RemDbError::InvalidConfig(
            "LZ4 compression feature not enabled".to_string(),
        )),
        #[cfg(feature = "wal-compression-zstd")]
        WALCompressionType::ZSTD => compress_zstd(data, level),
        #[cfg(not(feature = "wal-compression-zstd"))]
        WALCompressionType::ZSTD => Err(RemDbError::InvalidConfig(
            "ZSTD compression feature not enabled".to_string(),
        )),
    }
}

pub fn decompress_wal_data(data: &[u8], compression_type: WALCompressionType) -> Result<Vec<u8>> {
    match compression_type {
        WALCompressionType::None => Ok(data.to_vec()),
        #[cfg(feature = "wal-compression-lz4")]
        WALCompressionType::LZ4 => decompress_lz4(data),
        #[cfg(not(feature = "wal-compression-lz4"))]
        WALCompressionType::LZ4 => Err(RemDbError::InvalidConfig(
            "LZ4 compression feature not enabled".to_string(),
        )),
        #[cfg(feature = "wal-compression-zstd")]
        WALCompressionType::ZSTD => decompress_zstd(data),
        #[cfg(not(feature = "wal-compression-zstd"))]
        WALCompressionType::ZSTD => Err(RemDbError::InvalidConfig(
            "ZSTD compression feature not enabled".to_string(),
        )),
    }
}

#[cfg(feature = "wal-compression-lz4")]
fn compress_lz4(data: &[u8], level: u8) -> Result<Vec<u8>> {
    use lz4::EncoderBuilder;
    use std::io::Write;

    let mut encoder = EncoderBuilder::new()
        .level(level as u32)
        .build(Vec::new())
        .map_err(|_| RemDbError::CompressionError)?;

    encoder
        .write_all(data)
        .map_err(|_| RemDbError::CompressionError)?;

    let (compressed, result) = encoder.finish();
    result.map_err(|_| RemDbError::CompressionError)?;

    Ok(compressed)
}

#[cfg(feature = "wal-compression-lz4")]
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    use lz4::Decoder;

    let mut decoder = Decoder::new(data).map_err(|_| RemDbError::CompressionError)?;

    let mut decompressed = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut decompressed)
        .map_err(|_| RemDbError::CompressionError)?;

    Ok(decompressed)
}

#[cfg(feature = "wal-compression-zstd")]
fn compress_zstd(data: &[u8], level: u8) -> Result<Vec<u8>> {
    use zstd::bulk::compress;

    compress(data, level as i32).map_err(|_| RemDbError::CompressionError)
}

#[cfg(feature = "wal-compression-zstd")]
fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    use zstd::bulk::decompress;

    decompress(data, 1024 * 1024).map_err(|_| RemDbError::CompressionError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";
        let compressed = compress_wal_data(data, WALCompressionType::None, 1).unwrap();
        assert_eq!(compressed, data.to_vec());

        let decompressed = decompress_wal_data(&compressed, WALCompressionType::None).unwrap();
        assert_eq!(decompressed, data.to_vec());
    }

    #[cfg(feature = "wal-compression-lz4")]
    #[test]
    fn test_lz4_compression() {
        let data = b"Hello, World! This is a test data for LZ4 compression.";
        let compressed = compress_wal_data(data, WALCompressionType::LZ4, 3).unwrap();

        let decompressed = decompress_wal_data(&compressed, WALCompressionType::LZ4).unwrap();
        assert_eq!(decompressed, data.to_vec());
    }

    #[cfg(feature = "wal-compression-zstd")]
    #[test]
    fn test_zstd_compression() {
        let data = b"Hello, World! This is a test data for ZSTD compression.";
        let compressed = compress_wal_data(data, WALCompressionType::ZSTD, 3).unwrap();

        let decompressed = decompress_wal_data(&compressed, WALCompressionType::ZSTD).unwrap();
        assert_eq!(decompressed, data.to_vec());
    }

    #[test]
    fn test_large_data_compression() {
        let mut data = Vec::new();
        for i in 0u32..10000 {
            data.extend_from_slice(&i.to_le_bytes());
        }

        let compressed = compress_wal_data(&data, WALCompressionType::None, 1).unwrap();
        let decompressed = decompress_wal_data(&compressed, WALCompressionType::None).unwrap();
        assert_eq!(decompressed, data);
    }
}
