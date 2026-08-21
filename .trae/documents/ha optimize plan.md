# Implementation Plan: Complete Startup Synchronization for HA

## Problem Statement

The current HA startup synchronization is incomplete:
- `request_full_sync()` only sends a sync request message via PubSub
- Master's `handle_slave_ack()` callback only logs the message and returns `true`
- No actual data is sent back to the slave for initial synchronization

## Solution Overview

Implement a complete startup sync mechanism using PubSub for communication:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        STARTUP SYNC FLOW                                │
├─────────────────────────────────────────────────────────────────────────┤
│  Slave                                  Master                          │
│    │                                      │                             │
│    │──── SYNC_REQUEST (full/incremental)─►│                             │
│    │                                      │                             │
│    │◄─── SYNC_DATA_BEGIN (metadata) ──────│                             │
│    │◄─── SYNC_DATA_CHUNK (data) ──────────│  (multiple chunks)          │
│    │◄─── SYNC_DATA_END ───────────────────│                             │
│    │                                      │                             │
│    │──── SYNC_ACK ────────────────────────►│                             │
│    │                                      │                             │
└─────────────────────────────────────────────────────────────────────────┘
```

## Implementation Tasks

### Task 1: Define Sync Protocol Constants and Data Structures

**File**: `remdb/src/ha/protocol.rs` (new file)

Define message types and structures:
- `SyncRequest` - Request from slave for full/incremental sync
- `SyncDataBegin` - Metadata about sync data (type, size, table count)
- `SyncDataChunk` - Chunk of sync data (snapshot or WAL logs)
- `SyncDataEnd` - Marks end of sync data
- `SyncAck` - Acknowledgment from slave

### Task 2: Implement Master-Side Sync Handler

**File**: `remdb/src/ha/sync_handler.rs` (new file)

Implement `SyncHandler` struct that:
1. Listens for `SYNC_REQUEST_TOPIC` messages
2. Parses sync request (full vs incremental)
3. For full sync:
   - Creates database snapshot in chunks
   - Sends `SYNC_DATA_BEGIN` with metadata
   - Sends `SYNC_DATA_CHUNK` messages for each chunk
   - Sends `SYNC_DATA_END` when complete
4. For incremental sync:
   - Reads WAL logs since requested log index
   - Sends them in chunks via `SYNC_DATA_CHUNK`

### Task 3: Implement Slave-Side Sync Receiver

**File**: `remdb/src/ha/sync_receiver.rs` (new file)

Implement `SyncReceiver` struct that:
1. Subscribes to sync data topics
2. Receives `SYNC_DATA_BEGIN` and prepares for data
3. Accumulates `SYNC_DATA_CHUNK` data
4. On `SYNC_DATA_END`, applies data to local database
5. Sends `SYNC_ACK` to confirm completion

### Task 4: Add New PubSub Topics

**File**: `remdb/src/pubsub/topics.rs`

Add new topics:
```rust
pub const SYNC_DATA_BEGIN_TOPIC: u16 = 5;
pub const SYNC_DATA_CHUNK_TOPIC: u16 = 6;
pub const SYNC_DATA_END_TOPIC: u16 = 7;
pub const SYNC_ACK_TOPIC: u16 = 8;
```

### Task 5: Integrate with HAManager

**File**: `remdb/src/ha/manager.rs`

Modify `HAManager` to:
1. Initialize `SyncHandler` on master nodes
2. Initialize `SyncReceiver` on slave nodes
3. Wait for sync completion before marking node as ready

### Task 6: Update ReplicationManager

**File**: `remdb/src/ha/replication.rs`

Update `request_full_sync()` and `request_incremental_sync()`:
1. Start `SyncReceiver` before sending request
2. Wait for sync completion with timeout
3. Return success/failure status

### Task 7: Add Sync State Tracking

**File**: `remdb/src/ha/mod.rs`

Add sync state to track synchronization progress:
```rust
pub enum SyncState {
    Idle,
    Syncing,
    Synced,
    Failed,
}
```

## Detailed Design

### Sync Request Format

```rust
// Full sync request: [slave_id(1), sync_type(1)]
// sync_type: 0 = full, 1 = incremental

// Incremental sync request: [slave_id(1), sync_type(1), last_log_index(4)]
```

### Sync Data Begin Format

```rust
struct SyncDataBegin {
    sync_type: u8,        // 0 = snapshot, 1 = WAL logs
    total_size: u64,      // Total data size in bytes
    chunk_count: u32,     // Number of chunks to expect
    table_count: u8,      // Number of tables (for snapshot)
    log_count: u32,       // Number of log items (for WAL)
}
```

### Sync Data Chunk Format

```rust
struct SyncDataChunk {
    chunk_index: u32,     // Chunk sequence number
    data_size: u16,       // Size of data in this chunk
    data: [u8],           // Actual data (max ~64KB per chunk)
}
```

### Chunking Strategy

For large snapshots:
- Split into ~64KB chunks to fit within UDP packet limits
- Each chunk includes sequence number for reordering
- Use PubSub's existing reliability (NACK/retransmit)

## File Changes Summary

| File | Action | Description |
|------|--------|-------------|
| `remdb/src/ha/protocol.rs` | Create | Sync protocol definitions |
| `remdb/src/ha/sync_handler.rs` | Create | Master-side sync logic |
| `remdb/src/ha/sync_receiver.rs` | Create | Slave-side sync logic |
| `remdb/src/ha/mod.rs` | Modify | Add new modules and sync state |
| `remdb/src/ha/manager.rs` | Modify | Integrate sync components |
| `remdb/src/ha/replication.rs` | Modify | Update sync request methods |
| `remdb/src/pubsub/topics.rs` | Modify | Add sync topics |

## Testing Strategy

1. Unit tests for protocol encoding/decoding
2. Integration tests for sync handler and receiver
3. End-to-end test: master-slave startup sync
4. Test with various data sizes (small, medium, large)
5. Test failure scenarios (timeout, network error)

## Implementation Order

1. Task 1: Protocol definitions (foundation)
2. Task 4: PubSub topics (foundation)
3. Task 2: Master-side handler
4. Task 3: Slave-side receiver
5. Task 5: HAManager integration
6. Task 6: ReplicationManager updates
7. Task 7: State tracking
8. Testing