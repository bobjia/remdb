/*
 * RemDB C API Example
 *
 * This example demonstrates the basic usage of RemDB C API,
 * including database initialization, CRUD operations, transactions,
 * snapshot management, and health monitoring.
 *
 * Compile with:
 * gcc -o c_api_example c_api_example.c -lremdb -L. -I../include
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "remdb.h"

// Define a simple table structure for demonstration
typedef struct User {
    uint32_t id;
    char name[32];
    uint32_t age;
} User;

// Define field names as constants
const char* USER_ID_FIELD = "id";
const char* USER_NAME_FIELD = "name";
const char* USER_AGE_FIELD = "age";

int main() {
    printf("RemDB C API Example\n");
    printf("=====================\n\n");

    // Step 1: Define field definitions
    RemDbFieldDef user_fields[] = {
        { USER_ID_FIELD, REMDB_TYPE_UINT32, sizeof(uint32_t), offsetof(User, id) },
        { USER_NAME_FIELD, REMDB_TYPE_STRING, sizeof(((User*)0)->name), offsetof(User, name) },
        { USER_AGE_FIELD, REMDB_TYPE_UINT32, sizeof(uint32_t), offsetof(User, age) }
    };
    size_t user_fields_count = sizeof(user_fields) / sizeof(user_fields[0]);

    // Step 2: Define table definition
    RemDbTableDef user_table = {
        .id = 0,
        .name = "users",
        .fields = user_fields,
        .fields_count = user_fields_count,
        .primary_key = 0,  // id is primary key
        .secondary_index = -1,  // no secondary index
        .record_size = sizeof(User),
        .max_records = 1000
    };

    // Step 3: Define database configuration
    RemDbTableDef tables[] = { user_table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,  // 1 MB
        .low_power_mode_supported = 1,
        .low_power_max_records = 500
    };

    // Step 4: Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    if (err != REMDB_SUCCESS) {
        printf("Failed to initialize database: error code %d\n", err);
        return 1;
    }
    printf("Database initialized successfully!\n\n");

    // Step 5: Insert records
    printf("Inserting records...\n");
    
    User user1 = { .id = 1, .name = "Alice", .age = 25 };
    err = remdb_table_insert(handle, 0, &user1);
    if (err != REMDB_SUCCESS) {
        printf("Failed to insert user 1: error code %d\n", err);
    } else {
        printf("Inserted user: %d, %s, %d\n", user1.id, user1.name, user1.age);
    }

    User user2 = { .id = 2, .name = "Bob", .age = 30 };
    err = remdb_table_insert(handle, 0, &user2);
    if (err != REMDB_SUCCESS) {
        printf("Failed to insert user 2: error code %d\n", err);
    } else {
        printf("Inserted user: %d, %s, %d\n", user2.id, user2.name, user2.age);
    }

    User user3 = { .id = 3, .name = "Charlie", .age = 35 };
    err = remdb_table_insert(handle, 0, &user3);
    if (err != REMDB_SUCCESS) {
        printf("Failed to insert user 3: error code %d\n", err);
    } else {
        printf("Inserted user: %d, %s, %d\n", user3.id, user3.name, user3.age);
    }
    printf("\n");

    // Step 6: Query records
    printf("Querying records...\n");
    
    RemDbValue key;
    key.u32 = 2;
    User retrieved_user;
    
    err = remdb_table_get(handle, 0, &key, &retrieved_user);
    if (err != REMDB_SUCCESS) {
        printf("Failed to get user 2: error code %d\n", err);
    } else {
        printf("Retrieved user: %d, %s, %d\n", retrieved_user.id, retrieved_user.name, retrieved_user.age);
    }
    printf("\n");

    // Step 7: Update record
    printf("Updating record...\n");
    
    User updated_user = { .id = 2, .name = "Robert", .age = 31 };
    err = remdb_table_update(handle, 0, &key, &updated_user);
    if (err != REMDB_SUCCESS) {
        printf("Failed to update user 2: error code %d\n", err);
    } else {
        printf("Updated user 2 to: %d, %s, %d\n", updated_user.id, updated_user.name, updated_user.age);
        
        // Verify update
        err = remdb_table_get(handle, 0, &key, &retrieved_user);
        if (err == REMDB_SUCCESS) {
            printf("Verified updated user: %d, %s, %d\n", retrieved_user.id, retrieved_user.name, retrieved_user.age);
        }
    }
    printf("\n");

    // Step 8: Transaction example
    printf("Transaction example...\n");
    
    // Start transaction
    err = remdb_begin_transaction(handle, REMDB_TX_WRITE, REMDB_ISO_READ_COMMITTED);
    if (err != REMDB_SUCCESS) {
        printf("Failed to begin transaction: error code %d\n", err);
    } else {
        printf("Transaction started successfully\n");
        
        // Insert a record in transaction
        User user4 = { .id = 4, .name = "David", .age = 28 };
        err = remdb_table_insert(handle, 0, &user4);
        if (err != REMDB_SUCCESS) {
            printf("Failed to insert user 4 in transaction: error code %d\n", err);
            remdb_rollback_transaction(handle);
            printf("Transaction rolled back\n");
        } else {
            printf("Inserted user 4 in transaction: %d, %s, %d\n", user4.id, user4.name, user4.age);
            
            // Commit transaction
            err = remdb_commit_transaction(handle);
            if (err != REMDB_SUCCESS) {
                printf("Failed to commit transaction: error code %d\n", err);
                remdb_rollback_transaction(handle);
                printf("Transaction rolled back\n");
            } else {
                printf("Transaction committed successfully\n");
            }
        }
    }
    printf("\n");

    // Step 9: Get record count
    printf("Getting record count...\n");
    size_t record_count = 0;
    err = remdb_table_get_record_count(handle, 0, &record_count);
    if (err != REMDB_SUCCESS) {
        printf("Failed to get record count: error code %d\n", err);
    } else {
        printf("Current record count: %zu\n", record_count);
    }
    printf("\n");

    // Step 10: Snapshot management
    printf("Snapshot management...\n");
    
    // Save snapshot
    err = remdb_save_snapshot(handle, "example_snapshot");
    if (err != REMDB_SUCCESS) {
        printf("Failed to save snapshot: error code %d\n", err);
    } else {
        printf("Snapshot saved successfully to 'example_snapshot'\n");
    }
    
    // Delete a record
    printf("Deleting user 3...\n");
    RemDbValue delete_key;
    delete_key.u32 = 3;
    err = remdb_table_delete(handle, 0, &delete_key);
    if (err != REMDB_SUCCESS) {
        printf("Failed to delete user 3: error code %d\n", err);
    } else {
        printf("Deleted user 3\n");
        
        // Get updated record count
        err = remdb_table_get_record_count(handle, 0, &record_count);
        if (err == REMDB_SUCCESS) {
            printf("Record count after deletion: %zu\n", record_count);
        }
        
        // Restore snapshot
        printf("Restoring snapshot...\n");
        err = remdb_restore_snapshot(handle, "example_snapshot");
        if (err != REMDB_SUCCESS) {
            printf("Failed to restore snapshot: error code %d\n", err);
        } else {
            printf("Snapshot restored successfully\n");
            
            // Get record count after restoration
            err = remdb_table_get_record_count(handle, 0, &record_count);
            if (err == REMDB_SUCCESS) {
                printf("Record count after restoration: %zu\n", record_count);
            }
        }
    }
    printf("\n");

    // Step 11: Health check
    printf("Health check...\n");
    RemDbHealthCheckResult health_result;
    err = remdb_health_check(handle, &health_result);
    if (err != REMDB_SUCCESS) {
        printf("Failed to perform health check: error code %d\n", err);
    } else {
        const char* health_status_str = NULL;
        switch (health_result.status) {
            case REMDB_HEALTH_HEALTHY:
                health_status_str = "Healthy";
                break;
            case REMDB_HEALTH_WARNING:
                health_status_str = "Warning";
                break;
            case REMDB_HEALTH_UNHEALTHY:
                health_status_str = "Unhealthy";
                break;
            default:
                health_status_str = "Unknown";
        }
        printf("Health status: %s\n", health_status_str);
        printf("Health details: %s\n", health_result.details);
        printf("Memory usage: %zu / %zu bytes\n", health_result.metrics.used_memory, health_result.metrics.total_memory);
    }
    printf("\n");

    // Step 12: Dump metrics
    printf("Dumping metrics...\n");
    char metrics_buffer[1024];
    size_t written = 0;
    err = remdb_dump_metrics(handle, metrics_buffer, sizeof(metrics_buffer), &written);
    if (err != REMDB_SUCCESS) {
        printf("Failed to dump metrics: error code %d\n", err);
    } else {
        printf("Metrics:\n%s\n", metrics_buffer);
    }
    printf("\n");

    // Step 13: Get snapshot version
    printf("Getting snapshot version...\n");
    uint32_t snapshot_version = 0;
    err = remdb_get_snapshot_version(handle, &snapshot_version);
    if (err != REMDB_SUCCESS) {
        printf("Failed to get snapshot version: error code %d\n", err);
    } else {
        printf("Current snapshot version: %u\n", snapshot_version);
    }
    printf("\n");

    // Step 14: Low power mode example
    printf("Low power mode example...\n");
    uint8_t is_low_power = 0;
    err = remdb_is_low_power_mode(handle, &is_low_power);
    if (err != REMDB_SUCCESS) {
        printf("Failed to check low power mode: error code %d\n", err);
    } else {
        printf("Current low power mode status: %s\n", is_low_power ? "Enabled" : "Disabled");
        
        // Enter low power mode
        err = remdb_enter_low_power_mode(handle);
        if (err != REMDB_SUCCESS) {
            printf("Failed to enter low power mode: error code %d\n", err);
        } else {
            printf("Entered low power mode\n");
            
            // Check status again
            err = remdb_is_low_power_mode(handle, &is_low_power);
            if (err == REMDB_SUCCESS) {
                printf("Updated low power mode status: %s\n", is_low_power ? "Enabled" : "Disabled");
            }
            
            // Exit low power mode
            err = remdb_exit_low_power_mode(handle);
            if (err != REMDB_SUCCESS) {
                printf("Failed to exit low power mode: error code %d\n", err);
            } else {
                printf("Exited low power mode\n");
                
                // Check status again
                err = remdb_is_low_power_mode(handle, &is_low_power);
                if (err == REMDB_SUCCESS) {
                    printf("Final low power mode status: %s\n", is_low_power ? "Enabled" : "Disabled");
                }
            }
        }
    }
    printf("\n");

    // Step 15: SQL Query Example
    printf("SQL Query Example...\n");
    
    // Execute SQL query
    RemDbResultSet* result_set = NULL;
    err = remdb_sql_query(handle, "SELECT id, name, age FROM users WHERE age > 25", &result_set);
    if (err != REMDB_SUCCESS) {
        printf("Failed to execute SQL query: error code %d\n", err);
    } else {
        printf("SQL query executed successfully!\n");
        printf("Query results: %zu rows\n", result_set->rows_count);
        printf("Columns: %zu\n", result_set->columns_count);
        
        // Print column names
        printf("Column names: ");
        for (size_t i = 0; i < result_set->columns_count; i++) {
            const char* column_name = *(result_set->columns + i);
            printf("%s", column_name);
            if (i < result_set->columns_count - 1) {
                printf(", ");
            }
        }
        printf("\n\n");
        
        // Print rows
        for (size_t i = 0; i < result_set->rows_count; i++) {
            const RemDbResultRow* row = &result_set->rows[i];
            printf("Row %zu: ", i + 1);
            
            for (size_t j = 0; j < row->values_count; j++) {
                const RemDbTypedValue* value = &row->values[j];
                
                // Print value based on data type
                switch (value->data_type) {
                    case REMDB_TYPE_UINT32:
                        printf("%u", value->value.u32);
                        break;
                    case REMDB_TYPE_STRING:
                        printf("%s", (const char*)value->value.string);
                        break;
                    default:
                        printf("<unsupported type>");
                        break;
                }
                
                if (j < row->values_count - 1) {
                    printf(", ");
                }
            }
            printf("\n");
        }
        
        // Free result set
        err = remdb_free_result_set(result_set);
        if (err != REMDB_SUCCESS) {
            printf("Failed to free result set: error code %d\n", err);
        } else {
            printf("\nResult set freed successfully\n");
        }
    }
    printf("\n");
    
    // Step 16: Execute Query Example
    printf("Execute Query Example...\n");
    
    // Define columns to query
    const char* columns[] = {"id", "name"};
    size_t columns_count = sizeof(columns) / sizeof(columns[0]);
    
    // Execute query
    err = remdb_execute_query(handle, "users", columns, columns_count, "age < 30", 10, &result_set);
    if (err != REMDB_SUCCESS) {
        printf("Failed to execute query: error code %d\n", err);
    } else {
        printf("Query executed successfully! %zu rows returned\n", result_set->rows_count);
        
        // Free result set
        err = remdb_free_result_set(result_set);
        if (err != REMDB_SUCCESS) {
            printf("Failed to free result set: error code %d\n", err);
        }
    }
    printf("\n");
    
    // Step 17: Batch Insert Example
    printf("Batch Insert Example...\n");
    
    // Define column names for batch insert
    const char* batch_columns[] = {"id", "name", "age"};
    size_t batch_columns_count = sizeof(batch_columns) / sizeof(batch_columns[0]);
    
    // Prepare batch data
    const char* record1[] = {"5", "Eve", "29"};
    const char* record2[] = {"6", "Frank", "32"};
    const char* record3[] = {"7", "Grace", "27"};
    const char*** records = (const char***)malloc(3 * sizeof(const char**));
    records[0] = (const char**)record1;
    records[1] = (const char**)record2;
    records[2] = (const char**)record3;
    
    size_t affected_rows = 0;
    err = remdb_batch_insert_record(handle, "users", batch_columns, batch_columns_count, records, 3, 3, &affected_rows);
    if (err != REMDB_SUCCESS) {
        printf("Failed to batch insert records: error code %d\n", err);
    } else {
        printf("Batch inserted %zu records successfully!\n", affected_rows);
    }
    free(records);
    printf("\n");
    
    // Step 18: Update Record Example
    printf("Update Record Example...\n");
    
    err = remdb_update_record(handle, "users", "age = age + 1", "age > 30", &affected_rows);
    if (err != REMDB_SUCCESS) {
        printf("Failed to update records: error code %d\n", err);
    } else {
        printf("Updated %zu records successfully!\n", affected_rows);
    }
    printf("\n");
    
    // Step 19: Delete Record Example
    printf("Delete Record Example...\n");
    
    err = remdb_delete_record(handle, "users", "age < 28", &affected_rows);
    if (err != REMDB_SUCCESS) {
        printf("Failed to delete records: error code %d\n", err);
    } else {
        printf("Deleted %zu records successfully!\n", affected_rows);
    }
    printf("\n");
    
    // Step 20: Export DDL Example
    printf("Export DDL Example...\n");
    
    err = remdb_export_ddl(handle, "exported_ddl.sql");
    if (err != REMDB_SUCCESS) {
        printf("Failed to export DDL: error code %d\n", err);
    } else {
        printf("DDL exported successfully to 'exported_ddl.sql'\n");
    }
    printf("\n");
    
    // Step 21: Export Data Example
    printf("Export Data Example...\n");
    
    err = remdb_export_data(handle, "exported_data.sql");
    if (err != REMDB_SUCCESS) {
        printf("Failed to export data: error code %d\n", err);
    } else {
        printf("Data exported successfully to 'exported_data.sql'\n");
    }
    printf("\n");
    
    printf("RemDB C API Example completed successfully!\n");
    printf("========================================\n");

    return 0;
}
