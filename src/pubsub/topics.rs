// PubSub topic constants

/// WAL log topic prefix
pub const WAL_TOPIC_PREFIX: &str = "wal.";

/// WAL log topic for insert operations
pub const WAL_INSERT_TOPIC: &str = "wal.insert";

/// WAL log topic for update operations
pub const WAL_UPDATE_TOPIC: &str = "wal.update";

/// WAL log topic for delete operations
pub const WAL_DELETE_TOPIC: &str = "wal.delete";

/// WAL log topic for timeseries insert operations
pub const WAL_TIMESERIES_INSERT_TOPIC: &str = "wal.timeseriesInsert";

/// WAL log topic for commit operations
pub const WAL_COMMIT_TOPIC: &str = "wal.commit";

/// WAL log topic for abort operations
pub const WAL_ABORT_TOPIC: &str = "wal.abort";

/// WAL log topic for checkpoint operations
pub const WAL_CHECKPOINT_TOPIC: &str = "wal.checkpoint";

/// WAL log topic for all operations (wildcard)
pub const WAL_ALL_TOPIC: &str = "wal.*";

/// Table content topic prefix
pub const TABLE_CONTENT_TOPIC_PREFIX: &str = "table.";

/// Tables list topic
pub const TABLES_TOPIC: &str = "tables";

/// Database metrics topic
pub const METRICS_TOPIC: &str = "metrics";

/// Health status topic
pub const HEALTH_STATUS_TOPIC: &str = "healthstatus";

/// Returns the table content topic for a specific table name
pub fn get_table_content_topic(table_name: &str) -> alloc::string::String {
    alloc::format!("{}{}", TABLE_CONTENT_TOPIC_PREFIX, table_name)
}

/// Returns all WAL log topics
pub fn get_all_wal_topics() -> alloc::vec::Vec<&'static str> {
    alloc::vec![
        WAL_INSERT_TOPIC,
        WAL_UPDATE_TOPIC,
        WAL_DELETE_TOPIC,
        WAL_TIMESERIES_INSERT_TOPIC,
        WAL_COMMIT_TOPIC,
        WAL_ABORT_TOPIC,
        WAL_CHECKPOINT_TOPIC,
        WAL_ALL_TOPIC,
    ]
}

/// Returns all core topics
pub fn get_core_topics() -> alloc::vec::Vec<&'static str> {
    alloc::vec![
        TABLES_TOPIC,
        METRICS_TOPIC,
        HEALTH_STATUS_TOPIC,
    ]
}
