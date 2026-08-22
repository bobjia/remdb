/*
 * RemDB C API Header File
 *
 * This file defines the C API for RemDB, a lightweight embedded database.
 * The API is designed for resource-constrained embedded systems with
 * optimized memory usage and minimal CPU overhead.
 *
 * Copyright (c) 2023 RemDB Team
 */

#ifndef REMDB_H
#define REMDB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* =========================================== */
/*              Data Types                     */
/* =========================================== */

/**
 * @brief Data types supported by RemDB
 * Note: Use uint8_t for ABI compatibility with Rust's #[repr(u8)] enum
 */
typedef uint8_t RemDbDataType;

#define REMDB_TYPE_UINT8     ((RemDbDataType)0)
#define REMDB_TYPE_UINT16    ((RemDbDataType)1)
#define REMDB_TYPE_UINT32    ((RemDbDataType)2)
#define REMDB_TYPE_UINT64    ((RemDbDataType)3)
#define REMDB_TYPE_FLOAT32   ((RemDbDataType)4)
#define REMDB_TYPE_FLOAT64   ((RemDbDataType)5)
#define REMDB_TYPE_BOOL      ((RemDbDataType)6)
#define REMDB_TYPE_TIMESTAMP ((RemDbDataType)7)
#define REMDB_TYPE_STRING    ((RemDbDataType)8)
#define REMDB_TYPE_JSON      ((RemDbDataType)9)
#define REMDB_TYPE_VECTOR    ((RemDbDataType)10)

/**
 * @brief Compression types for time series data
 */
enum RemDbCompressionType {
    REMDB_COMPRESSION_NONE = 0,
    REMDB_COMPRESSION_DELTA_RUN_LENGTH = 1,
    REMDB_COMPRESSION_SNAPPY = 2,
};

/**
 * @brief Maximum string length supported by RemDB
 */
#define REMDB_MAX_STRING_LEN 64

/**
 * @brief Universal value type for RemDB
 */
typedef union RemDbValue {
    uint8_t u8;
    uint16_t u16;
    uint32_t u32;
    uint64_t u64;
    float float32;
    double float64;
    uint8_t boolean;
    uint64_t timestamp;
    uint8_t string[REMDB_MAX_STRING_LEN];
    struct {
        uint8_t pool_id;
        uint32_t offset;
        uint32_t length;
    } json;
    struct {
        uint8_t pool_id;
        uint32_t offset;
        uint32_t length;
    } vector;
} RemDbValue;

/**
 * @brief Field definition
 */
typedef struct RemDbFieldDef {
    const char* name;
    RemDbDataType data_type;
    size_t size;
    size_t offset;
} RemDbFieldDef;

/**
 * @brief Table definition
 */
typedef struct RemDbTableDef {
    uint8_t id;
    const char* name;
    const RemDbFieldDef* fields;
    size_t fields_count;
    size_t primary_key;
    int32_t secondary_index;
    size_t record_size;
    size_t max_records;
} RemDbTableDef;

/**
 * @brief Typed value for SQL result set
 * Note: data_type is uint8_t (1 byte) with 7 bytes padding before value
 * to match Rust's #[repr(C)] layout with #[repr(u8)] enum
 */
typedef struct RemDbTypedValue {
    RemDbDataType data_type;  /* 1 byte */
    /* 7 bytes implicit padding to align value to 8 bytes */
    RemDbValue value;         /* 64 bytes */
} RemDbTypedValue;

/**
 * @brief Result row for SQL result set
 */
typedef struct RemDbResultRow {
    const RemDbTypedValue* values;
    size_t values_count;
} RemDbResultRow;

/**
 * @brief Result set for SQL query
 */
typedef struct RemDbResultSet {
    const char** columns;
    size_t columns_count;
    const RemDbResultRow* rows;
    size_t rows_count;
} RemDbResultSet;

/**
 * @brief Time series record
 */
