// HA Sync Protocol Definitions
//
// This module defines the message types and structures used for
// startup synchronization between master and slave nodes.

/// Sync type enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SyncType {
    /// Full sync - complete database snapshot
    Full = 0,
    /// Incremental sync - WAL logs since last_log_index
    Incremental = 1,
}

impl From<u8> for SyncType {
    fn from(value: u8) -> Self {
        match value {
            0 => SyncType::Full,
            1 => SyncType::Incremental,
            _ => SyncType::Full,
        }
    }
}

/// Sync request from slave to master
/// Full sync: [slave_id(1), sync_type(1)]
/// Incremental sync: [slave_id(1), sync_type(1), last_log_index(4)]
#[derive(Clone, Copy, Debug)]
pub struct SyncRequest {
    /// Slave node ID
    pub slave_id: u8,
    /// Sync type (full or incremental)
    pub sync_type: SyncType,
    /// Last log index for incremental sync
    pub last_log_index: u32,
}

impl SyncRequest {
    /// Create a new full sync request
    pub fn new_full(slave_id: u8) -> Self {
        Self {
            slave_id,
            sync_type: SyncType::Full,
            last_log_index: 0,
        }
    }

    /// Create a new incremental sync request
    pub fn new_incremental(slave_id: u8, last_log_index: u32) -> Self {
        Self {
            slave_id,
            sync_type: SyncType::Incremental,
            last_log_index,
        }
    }

    /// Encode the request to bytes
    pub fn encode(&self) -> alloc::vec::Vec<u8> {
        let mut data = alloc::vec::Vec::with_capacity(6);
        data.push(self.slave_id);
        data.push(self.sync_type as u8);
        if self.sync_type == SyncType::Incremental {
            data.extend_from_slice(&self.last_log_index.to_le_bytes());
        }
        data
    }

    /// Decode a request from bytes
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }

        let slave_id = data[0];
        let sync_type = SyncType::from(data[1]);

        let last_log_index = if sync_type == SyncType::Incremental {
            if data.len() < 6 {
                return None;
            }
            u32::from_le_bytes([data[2], data[3], data[4], data[5]])
        } else {
            0
        };

        Some(Self {
            slave_id,
            sync_type,
            last_log_index,
        })
    }
}

/// Sync data begin message from master to slave
/// Contains metadata about the sync data to follow
#[derive(Clone, Copy, Debug)]
pub struct SyncDataBegin {
    /// Sync type (snapshot or WAL logs)
    pub sync_type: SyncType,
    /// Total data size in bytes
    pub total_size: u64,
    /// Number of chunks to expect
    pub chunk_count: u32,
    /// Number of tables (for snapshot sync)
    pub table_count: u8,
    /// Number of log items (for WAL sync)
    pub log_count: u32,
}

impl SyncDataBegin {
    /// Create a new sync data begin message for snapshot
    pub fn new_snapshot(total_size: u64, chunk_count: u32, table_count: u8) -> Self {
        Self {
            sync_type: SyncType::Full,
            total_size,
            chunk_count,
            table_count,
            log_count: 0,
        }
    }

    /// Create a new sync data begin message for WAL logs
    pub fn new_wal(total_size: u64, chunk_count: u32, log_count: u32) -> Self {
        Self {
            sync_type: SyncType::Incremental,
            total_size,
            chunk_count,
            table_count: 0,
            log_count,
        }
    }

    /// Encode to bytes
    /// Format: [sync_type(1), total_size(8), chunk_count(4), table_count(1), log_count(4)]
    pub fn encode(&self) -> [u8; 18] {
        let mut data = [0u8; 18];
        data[0] = self.sync_type as u8;
        data[1..9].copy_from_slice(&self.total_size.to_le_bytes());
        data[9..13].copy_from_slice(&self.chunk_count.to_le_bytes());
        data[13] = self.table_count;
        data[14..18].copy_from_slice(&self.log_count.to_le_bytes());
        data
    }

    /// Decode from bytes
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 18 {
            return None;
        }

        Some(Self {
            sync_type: SyncType::from(data[0]),
            total_size: u64::from_le_bytes([
                data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
            ]),
            chunk_count: u32::from_le_bytes([data[9], data[10], data[11], data[12]]),
            table_count: data[13],
            log_count: u32::from_le_bytes([data[14], data[15], data[16], data[17]]),
        })
    }
}

/// Maximum chunk data size (~60KB to fit within UDP packet with some margin)
pub const MAX_CHUNK_DATA_SIZE: usize = 60000;

/// Sync data chunk from master to slave
/// Contains a portion of the sync data
#[derive(Clone, Debug)]
pub struct SyncDataChunk {
    /// Chunk sequence number
    pub chunk_index: u32,
    /// Size of data in this chunk
    pub data_size: u16,
    /// Actual data
    pub data: alloc::vec::Vec<u8>,
}

impl SyncDataChunk {
    /// Create a new sync data chunk
    pub fn new(chunk_index: u32, data: &[u8]) -> Self {
        Self {
            chunk_index,
            data_size: data.len() as u16,
            data: data.to_vec(),
        }
    }

