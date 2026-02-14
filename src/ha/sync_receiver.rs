// Slave-Side Sync Receiver
//
// Receives sync data from master and applies it to the local database.

use crate::ha::protocol::{SyncAck, SyncDataBegin, SyncDataChunk, SyncDataEnd, SyncType};
use crate::ha::{HAError, Result, SyncState};
use crate::pubsub;
use crate::pubsub::topics::{
    SYNC_ACK_TOPIC, SYNC_DATA_BEGIN_TOPIC, SYNC_DATA_CHUNK_TOPIC, SYNC_DATA_END_TOPIC,
};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

/// Global sync state for callback access
static SYNC_RECEIVER_STATE: AtomicU32 = AtomicU32::new(SyncState::Idle as u32);

/// Global accumulated data for callback access
static mut SYNC_ACCUMULATED_DATA: Option<Vec<u8>> = None;
static mut SYNC_EXPECTED_CHUNKS: u32 = 0;
static mut SYNC_RECEIVED_CHUNKS: u32 = 0;
static mut SYNC_BEGIN_INFO: Option<SyncDataBegin> = None;

/// Slave-side sync receiver
pub struct SyncReceiver {
    /// Current sync state
    state: SyncState,
    /// Slave ID
    slave_id: u8,
    /// Expected chunk count
    expected_chunks: u32,
    /// Received chunk count
    received_chunks: u32,
    /// Accumulated data buffer
    accumulated_data: Vec<u8>,
    /// Sync begin info
    sync_begin_info: Option<SyncDataBegin>,
    /// Lock for thread safety
    lock: u32,
}

impl SyncReceiver {
    /// Create a new sync receiver
    pub fn new(slave_id: u8) -> Self {
        Self {
            state: SyncState::Idle,
            slave_id,
            expected_chunks: 0,
            received_chunks: 0,
            accumulated_data: Vec::new(),
            sync_begin_info: None,
            lock: 0,
        }
    }

    /// Initialize the sync receiver (subscribe to sync data topics)
    pub fn init(&mut self) -> Result<()> {
        #[cfg(feature = "log")]
        debug!("SyncReceiver: Initializing and subscribing to sync data topics");

        // Subscribe to sync data topics
        pubsub::subscribe(SYNC_DATA_BEGIN_TOPIC, Self::handle_sync_begin_callback)
            .map_err(|_| HAError::InitFailed)?;

        pubsub::subscribe(SYNC_DATA_CHUNK_TOPIC, Self::handle_sync_chunk_callback)
            .map_err(|_| HAError::InitFailed)?;

        pubsub::subscribe(SYNC_DATA_END_TOPIC, Self::handle_sync_end_callback)
            .map_err(|_| HAError::InitFailed)?;

        #[cfg(feature = "log")]
        info!("SyncReceiver: Successfully initialized");

        Ok(())
    }

    /// Start receiving sync data
    pub fn start_sync(&mut self) -> Result<()> {
        self.state = SyncState::Syncing;
        self.expected_chunks = 0;
        self.received_chunks = 0;
        self.accumulated_data.clear();
        self.sync_begin_info = None;

        SYNC_RECEIVER_STATE.store(SyncState::Syncing as u32, Ordering::SeqCst);

        // Reset global state
        unsafe {
            SYNC_ACCUMULATED_DATA = Some(Vec::new());
            SYNC_EXPECTED_CHUNKS = 0;
            SYNC_RECEIVED_CHUNKS = 0;
            SYNC_BEGIN_INFO = None;
        }

        #[cfg(feature = "log")]
        debug!("SyncReceiver: Started sync, waiting for data");

        Ok(())
    }