typedef struct RemDbTimeSeriesRecord {
    uint64_t timestamp;
    double value;
    uint8_t tag_count;
    uint64_t tags[8];
} RemDbTimeSeriesRecord;

/**
 * @brief Time series configuration
 */
typedef struct RemDbTimeSeriesConfig {
    uint64_t partition_duration_secs;
    uint64_t retention_period_secs;
    enum RemDbCompressionType compression;
    size_t max_partitions;
} RemDbTimeSeriesConfig;

/**
 * @brief Time series table definition
 */
typedef struct RemDbTimeSeriesTableDef {
    uint8_t id;
    const char* name;
    const RemDbFieldDef* fields;
    size_t fields_count;
    size_t primary_key;
    int32_t secondary_index;
    size_t record_size;
    size_t max_records;
    size_t time_field;
    size_t value_field;
    const size_t* tag_fields;
    size_t tag_fields_count;
    RemDbTimeSeriesConfig config;
} RemDbTimeSeriesTableDef;

/**
 * @brief Database configuration
 */
typedef struct RemDbConfig {
    const RemDbTableDef* tables;
    size_t tables_count;
    const RemDbTimeSeriesTableDef* time_series_tables;
    size_t time_series_tables_count;
    size_t total_memory;
    uint8_t low_power_mode_supported;
    int32_t low_power_max_records;
    void* ha_config;
} RemDbConfig;

/**
 * @brief Database handle
 */
typedef void* RemDbHandle;

/**
 * @brief Transaction type
 */
enum RemDbTransactionType {
    REMDB_TX_READ = 0,
    REMDB_TX_WRITE = 1,
};

/**
 * @brief Isolation level
 */
enum RemDbIsolationLevel {
    REMDB_ISO_READ_UNCOMMITTED = 0,
    REMDB_ISO_READ_COMMITTED = 1,
    REMDB_ISO_REPEATABLE_READ = 2,
    REMDB_ISO_SERIALIZABLE = 3,
};

/**
 * @brief Database metrics snapshot
 */
typedef struct RemDbMetricsSnapshot {
    size_t total_memory;
    size_t used_memory;
    uint64_t read_ops;
    uint64_t write_ops;
    uint64_t delete_ops;
    uint64_t update_ops;
    uint64_t cache_hits;
    uint64_t cache_misses;
    uint64_t index_lookups;
    uint64_t index_inserts;
    uint64_t index_deletes;
    uint64_t transactions;
    uint64_t committed_transactions;
    uint64_t rolled_back_transactions;
    uint64_t start_time;
} RemDbMetricsSnapshot;

/**
 * @brief Health status
 */
enum RemDbHealthStatus {
    REMDB_HEALTH_HEALTHY = 0,
    REMDB_HEALTH_WARNING = 1,
    REMDB_HEALTH_UNHEALTHY = 2,
};

/**
 * @brief Health check result
 */
typedef struct RemDbHealthCheckResult {
    enum RemDbHealthStatus status;
    RemDbMetricsSnapshot metrics;
    const char* details;
} RemDbHealthCheckResult;

/* =========================================== */
/*              Vector Index Types              */
/* =========================================== */

/**
 * @brief Vector index type
 */
enum RemDbVectorIndexType {
    REMDB_VECTOR_INDEX_HNSW = 0,
    REMDB_VECTOR_INDEX_HNSW_SQ = 1,
    REMDB_VECTOR_INDEX_HNSW_BQ = 2,
    REMDB_VECTOR_INDEX_IVF = 3,
    REMDB_VECTOR_INDEX_IVF_PQ = 4,
};

/**
 * @brief Vector distance type
 */
enum RemDbDistanceType {
    REMDB_DISTANCE_L2 = 0,
    REMDB_DISTANCE_INNER_PRODUCT = 1,
    REMDB_DISTANCE_COSINE = 2,
};

/**
 * @brief Vector metadata configuration
 */
