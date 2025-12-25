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
 */
enum RemDbDataType {
    REMDB_TYPE_INT8 = 0,
    REMDB_TYPE_INT16 = 1,
    REMDB_TYPE_INT32 = 2,
    REMDB_TYPE_INT64 = 3,
    REMDB_TYPE_FLOAT32 = 4,
    REMDB_TYPE_FLOAT64 = 5,
    REMDB_TYPE_BOOL = 6,
    REMDB_TYPE_TIMESTAMP = 7,
    REMDB_TYPE_STRING = 8,
};

/**
 * @brief Maximum string length supported by RemDB
 */
#define REMDB_MAX_STRING_LEN 64

/**
 * @brief Universal value type for RemDB
 */
typedef union RemDbValue {
    int8_t int8;
    int16_t int16;
    int32_t int32;
    int64_t int64;
    float float32;
    double float64;
    uint8_t bool;
    uint64_t timestamp;
    uint8_t string[REMDB_MAX_STRING_LEN];
} RemDbValue;

/**
 * @brief Field definition
 */
typedef struct RemDbFieldDef {
    const char* name;
    enum RemDbDataType data_type;
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
 * @brief Database configuration
 */
typedef struct RemDbConfig {
    const RemDbTableDef* tables;
    size_t tables_count;
    size_t total_memory;
    uint8_t low_power_mode_supported;
    int32_t low_power_max_records;
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

#ifdef __cplusplus
}
#endif

#endif /* REMDB_H */