    /// Wait for sync to complete with timeout
    pub fn wait_for_completion(&mut self, timeout_ms: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            let current_state = SYNC_RECEIVER_STATE.load(Ordering::SeqCst);

            match SyncState::from(current_state) {
                SyncState::Synced => {
                    #[cfg(feature = "log")]
                    info!("SyncReceiver: Sync completed successfully");
                    self.state = SyncState::Synced;
                    return Ok(());
                }
                SyncState::Failed => {
                    #[cfg(feature = "log")]
                    error!("SyncReceiver: Sync failed");
                    self.state = SyncState::Failed;
                    return Err(HAError::SyncFailed);
                }
                SyncState::Syncing => {
                    // Continue waiting
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                SyncState::Idle => {
                    // Should not happen during sync
                    break;
                }
            }
        }

        #[cfg(feature = "log")]
        error!("SyncReceiver: Sync timed out after {}ms", timeout_ms);
        self.state = SyncState::Failed;
        Err(HAError::SyncFailed)
    }

    /// Callback for handling sync begin messages
    fn handle_sync_begin_callback(topic_id: u16, data: &[u8]) -> bool {
        if topic_id != SYNC_DATA_BEGIN_TOPIC {
            return false;
        }

        #[cfg(feature = "log")]
        debug!("SyncReceiver: Received SYNC_DATA_BEGIN, data len: {}", data.len());

        let begin = match SyncDataBegin::decode(data) {
            Some(b) => b,
            None => {
                #[cfg(feature = "log")]
                error!("SyncReceiver: Failed to decode SYNC_DATA_BEGIN");
                SYNC_RECEIVER_STATE.store(SyncState::Failed as u32, Ordering::SeqCst);
                return false;
            }
        };

        #[cfg(feature = "log")]
        info!(
            "SyncReceiver: Sync begin - type: {:?}, total_size: {}, chunks: {}, tables: {}, logs: {}",
            begin.sync_type,
            begin.total_size,
            begin.chunk_count,
            begin.table_count,
            begin.log_count
        );

        // Store begin info and prepare for data
        unsafe {
            SYNC_BEGIN_INFO = Some(begin);
            SYNC_EXPECTED_CHUNKS = begin.chunk_count;
            SYNC_RECEIVED_CHUNKS = 0;
            if let Some(ref mut acc) = SYNC_ACCUMULATED_DATA {
                acc.reserve(begin.total_size as usize);
            }
        }

        true
    }

    /// Callback for handling sync chunk messages
    fn handle_sync_chunk_callback(topic_id: u16, data: &[u8]) -> bool {
        if topic_id != SYNC_DATA_CHUNK_TOPIC {
            return false;
        }

        let chunk = match SyncDataChunk::decode(data) {
            Some(c) => c,
            None => {
                #[cfg(feature = "log")]
                error!("SyncReceiver: Failed to decode SYNC_DATA_CHUNK");
                SYNC_RECEIVER_STATE.store(SyncState::Failed as u32, Ordering::SeqCst);
                return false;
            }
        };

        #[cfg(feature = "log")]
        debug!(
            "SyncReceiver: Received chunk {}/{}, size: {}",
            chunk.chunk_index + 1,
            unsafe { SYNC_EXPECTED_CHUNKS },
            chunk.data_size
        );

        // Accumulate data
        unsafe {
            if let Some(ref mut acc) = SYNC_ACCUMULATED_DATA {
                acc.extend_from_slice(&chunk.data);
            }
            SYNC_RECEIVED_CHUNKS += 1;
        }

        true
    }