typedef struct RemDbVectorMetadata {
    uint16_t dimension;
    enum RemDbDistanceType distance_type;
    enum RemDbVectorIndexType index_type;
    uint8_t compression_enabled;
    uint8_t compression_scheme;
    uint8_t compression_level;
    uint8_t hnsw_m;
    uint32_t hnsw_ef_construction;
    uint32_t hnsw_ef_search;
    uint32_t ivf_nlist;
    uint32_t ivf_nprobe;
} RemDbVectorMetadata;

/* =========================================== */
/*              PubSub Types                   */
/* =========================================== */

/**
 * @brief UDP mode for PubSub
 */
enum RemDbUdpMode {
    REMDB_UDP_UNICAST = 0,
    REMDB_UDP_BROADCAST = 1,
    REMDB_UDP_MULTICAST = 2,
};

/**
 * @brief PubSub configuration
 */
typedef struct RemDbPubSubConfig {
    enum RemDbUdpMode udp_mode;
    const char* multicast_addr;
    uint16_t port;
    size_t max_topics;
    size_t max_subscribers_per_topic;
    size_t buffer_size;
    uint8_t enable_nack;
    uint32_t retransmit_timeout_ms;
    size_t max_retransmits;
    uint32_t heartbeat_interval_secs;
    size_t frame_pool_size;
} RemDbPubSubConfig;

/**
 * @brief PubSub callback function type
 */
typedef uint8_t (*RemDbPubSubCallback)(uint16_t topic_id, const uint8_t* data, size_t data_len);

/* =========================================== */
/*              HA Types                       */
/* =========================================== */

/**
 * @brief HA role
 */
enum RemDbHARole {
    REMDB_HA_ROLE_MASTER = 0,
    REMDB_HA_ROLE_SLAVE = 1,
    REMDB_HA_ROLE_AUTO = 2,
};

/**
 * @brief Replication mode
 */
enum RemDbReplicationMode {
    REMDB_REPLICATION_MODE_ASYNC = 0,
    REMDB_REPLICATION_MODE_SYNC = 1,
};

/**
 * @brief HA configuration
 */
typedef struct RemDbHAConfig {
    enum RemDbHARole ha_role;
    enum RemDbReplicationMode replication_mode;
    uint32_t heartbeat_interval_ms;
    uint32_t failure_detection_ms;
    uint32_t sync_timeout_ms;
    const char* master_address;
    uint16_t master_port;
    uint16_t replication_port;
    uint16_t heartbeat_port;
    uint32_t node_id;
} RemDbHAConfig;

/* =========================================== */
/*              Error Codes                    */
/* =========================================== */

/**
 * @brief Error codes returned by RemDB API functions
 */
enum RemDbError {
    REMDB_SUCCESS = 0,
    REMDB_ERROR_OUT_OF_MEMORY = 1,
    REMDB_ERROR_RECORD_NOT_FOUND = 2,
    REMDB_ERROR_DUPLICATE_KEY = 3,
    REMDB_ERROR_FIELD_NOT_FOUND = 4,
    REMDB_ERROR_TYPE_MISMATCH = 5,
    REMDB_ERROR_TRANSACTION_ERROR = 6,
    REMDB_ERROR_CONFIG_ERROR = 7,
    REMDB_ERROR_UNSUPPORTED_OPERATION = 8,
    REMDB_ERROR_FILE_IO_ERROR = 9,
    REMDB_ERROR_SNAPSHOT_FORMAT_ERROR = 10,
    REMDB_ERROR_CRC32_ERROR = 11,
    REMDB_ERROR_LOG_FORMAT_ERROR = 12,
    REMDB_ERROR_LOG_RECORD_NOT_FOUND = 13,
    REMDB_ERROR_LOG_CHECKSUM_ERROR = 14,
    REMDB_ERROR_LOCK_CONFLICT = 15,
    REMDB_ERROR_LOCK_TIMEOUT = 16,
    REMDB_ERROR_TABLE_NOT_FOUND = 17,
    REMDB_ERROR_INVALID_RECORD_SIZE = 18,
    REMDB_ERROR_PUBSUB_INIT_FAILED = 19,
    REMDB_ERROR_PUBSUB_NETWORK_ERROR = 20,
    REMDB_ERROR_PUBSUB_INVALID_PARAMETER = 21,
    REMDB_ERROR_PUBSUB_RESOURCE_EXHAUSTED = 22,
    REMDB_ERROR_PUBSUB_INVALID_FRAME_FORMAT = 23,
    REMDB_ERROR_PUBSUB_CRC_CHECK_FAILED = 24,
    REMDB_ERROR_PUBSUB_TOPIC_NOT_FOUND = 25,
    REMDB_ERROR_PUBSUB_SUBSCRIPTION_NOT_FOUND = 26,
    REMDB_ERROR_NOT_ALLOWED = 27,
};

