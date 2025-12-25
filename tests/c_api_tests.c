/*
 * RemDB C API Unit Tests
 *
 * This file contains unit tests for the RemDB C API,
 * covering basic functionality, edge cases, and error handling.
 *
 * Compile with:
 * gcc -o c_api_tests c_api_tests.c -lremdb -L. -I../include
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "remdb.h"

// Define a simple table structure for testing
typedef struct TestRecord {
    int32_t id;
    char name[16];
    float value;
} TestRecord;

// Helper functions for testing
static void test_init_and_get() {
    printf("Testing init and get... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define table definition
    RemDbTableDef table = {
        .id = 0,
        .name = "test_table",
        .fields = fields,
        .fields_count = fields_count,
        .primary_key = 0,
        .secondary_index = -1,
        .record_size = sizeof(TestRecord),
        .max_records = 100
    };

    // Define database configuration
    RemDbTableDef tables[] = { table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 1,
        .low_power_max_records = 50
    };

    // Test initialization
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    assert(handle != NULL);
    
    // Test get global instance
    RemDbHandle handle2 = NULL;
    err = remdb_get_global(&handle2);
    assert(err == REMDB_SUCCESS);
    assert(handle2 == handle);
    
    printf("✓\n");
}

static void test_table_operations() {
    printf("Testing table operations... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define table definition
    RemDbTableDef table = {
        .id = 0,
        .name = "test_table",
        .fields = fields,
        .fields_count = fields_count,
        .primary_key = 0,
        .secondary_index = -1,
        .record_size = sizeof(TestRecord),
        .max_records = 100
    };

    // Define database configuration
    RemDbTableDef tables[] = { table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 1,
        .low_power_max_records = 50
    };

    // Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Test insert
    TestRecord record = { .id = 1, .name = "test", .value = 3.14f };
    err = remdb_table_insert(handle, 0, &record);
    assert(err == REMDB_SUCCESS);
    
    // Test get record count
    size_t count = 0;
    err = remdb_table_get_record_count(handle, 0, &count);
    assert(err == REMDB_SUCCESS);
    assert(count == 1);
    
    // Test get
    TestRecord retrieved;
    RemDbValue key;
    key.int32 = 1;
    err = remdb_table_get(handle, 0, &key, &retrieved);
    assert(err == REMDB_SUCCESS);
    assert(retrieved.id == record.id);
    assert(strcmp(retrieved.name, record.name) == 0);
    assert(retrieved.value == record.value);
    
    // Test update
    TestRecord updated = { .id = 1, .name = "updated", .value = 6.28f };
    err = remdb_table_update(handle, 0, &key, &updated);
    assert(err == REMDB_SUCCESS);
    
    // Test get updated record
    err = remdb_table_get(handle, 0, &key, &retrieved);
    assert(err == REMDB_SUCCESS);
    assert(retrieved.id == updated.id);
    assert(strcmp(retrieved.name, updated.name) == 0);
    assert(retrieved.value == updated.value);
    
    // Test delete
    err = remdb_table_delete(handle, 0, &key);
    assert(err == REMDB_SUCCESS);
    
    // Test get after delete
    err = remdb_table_get(handle, 0, &key, &retrieved);
    assert(err == REMDB_RECORD_NOT_FOUND);
    
    // Test record count after delete
    err = remdb_table_get_record_count(handle, 0, &count);
    assert(err == REMDB_SUCCESS);
    assert(count == 0);
    
    printf("✓\n");
}

static void test_transactions() {
    printf("Testing transactions... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define table definition
    RemDbTableDef table = {
        .id = 0,
        .name = "test_table",
        .fields = fields,
        .fields_count = fields_count,
        .primary_key = 0,
        .secondary_index = -1,
        .record_size = sizeof(TestRecord),
        .max_records = 100
    };

    // Define database configuration
    RemDbTableDef tables[] = { table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 1,
        .low_power_max_records = 50
    };

    // Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Test commit transaction
    err = remdb_begin_transaction(handle, REMDB_TX_WRITE, REMDB_ISO_READ_COMMITTED);
    assert(err == REMDB_SUCCESS);
    
    TestRecord record1 = { .id = 1, .name = "tx_test", .value = 1.0f };
    err = remdb_table_insert(handle, 0, &record1);
    assert(err == REMDB_SUCCESS);
    
    err = remdb_commit_transaction(handle);
    assert(err == REMDB_SUCCESS);
    
    size_t count = 0;
    err = remdb_table_get_record_count(handle, 0, &count);
    assert(err == REMDB_SUCCESS);
    assert(count == 1);
    
    // Test rollback transaction
    err = remdb_begin_transaction(handle, REMDB_TX_WRITE, REMDB_ISO_READ_COMMITTED);
    assert(err == REMDB_SUCCESS);
    
    TestRecord record2 = { .id = 2, .name = "tx_rollback", .value = 2.0f };
    err = remdb_table_insert(handle, 0, &record2);
    assert(err == REMDB_SUCCESS);
    
    err = remdb_rollback_transaction(handle);
    assert(err == REMDB_SUCCESS);
    
    // Check that record2 was not inserted
    err = remdb_table_get_record_count(handle, 0, &count);
    assert(err == REMDB_SUCCESS);
    assert(count == 1);
    
    // Test duplicate key in transaction
    err = remdb_begin_transaction(handle, REMDB_TX_WRITE, REMDB_ISO_READ_COMMITTED);
    assert(err == REMDB_SUCCESS);
    
    TestRecord duplicate = { .id = 1, .name = "duplicate", .value = 3.0f };
    err = remdb_table_insert(handle, 0, &duplicate);
    assert(err == REMDB_DUPLICATE_KEY);
    
    err = remdb_rollback_transaction(handle);
    assert(err == REMDB_SUCCESS);
    
    printf("✓\n");
}

static void test_low_power_mode() {
    printf("Testing low power mode... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define table definition
    RemDbTableDef table = {
        .id = 0,
        .name = "test_table",
        .fields = fields,
        .fields_count = fields_count,
        .primary_key = 0,
        .secondary_index = -1,
        .record_size = sizeof(TestRecord),
        .max_records = 100
    };

    // Define database configuration
    RemDbTableDef tables[] = { table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 1,
        .low_power_max_records = 50
    };

    // Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Test is_low_power_mode (should be false initially)
    uint8_t is_low_power = 1;
    err = remdb_is_low_power_mode(handle, &is_low_power);
    assert(err == REMDB_SUCCESS);
    assert(is_low_power == 0);
    
    // Test enter_low_power_mode
    err = remdb_enter_low_power_mode(handle);
    assert(err == REMDB_SUCCESS);
    
    // Test is_low_power_mode (should be true now)
    err = remdb_is_low_power_mode(handle, &is_low_power);
    assert(err == REMDB_SUCCESS);
    assert(is_low_power == 1);
    
    // Test exit_low_power_mode
    err = remdb_exit_low_power_mode(handle);
    assert(err == REMDB_SUCCESS);
    
    // Test is_low_power_mode (should be false again)
    err = remdb_is_low_power_mode(handle, &is_low_power);
    assert(err == REMDB_SUCCESS);
    assert(is_low_power == 0);
    
    printf("✓\n");
}

static void test_metrics_and_health() {
    printf("Testing metrics and health... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define table definition
    RemDbTableDef table = {
        .id = 0,
        .name = "test_table",
        .fields = fields,
        .fields_count = fields_count,
        .primary_key = 0,
        .secondary_index = -1,
        .record_size = sizeof(TestRecord),
        .max_records = 100
    };

    // Define database configuration
    RemDbTableDef tables[] = { table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 1,
        .low_power_max_records = 50
    };

    // Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Test metrics snapshot
    RemDbMetricsSnapshot snapshot;
    err = remdb_get_metrics_snapshot(handle, &snapshot);
    assert(err == REMDB_SUCCESS);
    assert(snapshot.total_memory == config.total_memory);
    
    // Test reset metrics
    err = remdb_reset_metrics(handle);
    assert(err == REMDB_SUCCESS);
    
    // Test dump metrics
    char buffer[512];
    size_t written = 0;
    err = remdb_dump_metrics(handle, buffer, sizeof(buffer), &written);
    assert(err == REMDB_SUCCESS);
    assert(written > 0);
    
    // Test health check
    RemDbHealthCheckResult health;
    err = remdb_health_check(handle, &health);
    assert(err == REMDB_SUCCESS);
    assert(health.status == REMDB_HEALTH_HEALTHY || health.status == REMDB_HEALTH_WARNING);
    
    // Test snapshot version
    uint32_t version = 0;
    err = remdb_get_snapshot_version(handle, &version);
    assert(err == REMDB_SUCCESS);
    assert(version == 0);
    
    printf("✓\n");
}

static void test_snapshot_management() {
    printf("Testing snapshot management... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define table definition
    RemDbTableDef table = {
        .id = 0,
        .name = "test_table",
        .fields = fields,
        .fields_count = fields_count,
        .primary_key = 0,
        .secondary_index = -1,
        .record_size = sizeof(TestRecord),
        .max_records = 100
    };

    // Define database configuration
    RemDbTableDef tables[] = { table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 1,
        .low_power_max_records = 50
    };

    // Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Insert a record
    TestRecord record = { .id = 1, .name = "snapshot_test", .value = 1.0f };
    err = remdb_table_insert(handle, 0, &record);
    assert(err == REMDB_SUCCESS);
    
    // Test save snapshot
    err = remdb_save_snapshot(handle, "test_snapshot");
    assert(err == REMDB_SUCCESS);
    
    // Insert another record
    TestRecord record2 = { .id = 2, .name = "before_restore", .value = 2.0f };
    err = remdb_table_insert(handle, 0, &record2);
    assert(err == REMDB_SUCCESS);
    
    // Check record count
    size_t count = 0;
    err = remdb_table_get_record_count(handle, 0, &count);
    assert(err == REMDB_SUCCESS);
    assert(count == 2);
    
    // Test restore snapshot
    err = remdb_restore_snapshot(handle, "test_snapshot");
    assert(err == REMDB_SUCCESS);
    
    // Check record count after restore
    err = remdb_table_get_record_count(handle, 0, &count);
    assert(err == REMDB_SUCCESS);
    assert(count == 1);
    
    // Test incremental snapshot
    err = remdb_save_incremental_snapshot(handle, "test_incremental_snapshot");
    assert(err == REMDB_SUCCESS);
    
    printf("✓\n");
}

static void test_error_handling() {
    printf("Testing error handling... ");
    
    // Test null pointer handling
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_get_global(&handle);
    assert(err == REMDB_CONFIG_ERROR);
    
    // Initialize database with minimal config
    RemDbTableDef tables[0] = {};
    RemDbConfig config = {
        .tables = tables,
        .tables_count = 0,
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 0,
        .low_power_max_records = -1
    };
    
    err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Test invalid table ID
    TestRecord record = { .id = 1, .name = "error_test", .value = 0.0f };
    err = remdb_table_insert(handle, 1, &record);
    assert(err == REMDB_TABLE_NOT_FOUND);
    
    // Test record not found
    RemDbValue key;
    key.int32 = 999;
    TestRecord retrieved;
    err = remdb_table_get(handle, 0, &key, &retrieved);
    assert(err == REMDB_RECORD_NOT_FOUND);
    
    printf("✓\n");
}

static void test_table_get_by_name() {
    printf("Testing table get by name... ");
    
    // Define field definitions
    RemDbFieldDef fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(TestRecord, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((TestRecord*)0)->name), offsetof(TestRecord, name) },
        { "value", REMDB_TYPE_FLOAT32, sizeof(float), offsetof(TestRecord, value) }
    };
    size_t fields_count = sizeof(fields) / sizeof(fields[0]);

    // Define multiple table definitions
    RemDbTableDef tables[] = {
        {
            .id = 0,
            .name = "table1",
            .fields = fields,
            .fields_count = fields_count,
            .primary_key = 0,
            .secondary_index = -1,
            .record_size = sizeof(TestRecord),
            .max_records = 100
        },
        {
            .id = 1,
            .name = "table2",
            .fields = fields,
            .fields_count = fields_count,
            .primary_key = 0,
            .secondary_index = -1,
            .record_size = sizeof(TestRecord),
            .max_records = 100
        }
    };

    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,
        .low_power_mode_supported = 0,
        .low_power_max_records = -1
    };

    // Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    assert(err == REMDB_SUCCESS);
    
    // Test get existing table by name
    size_t table_id = 0;
    err = remdb_table_get_by_name(handle, "table1", &table_id);
    assert(err == REMDB_SUCCESS);
    assert(table_id == 0);
    
    err = remdb_table_get_by_name(handle, "table2", &table_id);
    assert(err == REMDB_SUCCESS);
    assert(table_id == 1);
    
    // Test get non-existent table by name
    err = remdb_table_get_by_name(handle, "nonexistent_table", &table_id);
    assert(err == REMDB_TABLE_NOT_FOUND);
    
    printf("✓\n");
}

int main() {
    printf("RemDB C API Unit Tests\n");
    printf("====================\n\n");
    
    test_init_and_get();
    test_table_operations();
    test_transactions();
    test_low_power_mode();
    test_metrics_and_health();
    test_snapshot_management();
    test_error_handling();
    test_table_get_by_name();
    
    printf("\nAll tests passed successfully!\n");
    return 0;
}
