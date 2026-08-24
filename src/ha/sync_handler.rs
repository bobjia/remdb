// Master-Side Sync Handler
//
// Handles sync requests from slave nodes and sends database
// snapshots or WAL logs in chunks.

use crate::ha::protocol::{
    SyncAck, SyncDataBegin, SyncDataChunk, SyncDataEnd, SyncRequest, SyncType, MAX_CHUNK_DATA_SIZE,
};
use crate::ha::{HAError, Result, SyncState};
use crate::pubsub;
use crate::pubsub::topics::{
    SYNC_ACK_TOPIC, SYNC_DATA_BEGIN_TOPIC, SYNC_DATA_CHUNK_TOPIC, SYNC_DATA_END_TOPIC,
    SYNC_REQUEST_TOPIC,
};
use crate::transaction::LogItem;
use alloc::vec::Vec;

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

/// Sync handler state
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SyncHandlerState {
    /// Idle, waiting for sync request
    Idle,
    /// Processing sync request
    Syncing,
    /// Sync completed successfully
    Completed,
    /// Sync failed
    Failed,
}

impl From<SyncHandlerState> for SyncState {
    fn from(state: SyncHandlerState) -> Self {
        match state {
            SyncHandlerState::Idle => SyncState::Idle,
            SyncHandlerState::Syncing => SyncState::Syncing,
            SyncHandlerState::Completed => SyncState::Synced,
            SyncHandlerState::Failed => SyncState::Failed,
        }
    }
}

/// Master-side sync handler
pub struct SyncHandler {
    /// Handler state
    state: SyncHandlerState,
    /// Current sync request being processed
    current_request: Option<SyncRequest>,
    /// Number of chunks sent in current sync
    chunks_sent: u32,
    /// Total bytes sent
    bytes_sent: u64,
    /// Lock for thread safety
    lock: u32,
}

impl SyncHandler {
    /// Create a new sync handler
    pub fn new() -> Self {
        Self {
            state: SyncHandlerState::Idle,
            current_request: None,
            chunks_sent: 0,
            bytes_sent: 0,
            lock: 0,
        }
    }

    /// Initialize the sync handler (subscribe to sync request topic)
    pub fn init(&mut self) -> Result<()> {
        #[cfg(feature = "log")]
        debug!("SyncHandler: Initializing and subscribing to SYNC_REQUEST_TOPIC");

        // Subscribe to sync request topic
        pubsub::subscribe(SYNC_REQUEST_TOPIC, Self::handle_sync_request_callback)
            .map_err(|_| HAError::InitFailed)?;

        // Subscribe to sync ack topic to receive acknowledgments
        pubsub::subscribe(SYNC_ACK_TOPIC, Self::handle_sync_ack_callback)
            .map_err(|_| HAError::InitFailed)?;

        #[cfg(feature = "log")]
        info!("SyncHandler: Successfully initialized");

        Ok(())
    }

    /// Callback for handling sync requests
    fn handle_sync_request_callback(topic_id: u16, data: &[u8]) -> bool {
        if topic_id != SYNC_REQUEST_TOPIC {
            return false;
        }

        #[cfg(feature = "log")]
        debug!(
            "SyncHandler: Received sync request, data len: {}",
            data.len()
        );

        // Parse the sync request
        let request = match SyncRequest::decode(data) {
            Some(req) => req,
            None => {
                #[cfg(feature = "log")]
                error!("SyncHandler: Failed to decode sync request");
                return false;
            }
        };

        #[cfg(feature = "log")]
        info!(
            "SyncHandler: Sync request from slave {}, type: {:?}",
            request.slave_id, request.sync_type
        );

        // Process the request (spawn processing in background)
        match request.sync_type {
            SyncType::Full => {
                Self::process_full_sync_request(request);
            }
            SyncType::Incremental => {
                Self::process_incremental_sync_request(request);
            }
        }

        true
    }

    /// Callback for handling sync acknowledgments
    fn handle_sync_ack_callback(topic_id: u16, data: &[u8]) -> bool {
        if topic_id != SYNC_ACK_TOPIC {
            return false;
        }

        // Parse the acknowledgment
        let ack = match SyncAck::decode(data) {
            Some(a) => a,
            None => {
                #[cfg(feature = "log")]
                warn!("SyncHandler: Failed to decode sync ack");
                return false;
            }
        };

        #[cfg(feature = "log")]
        info!(
            "SyncHandler: Received sync ack from slave {}, success: {}, chunks: {}",
            ack.slave_id, ack.success, ack.chunks_received
        );

        true
    }