/* =========================================== */
/*              API Functions                  */
/* =========================================== */

/**
 * @brief Initialize the global database instance
 *
 * @param config Database configuration
 * @param handle Output parameter for database handle
 * @return Error code
 */
enum RemDbError remdb_init_global(const RemDbConfig* config, RemDbHandle* handle);

/**
 * @brief Get the global database instance
 *
 * @param handle Output parameter for database handle
 * @return Error code
 */
enum RemDbError remdb_get_global(RemDbHandle* handle);

/**
 * @brief Enter low power mode
 *
 * @param handle Database handle
 * @return Error code
 */
enum RemDbError remdb_enter_low_power_mode(RemDbHandle handle);

/**
 * @brief Exit low power mode
 *
 * @param handle Database handle
 * @return Error code
 */
enum RemDbError remdb_exit_low_power_mode(RemDbHandle handle);

/**
 * @brief Check if low power mode is enabled
 *
 * @param handle Database handle
 * @param is_enabled Output parameter for low power mode status
 * @return Error code
 */
enum RemDbError remdb_is_low_power_mode(RemDbHandle handle, uint8_t* is_enabled);

/**
 * @brief Begin a transaction
 *
 * @param handle Database handle
 * @param tx_type Transaction type
 * @param isolation_level Isolation level
 * @return Error code
 */
enum RemDbError remdb_begin_transaction(RemDbHandle handle, 
                                       enum RemDbTransactionType tx_type,
                                       enum RemDbIsolationLevel isolation_level);

/**
 * @brief Commit a transaction
 *
 * @param handle Database handle
 * @return Error code
 */
enum RemDbError remdb_commit_transaction(RemDbHandle handle);

/**
 * @brief Rollback a transaction
 *
 * @param handle Database handle
 * @return Error code
 */
enum RemDbError remdb_rollback_transaction(RemDbHandle handle);

/**
 * @brief Save snapshot to file
 *
 * @param handle Database handle
 * @param path File path
 * @return Error code
 */
enum RemDbError remdb_save_snapshot(RemDbHandle handle, const char* path);

/**
 * @brief Restore snapshot from file
 *
 * @param handle Database handle
 * @param path File path
 * @return Error code
 */
enum RemDbError remdb_restore_snapshot(RemDbHandle handle, const char* path);

/**
 * @brief Save incremental snapshot to file
 *
 * @param handle Database handle
 * @param path File path
 * @return Error code
 */
enum RemDbError remdb_save_incremental_snapshot(RemDbHandle handle, const char* path);

/**
 * @brief Get metrics snapshot
 *
 * @param handle Database handle
 * @param snapshot Output parameter for metrics snapshot
 * @return Error code
 */
enum RemDbError remdb_get_metrics_snapshot(RemDbHandle handle, RemDbMetricsSnapshot* snapshot);

/**
 * @brief Reset all metrics
 *
 * @param handle Database handle
 * @return Error code
 */
