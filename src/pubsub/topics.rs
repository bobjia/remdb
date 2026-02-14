// PubSub topic constants

/// WAL log topic for all WAL operations
pub const WAL_TOPIC: &str = "wal";

/// Table content topic prefix
pub const TABLE_CONTENT_TOPIC_PREFIX: &str = "table.";

/// Tables list topic
pub const TABLES_TOPIC: &str = "tables";

/// Database metrics topic
pub const METRICS_TOPIC: &str = "metrics";

/// Health status topic
pub const HEALTH_STATUS_TOPIC: &str = "healthstatus";

// =============================================================================
// HA Sync Protocol Topics
// =============================================================================

/// Sync request topic - slave sends sync request to master
pub const SYNC_REQUEST_TOPIC: u16 = 2;

/// Sync data begin topic - master sends sync metadata to slave
pub const SYNC_DATA_BEGIN_TOPIC: u16 = 5;

/// Sync data chunk topic - master sends sync data chunks to slave
pub const SYNC_DATA_CHUNK_TOPIC: u16 = 6;

/// Sync data end topic - master signals end of sync data
pub const SYNC_DATA_END_TOPIC: u16 = 7;

/// Sync acknowledgment topic - slave sends ack to master
pub const SYNC_ACK_TOPIC: u16 = 8;

/// Returns the table content topic for a specific table name
pub fn get_table_content_topic(table_name: &str) -> alloc::string::String {
    alloc::format!("{}{}", TABLE_CONTENT_TOPIC_PREFIX, table_name)
}

/// Returns all WAL log topics
pub fn get_all_wal_topics() -> alloc::vec::Vec<&'static str> {
    alloc::vec![WAL_TOPIC,]
}

/// Returns all core topics
pub fn get_core_topics() -> alloc::vec::Vec<&'static str> {
    alloc::vec![TABLES_TOPIC, METRICS_TOPIC, HEALTH_STATUS_TOPIC,]
}

/// Returns all HA sync topics (as topic IDs)
pub fn get_ha_sync_topics() -> alloc::vec::Vec<u16> {
    alloc::vec![
        SYNC_REQUEST_TOPIC,
        SYNC_DATA_BEGIN_TOPIC,
        SYNC_DATA_CHUNK_TOPIC,
        SYNC_DATA_END_TOPIC,
        SYNC_ACK_TOPIC,
    ]
}