    /// Process a full sync request
    fn process_full_sync_request(request: SyncRequest) {
        #[cfg(feature = "log")]
        debug!(
            "SyncHandler: Processing full sync request from slave {}",
            request.slave_id
        );

        // Get database snapshot
        let snapshot_data = match Self::create_database_snapshot() {
            Ok(data) => data,
            Err(e) => {
                #[cfg(feature = "log")]
                error!("SyncHandler: Failed to create snapshot: {:?}", e);
                return;
            }
        };

        #[cfg(feature = "log")]
        info!(
            "SyncHandler: Created snapshot, size: {} bytes",
            snapshot_data.len()
        );

        // Split into chunks and send
        if let Err(e) = Self::send_snapshot_chunks(&snapshot_data) {
            #[cfg(feature = "log")]
            error!("SyncHandler: Failed to send snapshot chunks: {:?}", e);
        }
    }

    /// Process an incremental sync request
    fn process_incremental_sync_request(request: SyncRequest) {
        #[cfg(feature = "log")]
        debug!(
            "SyncHandler: Processing incremental sync request from slave {}, last_log_index: {}",
            request.slave_id, request.last_log_index
        );

        // Get WAL logs since the requested index
        let wal_data = match Self::get_wal_logs_since(request.last_log_index) {
            Ok(data) => data,
            Err(e) => {
                #[cfg(feature = "log")]
                error!("SyncHandler: Failed to get WAL logs: {:?}", e);
                return;
            }
        };

        #[cfg(feature = "log")]
        info!(
            "SyncHandler: Retrieved WAL logs, size: {} bytes",
            wal_data.len()
        );

        // Send WAL logs in chunks
        if let Err(e) = Self::send_wal_chunks(&wal_data) {
            #[cfg(feature = "log")]
            error!("SyncHandler: Failed to send WAL chunks: {:?}", e);
        }
    }

    /// Create a database snapshot
    fn create_database_snapshot() -> Result<Vec<u8>> {
        let mut snapshot = Vec::new();

        // Get the global database instance
        let db = unsafe { crate::get_global_db() }.ok_or(HAError::SyncFailed)?;

        unsafe {
            // Write header: number of tables
            let table_count = db.tables.len() as u8;
            snapshot.push(table_count);

            // Iterate through each table
            for (table_id, table_opt) in db.tables.iter().enumerate() {
                if let Some(table) = table_opt {
                    // Write table metadata
                    let table_name = &table.def.name;
                    let name_bytes = table_name.as_bytes();
                    snapshot.push(name_bytes.len() as u8);
                    snapshot.extend_from_slice(name_bytes);

                    // Write record size
                    snapshot.extend_from_slice(&(table.record_size as u32).to_le_bytes());

                    // Write record count
                    snapshot.extend_from_slice(&(table.record_count as u32).to_le_bytes());

                    // Write max records
                    snapshot.extend_from_slice(&(table.def.max_records as u32).to_le_bytes());

                    // Write field count
                    snapshot.push(table.def.fields.len() as u8);

                    // Write field definitions
                    for field in &table.def.fields {
                        // Field name
                        let field_name_bytes = field.name.as_bytes();
                        snapshot.push(field_name_bytes.len() as u8);
                        snapshot.extend_from_slice(field_name_bytes);

                        // Data type
                        snapshot.push(field.data_type as u8);

                        // Offset
                        snapshot.extend_from_slice(&(field.offset as u16).to_le_bytes());

                        // Dimension (for vectors) - get from vector_metadata
                        let dimension = field
                            .vector_metadata
                            .as_ref()
                            .map(|vm| vm.dimension)
                            .unwrap_or(0);
                        snapshot.extend_from_slice(&dimension.to_le_bytes());
                    }

                    // Write primary key count
                    snapshot.push(table.def.primary_key.len() as u8);

                    // Write primary key indices
                    for &pk_idx in &table.def.primary_key {
                        snapshot.push(pk_idx as u8);
                    }

                    // Write records (only used slots)
                    for record_id in 0..table.def.max_records {
                        let status_ptr = table.get_status_ptr(record_id);
                        if (*status_ptr).status == crate::types::RecordStatus::Used {
                            // Mark as used record
                            snapshot.push(1); // Used flag

                            // Write record ID
                            snapshot.extend_from_slice(&(record_id as u32).to_le_bytes());

                            // Write record data (read-only access)
                            let record_ptr = table.get_record_ptr(record_id);
                            let record_data =
                                core::slice::from_raw_parts(record_ptr, table.record_size);
                            snapshot.extend_from_slice(record_data);
                        }
                    }

                    // Mark end of records for this table
                    snapshot.push(0); // End of records marker

                    #[cfg(feature = "log")]
                    debug!(
                        "SyncHandler: Snapshotted table '{}' (id: {}), {} records",
                        table_name, table_id, table.record_count
                    );
                }
            }
        }

        Ok(snapshot)
    }