enum RemDbError remdb_reset_metrics(RemDbHandle handle);

/**
 * @brief Perform health check
 *
 * @param handle Database handle
 * @param result Output parameter for health check result
 * @return Error code
 */
enum RemDbError remdb_health_check(RemDbHandle handle, RemDbHealthCheckResult* result);

/**
 * @brief Dump metrics to string
 *
 * @param handle Database handle
 * @param buffer Output buffer
 * @param buffer_size Buffer size
 * @param written Output parameter for number of bytes written
 * @return Error code
 */
enum RemDbError remdb_dump_metrics(RemDbHandle handle, char* buffer, size_t buffer_size, size_t* written);

/**
 * @brief Get the snapshot version
 *
 * @param handle Database handle
 * @param version Output parameter for snapshot version
 * @return Error code
 */
enum RemDbError remdb_get_snapshot_version(RemDbHandle handle, uint32_t* version);

/* =========================================== */
/*              Table Operations               */
/* =========================================== */

/**
 * @brief Insert a record into a table
 *
 * @param handle Database handle
 * @param table_id Table ID
 * @param record Record data
 * @return Error code
 */
enum RemDbError remdb_table_insert(RemDbHandle handle, size_t table_id, const void* record);

/**
 * @brief Get a record from a table by primary key
 *
 * @param handle Database handle
 * @param table_id Table ID
 * @param key Primary key value
 * @param record Output buffer for record data
 * @return Error code
 */
enum RemDbError remdb_table_get(RemDbHandle handle, size_t table_id, const RemDbValue* key, void* record);

/**
 * @brief Update a record in a table by primary key
 *
 * @param handle Database handle
 * @param table_id Table ID
 * @param key Primary key value
 * @param record New record data
 * @return Error code
 */
enum RemDbError remdb_table_update(RemDbHandle handle, size_t table_id, const RemDbValue* key, const void* record);

/**
 * @brief Delete a record from a table by primary key
 *
 * @param handle Database handle
 * @param table_id Table ID
 * @param key Primary key value
 * @return Error code
 */
enum RemDbError remdb_table_delete(RemDbHandle handle, size_t table_id, const RemDbValue* key);

/**
 * @brief Get record count for a table
 *
 * @param handle Database handle
 * @param table_id Table ID
 * @param count Output parameter for record count
 * @return Error code
 */
enum RemDbError remdb_table_get_record_count(RemDbHandle handle, size_t table_id, size_t* count);

/**
 * @brief Get table by name
 *
 * @param handle Database handle
 * @param name Table name
 * @param table_id Output parameter for table ID
 * @return Error code
 */
enum RemDbError remdb_table_get_by_name(RemDbHandle handle, const char* name, size_t* table_id);

/* =========================================== */
/*           Time Series Operations            */
/* =========================================== */

/**
 * @brief Batch write time series records
 *
 * @param handle Database handle
 * @param table_id Time series table ID
 * @param records Array of time series records
 * @param count Number of records to write
 * @param written Output parameter for number of records written
 * @return Error code
 */
enum RemDbError remdb_time_series_batch_write(RemDbHandle handle, size_t table_id, const RemDbTimeSeriesRecord* records, size_t count, size_t* written);

/**
 * @brief Query time series data by time range
 *
 * @param handle Database handle
 * @param table_id Time series table ID
 * @param start_time Start timestamp (in seconds)
 * @param end_time End timestamp (in seconds)
 * @param buffer Output buffer for results
 * @param buffer_size Maximum number of records to return
 * @param result_count Output parameter for number of records returned
 * @return Error code
 */
enum RemDbError remdb_time_series_query(RemDbHandle handle, size_t table_id, uint64_t start_time, uint64_t end_time, RemDbTimeSeriesRecord* buffer, size_t buffer_size, size_t* result_count);