    /// Callback for handling sync end messages
    fn handle_sync_end_callback(topic_id: u16, data: &[u8]) -> bool {
        if topic_id != SYNC_DATA_END_TOPIC {
            return false;
        }

        #[cfg(feature = "log")]
        debug!("SyncReceiver: Received SYNC_DATA_END");

        let end = match SyncDataEnd::decode(data) {
            Some(e) => e,
            None => {
                #[cfg(feature = "log")]
                error!("SyncReceiver: Failed to decode SYNC_DATA_END");
                SYNC_RECEIVER_STATE.store(SyncState::Failed as u32, Ordering::SeqCst);
                return false;
            }
        };

        let expected = unsafe { SYNC_EXPECTED_CHUNKS };
        let received = unsafe { SYNC_RECEIVED_CHUNKS };

        #[cfg(feature = "log")]
        info!(
            "SyncReceiver: Sync end - total_chunks: {}, checksum: {}, received: {}",
            end.total_chunks, end.checksum, received
        );

        // Verify chunk count
        if received != expected || end.total_chunks != expected {
            #[cfg(feature = "log")]
            error!(
                "SyncReceiver: Chunk count mismatch - expected: {}, received: {}, reported: {}",
                expected, received, end.total_chunks
            );
            SYNC_RECEIVER_STATE.store(SyncState::Failed as u32, Ordering::SeqCst);
            return false;
        }

        // Apply the accumulated data
        let sync_data = unsafe { SYNC_ACCUMULATED_DATA.take().unwrap_or_default() };
        let begin_info = unsafe { SYNC_BEGIN_INFO.take() };

        if let Some(begin) = begin_info {
            let result = match begin.sync_type {
                SyncType::Full => Self::apply_snapshot(&sync_data),
                SyncType::Incremental => Self::apply_wal_logs(&sync_data),
            };

            match result {
                Ok(_) => {
                    #[cfg(feature = "log")]
                    info!("SyncReceiver: Successfully applied sync data");

                    // Send acknowledgment
                    Self::send_ack(true, received);

                    SYNC_RECEIVER_STATE.store(SyncState::Synced as u32, Ordering::SeqCst);
                }
                Err(e) => {
                    #[cfg(feature = "log")]
                    error!("SyncReceiver: Failed to apply sync data: {:?}", e);

                    // Send negative acknowledgment
                    Self::send_ack(false, received);

                    SYNC_RECEIVER_STATE.store(SyncState::Failed as u32, Ordering::SeqCst);
                }
            }
        } else {
            #[cfg(feature = "log")]
            error!("SyncReceiver: No sync begin info available");
            SYNC_RECEIVER_STATE.store(SyncState::Failed as u32, Ordering::SeqCst);
        }

        true
    }

