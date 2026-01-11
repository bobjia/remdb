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

/// Returns the table content topic for a specific table name
pub fn get_table_content_topic(table_name: &str) -> alloc::string::String {
    alloc::format!("{}{}", TABLE_CONTENT_TOPIC_PREFIX, table_name)
}

/// Returns all WAL log topics
pub fn get_all_wal_topics() -> alloc::vec::Vec<&'static str> {
    alloc::vec![
        WAL_TOPIC,
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