    /// Get WAL logs since a specific index
    fn get_wal_logs_since(_last_log_index: u32) -> Result<Vec<u8>> {
        // TODO: Implement WAL log retrieval
        // This would need access to the WAL storage system
        #[cfg(feature = "log")]
        warn!("SyncHandler: WAL log retrieval not yet implemented");

        // Return empty data for now
        Ok(Vec::new())
    }

    /// Send snapshot data in chunks
    fn send_snapshot_chunks(data: &[u8]) -> Result<()> {
        let total_size = data.len() as u64;
        let chunk_count = (total_size as usize).div_ceil(MAX_CHUNK_DATA_SIZE) as u32;

        // Get table count from the data
        let table_count = if !data.is_empty() { data[0] } else { 0 };

        #[cfg(feature = "log")]
        info!(
            "SyncHandler: Sending {} chunks, total size: {} bytes, {} tables",
            chunk_count, total_size, table_count
        );

        // Send SYNC_DATA_BEGIN
        let begin = SyncDataBegin::new_snapshot(total_size, chunk_count, table_count);
        let begin_data = begin.encode();
        pubsub::publish(SYNC_DATA_BEGIN_TOPIC, &begin_data).map_err(|_| HAError::SyncFailed)?;

        // Send chunks
        let mut offset = 0;
        let mut chunk_index = 0;

        while offset < data.len() {
            let chunk_end = core::cmp::min(offset + MAX_CHUNK_DATA_SIZE, data.len());
            let chunk_data = &data[offset..chunk_end];

            let chunk = SyncDataChunk::new(chunk_index, chunk_data);
            let encoded = chunk.encode();

            pubsub::publish(SYNC_DATA_CHUNK_TOPIC, &encoded).map_err(|_| HAError::SyncFailed)?;

            offset = chunk_end;
            chunk_index += 1;

            // Small delay between chunks to avoid overwhelming the network
            #[cfg(feature = "std")]
            std::thread::sleep(std::time::Duration::from_micros(100));
        }

        // Send SYNC_DATA_END
        let end = SyncDataEnd::new(chunk_count, 0); // No checksum for now
        let end_data = end.encode();
        pubsub::publish(SYNC_DATA_END_TOPIC, &end_data).map_err(|_| HAError::SyncFailed)?;

        #[cfg(feature = "log")]
        info!("SyncHandler: Completed sending {} chunks", chunk_count);

        Ok(())
    }

    /// Send WAL logs in chunks
    fn send_wal_chunks(data: &[u8]) -> Result<()> {
        let total_size = data.len() as u64;
        let chunk_count = (total_size as usize).div_ceil(MAX_CHUNK_DATA_SIZE) as u32;

        // Estimate log count (rough estimate based on average LogItem size)
        let log_count = (total_size / core::mem::size_of::<LogItem>() as u64) as u32;

        #[cfg(feature = "log")]
        info!(
            "SyncHandler: Sending {} WAL chunks, total size: {} bytes, ~{} logs",
            chunk_count, total_size, log_count
        );

        // Send SYNC_DATA_BEGIN
        let begin = SyncDataBegin::new_wal(total_size, chunk_count, log_count);
        let begin_data = begin.encode();
        pubsub::publish(SYNC_DATA_BEGIN_TOPIC, &begin_data).map_err(|_| HAError::SyncFailed)?;

        // Send chunks
        let mut offset = 0;
        let mut chunk_index = 0;

        while offset < data.len() {
            let chunk_end = core::cmp::min(offset + MAX_CHUNK_DATA_SIZE, data.len());
            let chunk_data = &data[offset..chunk_end];

            let chunk = SyncDataChunk::new(chunk_index, chunk_data);
            let encoded = chunk.encode();

            pubsub::publish(SYNC_DATA_CHUNK_TOPIC, &encoded).map_err(|_| HAError::SyncFailed)?;

            offset = chunk_end;
            chunk_index += 1;

            // Small delay between chunks
            #[cfg(feature = "std")]
            std::thread::sleep(std::time::Duration::from_micros(100));
        }

        // Send SYNC_DATA_END
        let end = SyncDataEnd::new(chunk_count, 0);
        let end_data = end.encode();
        pubsub::publish(SYNC_DATA_END_TOPIC, &end_data).map_err(|_| HAError::SyncFailed)?;

        #[cfg(feature = "log")]
        info!("SyncHandler: Completed sending {} WAL chunks", chunk_count);

        Ok(())
    }

    /// Shutdown the sync handler
    pub fn shutdown(&mut self) -> Result<()> {
        self.state = SyncHandlerState::Idle;
        self.current_request = None;
        self.chunks_sent = 0;
        self.bytes_sent = 0;
        Ok(())
    }

    /// Get current state
    pub fn get_state(&self) -> SyncHandlerState {
        self.state
    }
}

impl Default for SyncHandler {
    fn default() -> Self {
        Self::new()
    }
}