    /// Encode to bytes
    /// Format: [chunk_index(4), data_size(2), data(data_size)]
    pub fn encode(&self) -> alloc::vec::Vec<u8> {
        let mut data = alloc::vec::Vec::with_capacity(6 + self.data.len());
        data.extend_from_slice(&self.chunk_index.to_le_bytes());
        data.extend_from_slice(&self.data_size.to_le_bytes());
        data.extend_from_slice(&self.data);
        data
    }

    /// Decode from bytes
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        let chunk_index = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let data_size = u16::from_le_bytes([data[4], data[5]]) as usize;

        if data.len() < 6 + data_size {
            return None;
        }

        Some(Self {
            chunk_index,
            data_size: data_size as u16,
            data: data[6..6 + data_size].to_vec(),
        })
    }
}

/// Sync data end message from master to slave
/// Marks the end of sync data transmission
#[derive(Clone, Copy, Debug)]
pub struct SyncDataEnd {
    /// Total chunks sent
    pub total_chunks: u32,
    /// CRC32 checksum of all data (optional, 0 if not used)
    pub checksum: u32,
}

impl SyncDataEnd {
    /// Create a new sync data end message
    pub fn new(total_chunks: u32, checksum: u32) -> Self {
        Self {
            total_chunks,
            checksum,
        }
    }

    /// Encode to bytes
    /// Format: [total_chunks(4), checksum(4)]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0u8; 8];
        data[0..4].copy_from_slice(&self.total_chunks.to_le_bytes());
        data[4..8].copy_from_slice(&self.checksum.to_le_bytes());
        data
    }

    /// Decode from bytes
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        Some(Self {
            total_chunks: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            checksum: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        })
    }
}

/// Sync acknowledgment from slave to master
#[derive(Clone, Copy, Debug)]
pub struct SyncAck {
    /// Slave node ID
    pub slave_id: u8,
    /// Success flag
    pub success: bool,
    /// Number of chunks received
    pub chunks_received: u32,
}

impl SyncAck {
    /// Create a new sync acknowledgment
    pub fn new(slave_id: u8, success: bool, chunks_received: u32) -> Self {
        Self {
            slave_id,
            success,
            chunks_received,
        }
    }

    /// Encode to bytes
    /// Format: [slave_id(1), success(1), chunks_received(4)]
    pub fn encode(&self) -> [u8; 6] {
        let mut data = [0u8; 6];
        data[0] = self.slave_id;
        data[1] = if self.success { 1 } else { 0 };
        data[2..6].copy_from_slice(&self.chunks_received.to_le_bytes());
        data
    }

    /// Decode from bytes
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        Some(Self {
            slave_id: data[0],
            success: data[1] != 0,
            chunks_received: u32::from_le_bytes([data[2], data[3], data[4], data[5]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_request_full() {
        let req = SyncRequest::new_full(5);
        let encoded = req.encode();
        let decoded = SyncRequest::decode(&encoded).unwrap();

        assert_eq!(decoded.slave_id, 5);
        assert_eq!(decoded.sync_type, SyncType::Full);
        assert_eq!(decoded.last_log_index, 0);
    }

    #[test]
    fn test_sync_request_incremental() {
        let req = SyncRequest::new_incremental(3, 12345);
        let encoded = req.encode();
        let decoded = SyncRequest::decode(&encoded).unwrap();

        assert_eq!(decoded.slave_id, 3);
        assert_eq!(decoded.sync_type, SyncType::Incremental);
        assert_eq!(decoded.last_log_index, 12345);
    }

    #[test]
    fn test_sync_data_begin() {
        let begin = SyncDataBegin::new_snapshot(1024 * 1024, 20, 5);
        let encoded = begin.encode();
        let decoded = SyncDataBegin::decode(&encoded).unwrap();

        assert_eq!(decoded.sync_type, SyncType::Full);
        assert_eq!(decoded.total_size, 1024 * 1024);
        assert_eq!(decoded.chunk_count, 20);
        assert_eq!(decoded.table_count, 5);
    }

    #[test]
    fn test_sync_data_chunk() {
        let chunk_data = vec![1, 2, 3, 4, 5];
        let chunk = SyncDataChunk::new(10, &chunk_data);
        let encoded = chunk.encode();
        let decoded = SyncDataChunk::decode(&encoded).unwrap();

        assert_eq!(decoded.chunk_index, 10);
        assert_eq!(decoded.data_size, 5);
        assert_eq!(decoded.data, chunk_data);
    }

    #[test]
    fn test_sync_data_end() {
        let end = SyncDataEnd::new(20, 0xDEADBEEF);
        let encoded = end.encode();
        let decoded = SyncDataEnd::decode(&encoded).unwrap();

        assert_eq!(decoded.total_chunks, 20);
        assert_eq!(decoded.checksum, 0xDEADBEEF);
    }

    #[test]
    fn test_sync_ack() {
        let ack = SyncAck::new(5, true, 20);
        let encoded = ack.encode();
        let decoded = SyncAck::decode(&encoded).unwrap();

        assert_eq!(decoded.slave_id, 5);
        assert!(decoded.success);
        assert_eq!(decoded.chunks_received, 20);
    }
}