    /// Apply snapshot data to local database
    fn apply_snapshot(data: &[u8]) -> Result<()> {
        if data.is_empty() {
            #[cfg(feature = "log")]
            warn!("SyncReceiver: Empty snapshot data");
            return Ok(());
        }

        #[cfg(feature = "log")]
        info!("SyncReceiver: Applying snapshot, size: {} bytes", data.len());

        let db = unsafe { crate::get_global_db() }.ok_or(HAError::SyncFailed)?;

        unsafe {
            let mut offset = 0;

            // Read table count
            let table_count = data[offset] as usize;
            offset += 1;

            #[cfg(feature = "log")]
            debug!("SyncReceiver: Processing {} tables", table_count);

            for _ in 0..table_count {
                // Read table name
                let name_len = data[offset] as usize;
                offset += 1;
                let table_name = core::str::from_utf8(&data[offset..offset + name_len])
                    .map_err(|_| HAError::SyncFailed)?;
                offset += name_len;

                // Read record size
                let record_size = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;

                // Read record count
                let _record_count = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;

                // Read max records
                let _max_records = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;

                // Read field count
                let field_count = data[offset] as usize;
                offset += 1;

                // Skip field definitions for now
                for _ in 0..field_count {
                    let field_name_len = data[offset] as usize;
                    offset += 1 + field_name_len; // name
                    offset += 1; // data type
                    offset += 2; // offset
                    offset += 2; // dimension
                }

                // Read primary key count
                let pk_count = data[offset] as usize;
                offset += 1;

                // Skip primary key indices
                offset += pk_count;

                #[cfg(feature = "log")]
                debug!(
                    "SyncReceiver: Processing table '{}' (record_size: {})",
                    table_name, record_size
                );

                // Find the table in the local database
                let table_id = db.tables.iter().position(|t| {
                    t.as_ref()
                        .map(|tbl| tbl.def.name == table_name)
                        .unwrap_or(false)
                });

                if let Some(tid) = table_id {
                    if let Some(table) = &mut db.tables[tid] {
                        // Read records
                        loop {
                            let used_flag = data[offset];
                            offset += 1;

                            if used_flag == 0 {
                                // End of records marker
                                break;
                            }

                            // Read record ID
                            let record_id = u32::from_le_bytes([
                                data[offset],
                                data[offset + 1],
                                data[offset + 2],
                                data[offset + 3],
                            ]) as usize;
                            offset += 4;

                            // Read record data
                            let record_data = &data[offset..offset + record_size];
                            offset += record_size;

                            // Apply record to table
                            if record_id < table.def.max_records {
                                let record_ptr = table.get_record_ptr_mut(record_id);
                                crate::platform::memcpy(record_ptr, record_data.as_ptr(), record_size);

                                // Update status
                                let status_ptr = table.get_status_ptr(record_id);
                                if (*status_ptr).status != crate::types::RecordStatus::Used {
                                    (*status_ptr).status = crate::types::RecordStatus::Used;
                                    table.record_count += 1;
                                }
                                (*status_ptr).version += 1;
                            }
                        }
                    }
                } else {
                    // Table not found, skip records
                    #[cfg(feature = "log")]
                    warn!("SyncReceiver: Table '{}' not found, skipping records", table_name);

                    loop {
                        let used_flag = data[offset];
                        offset += 1;

                        if used_flag == 0 {
                            break;
                        }

                        // Skip record ID and data
                        offset += 4 + record_size;
                    }
                }
            }
        }

        #[cfg(feature = "log")]
        info!("SyncReceiver: Successfully applied snapshot");

        Ok(())
    }

    /// Apply WAL logs to local database
    fn apply_wal_logs(data: &[u8]) -> Result<()> {
        if data.is_empty() {
            #[cfg(feature = "log")]
            warn!("SyncReceiver: No WAL logs to apply");
            return Ok(());
        }

        #[cfg(feature = "log")]
        info!("SyncReceiver: Applying WAL logs, size: {} bytes", data.len());

        // TODO: Implement WAL log application
        // This would parse LogItem structures from the data and apply them

        #[cfg(feature = "log")]
        info!("SyncReceiver: WAL log application not yet fully implemented");

        Ok(())
    }

    /// Send acknowledgment to master
    fn send_ack(success: bool, chunks_received: u32) {
        let slave_id = unsafe {
            crate::ha::get_ha_manager()
                .map(|m| m.get_replication_manager().get_slave_id())
                .unwrap_or(0)
        };

        let ack = SyncAck::new(slave_id, success, chunks_received);
        let ack_data = ack.encode();

        if let Err(e) = pubsub::publish(SYNC_ACK_TOPIC, &ack_data) {
            #[cfg(feature = "log")]
            error!("SyncReceiver: Failed to send ack: {:?}", e);
        } else {
            #[cfg(feature = "log")]
            debug!(
                "SyncReceiver: Sent ack - success: {}, chunks: {}",
                success, chunks_received
            );
        }
    }

    /// Shutdown the sync receiver
    pub fn shutdown(&mut self) -> Result<()> {
        self.state = SyncState::Idle;
        self.accumulated_data.clear();
        self.sync_begin_info = None;

        SYNC_RECEIVER_STATE.store(SyncState::Idle as u32, Ordering::SeqCst);

        unsafe {
            SYNC_ACCUMULATED_DATA = None;
            SYNC_BEGIN_INFO = None;
        }

        Ok(())
    }

    /// Get current state
    pub fn get_state(&self) -> SyncState {
        self.state
    }
}

impl Default for SyncReceiver {
    fn default() -> Self {
        Self::new(0)
    }
}