/**
 * @brief Get time series table by name
 *
 * @param handle Database handle
 * @param name Time series table name
 * @param table_id Output parameter for table ID
 * @return Error code
 */
enum RemDbError remdb_time_series_table_get_by_name(RemDbHandle handle, const char* name, size_t* table_id);

/* =========================================== */
/*              SQL Query Operations           */
/* =========================================== */

/**
 * @brief Execute SQL query
 *
 * @param handle Database handle
 * @param sql SQL query string
 * @param result_set Output parameter for result set
 * @return Error code
 */
enum RemDbError remdb_sql_query(RemDbHandle handle, const char* sql, RemDbResultSet** result_set);

/**
 * @brief Execute query operation
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param columns Column names to query
 * @param columns_count Number of columns
 * @param where_clause WHERE clause (optional)
 * @param limit Result limit (-1 for no limit)
 * @param result_set Output parameter for result set
 * @return Error code
 */
enum RemDbError remdb_execute_query(RemDbHandle handle, const char* table_name, const char** columns, size_t columns_count, const char* where_clause, int32_t limit, RemDbResultSet** result_set);

/**
 * @brief Free result set memory
 *
 * @param result_set Result set to free
 * @return Error code
 */
enum RemDbError remdb_free_result_set(RemDbResultSet* result_set);

/**
 * @brief Get JSON string from a typed value
 *
 * @param value Typed value containing JSON data
 * @param json_string Output parameter for JSON string pointer
 * @param length Output parameter for string length
 * @return Error code
 */
enum RemDbError remdb_get_json_string(const RemDbTypedValue* value, const char** json_string, size_t* length);

/**
 * @brief Free string memory allocated by RemDB
 *
 * @param s String pointer to free
 * @return Error code
 */
enum RemDbError remdb_free_string(const char* s);

/* =========================================== */
/*              Data Manipulation Operations   */
/* =========================================== */

/**
 * @brief Create table
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param fields Field definitions
 * @param fields_count Number of fields
 * @param primary_key Primary key field index (-1 for no primary key)
 * @return Error code
 */
enum RemDbError remdb_create_table(RemDbHandle handle, const char* table_name, const RemDbFieldDef* fields, size_t fields_count, int32_t primary_key);

/**
 * @brief Batch insert records
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param column_names Column names
 * @param column_names_count Number of column names
 * @param records Records to insert
 * @param records_count Number of records
 * @param values_per_record Number of values per record
 * @param affected_rows Output parameter for number of affected rows
 * @return Error code
 */
enum RemDbError remdb_batch_insert_record(RemDbHandle handle, const char* table_name, const char** column_names, size_t column_names_count, const char*** records, size_t records_count, size_t values_per_record, size_t* affected_rows);

/**
 * @brief Update records
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param set_clause SET clause
 * @param where_clause WHERE clause (optional)
 * @param affected_rows Output parameter for number of affected rows
 * @return Error code
 */
enum RemDbError remdb_update_record(RemDbHandle handle, const char* table_name, const char* set_clause, const char* where_clause, size_t* affected_rows);

/**
 * @brief Delete records
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param where_clause WHERE clause (optional)
 * @param affected_rows Output parameter for number of affected rows
 * @return Error code
 */
enum RemDbError remdb_delete_record(RemDbHandle handle, const char* table_name, const char* where_clause, size_t* affected_rows);

/* =========================================== */
/*              Export Operations              */
/* =========================================== */

/**
 * @brief Export DDL to file
 *
 * @param handle Database handle
 * @param path File path
 * @return Error code
 */
enum RemDbError remdb_export_ddl(RemDbHandle handle, const char* path);

/**
 * @brief Export data to file
 *
 * @param handle Database handle
 * @param path File path
 * @return Error code
 */
enum RemDbError remdb_export_data(RemDbHandle handle, const char* path);

/* =========================================== */
/*              Vector Index Operations         */
/* =========================================== */

