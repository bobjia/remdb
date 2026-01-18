/*
 * RemDB C API Simple Example
 *
 * This is a simplified example demonstrating basic usage of RemDB C API.
 * It only includes database initialization and basic insert operations.
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
    printf("RemDB C API Simple Example\n");
    printf("=========================\n\n");

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
        .low_power_max_records = 500,
        .time_series_tables = NULL,
        .time_series_tables_count = 0
    };

    // Step 4: Initialize database
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    if (err != REMDB_SUCCESS) {
        printf("Failed to initialize database: error code %d\n", err);
        return 1;
    }
    printf("Database initialized successfully!\n\n");

    // Step 5: Insert a record
    printf("Inserting a record...\n");
    
    User user1 = { .id = 1, .name = "Alice", .age = 25 };
    err = remdb_table_insert(handle, 0, &user1);
    if (err != REMDB_SUCCESS) {
        printf("Failed to insert user 1: error code %d\n", err);
    } else {
        printf("Inserted user: %d, %s, %d\n", user1.id, user1.name, user1.age);
    }
    printf("\n");

    // Step 6: Get record count
    printf("Getting record count...\n");
    size_t record_count = 0;
    err = remdb_table_get_record_count(handle, 0, &record_count);
    if (err != REMDB_SUCCESS) {
        printf("Failed to get record count: error code %d\n", err);
    } else {
        printf("Current record count: %zu\n", record_count);
    }
    printf("\n");

    printf("RemDB C API Simple Example completed!\n");
    printf("================================\n");

    return 0;
}