/**
 * @brief Initialize index build thread pool
 *
 * @param thread_count Number of threads to use for index building
 * @return Error code
 */
enum RemDbError remdb_init_index_build_thread_pool(uint32_t thread_count);

/**
 * @brief Create vector index
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param field_name Field name to index
 * @param metadata Vector metadata configuration
 * @return Error code
 */
enum RemDbError remdb_create_vector_index(RemDbHandle handle, const char* table_name, const char* field_name, const RemDbVectorMetadata* metadata);

/**
 * @brief Vector similarity search
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param field_name Field name with vector index
 * @param query_vector Query vector
 * @param vector_dim Vector dimension
 * @param k Number of results to return
 * @param results Output parameter for matching record IDs
 * @param distances Output parameter for distances
 * @param result_count Output parameter for actual number of results
 * @return Error code
 */
enum RemDbError remdb_vector_search(RemDbHandle handle, const char* table_name, const char* field_name, const float* query_vector, uint16_t vector_dim, uint32_t k, uint32_t** results, float** distances, uint32_t* result_count);

/**
 * @brief Free vector search results memory
 *
 * @param results Results array to free
 * @param distances Distances array to free
 * @param count Number of elements in arrays
 * @return Error code
 */
enum RemDbError remdb_free_vector_search_results(uint32_t* results, float* distances, uint32_t count);

/**
 * @brief Get index build status
 *
 * @param handle Database handle
 * @param table_name Table name
 * @param field_name Field name
 * @param is_building Output parameter for build status
 * @param progress Output parameter for build progress (0-100)
 * @return Error code
 */
enum RemDbError remdb_get_index_build_status(RemDbHandle handle, const char* table_name, const char* field_name, uint8_t* is_building, uint32_t* progress);

/* =========================================== */
/*              PubSub Operations              */
/* =========================================== */

/**
 * @brief Initialize PubSub system
 *
 * @param config PubSub configuration
 * @return Error code
 */
enum RemDbError remdb_pubsub_init(const RemDbPubSubConfig* config);

/**
 * @brief Subscribe to a topic
 *
 * @param topic_id Topic ID to subscribe to
 * @param callback Callback function for received messages
 * @param subscription_id Output parameter for subscription ID
 * @return Error code
 */
enum RemDbError remdb_pubsub_subscribe(uint16_t topic_id, RemDbPubSubCallback callback, size_t* subscription_id);

/**
 * @brief Unsubscribe from a topic
 *
 * @param subscription_id Subscription ID to cancel
 * @return Error code
 */
enum RemDbError remdb_pubsub_unsubscribe(size_t subscription_id);

/**
 * @brief Publish data to a topic
 *
 * @param topic_id Topic ID to publish to
 * @param data Data to publish
 * @param data_len Length of data
 * @return Error code
 */
enum RemDbError remdb_pubsub_publish(uint16_t topic_id, const uint8_t* data, size_t data_len);

/**
 * @brief Start PubSub receiver thread
 *
 * @return Error code
 */
enum RemDbError remdb_pubsub_start_receiver(void);

/**
 * @brief Shutdown PubSub system
 *
 * @return Error code
 */
enum RemDbError remdb_pubsub_shutdown(void);

/* =========================================== */
/*              HA Operations                  */
/* =========================================== */

/**
 * @brief Get current HA role
 *
 * @param role Output parameter for current HA role
 * @return Error code
 */
enum RemDbError remdb_ha_get_role(enum RemDbHARole* role);

/**
 * @brief Promote current node to Master
 *
 * @return Error code
 */
enum RemDbError remdb_ha_promote_to_master(void);

/**
 * @brief Demote current node to Slave
 *
 * @return Error code
 */
enum RemDbError remdb_ha_demote_to_slave(void);

/**
 * @brief Check HA status
 *
 * @return Error code
 */
enum RemDbError remdb_ha_check_status(void);

#ifdef __cplusplus
}
#endif

#endif /* REMDB_H */
