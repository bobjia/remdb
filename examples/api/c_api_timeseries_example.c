#include <stdio.h>
#include <stdint.h>
#include <time.h>
#include "../include/remdb.h"

// 定义时序表字段
static const RemDbFieldDef timeseries_fields[] = {
    {"id", REMDB_TYPE_UINT64, sizeof(uint64_t), 0},
    {"timestamp", REMDB_TYPE_TIMESTAMP, sizeof(uint64_t), sizeof(uint64_t)},
    {"value", REMDB_TYPE_FLOAT64, sizeof(double), sizeof(uint64_t) * 2},
    {"tag1", REMDB_TYPE_UINT32, sizeof(uint32_t), sizeof(uint64_t) * 2 + sizeof(double)},
    {"tag2", REMDB_TYPE_UINT32, sizeof(uint32_t), sizeof(uint64_t) * 2 + sizeof(double) + sizeof(uint32_t)},
};

// 定义标签字段索引
static const size_t tag_fields[] = {3, 4}; // tag1和tag2的索引

// 定义时序表配置
static const RemDbTimeSeriesConfig ts_config = {
    .partition_duration_secs = 3600,  // 1小时
    .retention_period_secs = 86400,    // 1天
    .compression = REMDB_COMPRESSION_DELTA_RUN_LENGTH,
    .max_partitions = 1000,
};

// 定义时序表
static const RemDbTimeSeriesTableDef timeseries_table_def = {
    .id = 1,
    .name = "sensor_data",
    .fields = timeseries_fields,
    .fields_count = sizeof(timeseries_fields) / sizeof(RemDbFieldDef),
    .primary_key = 0, // id是主键
    .secondary_index = -1, // 没有辅助索引
    .record_size = sizeof(uint64_t) * 2 + sizeof(double) + sizeof(uint32_t) * 2,
    .max_records = 1000000,
    .time_field = 1, // timestamp是时间字段
    .value_field = 2, // value是值字段
    .tag_fields = tag_fields,
    .tag_fields_count = sizeof(tag_fields) / sizeof(size_t),
    .config = ts_config,
};

// 数据库配置
static const RemDbConfig config = {
    .tables = NULL,
    .tables_count = 0,
    .time_series_tables = &timeseries_table_def,
    .time_series_tables_count = 1,
    .total_memory = 1024 * 1024 * 1024, // 1GB
    .low_power_mode_supported = 0,
    .low_power_max_records = -1,
};

int main() {
    RemDbHandle handle;
    enum RemDbError error;
    
    // 初始化数据库
    error = remdb_init_global(&config, &handle);
    if (error != REMDB_SUCCESS) {
        printf("Failed to initialize database: %d\n", error);
        return 1;
    }
    
    printf("Database initialized successfully\n");
    
    // 获取时序表ID
    size_t table_id;
    error = remdb_time_series_table_get_by_name(handle, "sensor_data", &table_id);
    if (error != REMDB_SUCCESS) {
        printf("Failed to get time series table: %d\n", error);
        return 1;
    }
    
    printf("Got time series table ID: %zu\n", table_id);
    
    // 生成测试数据
    const int num_records = 10;
    RemDbTimeSeriesRecord records[num_records];
    
    time_t now = time(NULL);
    for (int i = 0; i < num_records; i++) {
        records[i].timestamp = now + i;
        records[i].value = 25.0 + i * 0.5;
        records[i].tag_count = 2;
        records[i].tags[0] = 100;
        records[i].tags[1] = 200 + i;
    }
    
    // 批量写入时序数据
    size_t written;
    error = remdb_time_series_batch_write(handle, table_id, records, num_records, &written);
    if (error != REMDB_SUCCESS) {
        printf("Failed to write time series data: %d\n", error);
        return 1;
    }
    
    printf("Written %zu records to time series table\n", written);
    
    // 查询时序数据
    RemDbTimeSeriesRecord result_buffer[20];
    size_t result_count;
    
    error = remdb_time_series_query(handle, table_id, now, now + num_records, result_buffer, 20, &result_count);
    if (error != REMDB_SUCCESS) {
        printf("Failed to query time series data: %d\n", error);
        return 1;
    }
    
    printf("Query returned %zu records\n", result_count);
    
    // 打印查询结果
    for (size_t i = 0; i < result_count; i++) {
        printf("Record %zu: timestamp=%llu, value=%.2f, tag_count=%u, tag1=%llu, tag2=%llu\n",
               i,
               result_buffer[i].timestamp,
               result_buffer[i].value,
               result_buffer[i].tag_count,
               result_buffer[i].tags[0],
               result_buffer[i].tags[1]);
    }
    
    // 导出DDL
    printf("\nExporting DDL...\n");
    error = remdb_export_ddl(handle, "timeseries_ddl.sql");
    if (error != REMDB_SUCCESS) {
        printf("Failed to export DDL: %d\n", error);
    } else {
        printf("DDL exported successfully to 'timeseries_ddl.sql'\n");
    }
    
    // 导出数据
    printf("Exporting data...\n");
    error = remdb_export_data(handle, "timeseries_data.sql");
    if (error != REMDB_SUCCESS) {
        printf("Failed to export data: %d\n", error);
    } else {
        printf("Data exported successfully to 'timeseries_data.sql'\n");
    }
    
    // SQL查询时序表
    printf("\nSQL Query Example...\n");
    RemDbResultSet* result_set = NULL;
    error = remdb_sql_query(handle, "SELECT id, timestamp, value, tag1, tag2 FROM sensor_data WHERE tag1 = 100", &result_set);
    if (error != REMDB_SUCCESS) {
        printf("Failed to execute SQL query: %d\n", error);
    } else {
        printf("SQL query executed successfully!\n");
        printf("Query results: %zu rows\n", result_set->rows_count);
        printf("Columns: %zu\n", result_set->columns_count);
        
        // 打印列名
        printf("Column names: ");
        for (size_t i = 0; i < result_set->columns_count; i++) {
            const char* column_name = *(result_set->columns + i);
            printf("%s", column_name);
            if (i < result_set->columns_count - 1) {
                printf(", ");
            }
        }
        printf("\n\n");
        
        // 打印行数据
        for (size_t i = 0; i < result_set->rows_count; i++) {
            const RemDbResultRow* row = &result_set->rows[i];
            printf("Row %zu: ", i + 1);
            
            for (size_t j = 0; j < row->values_count; j++) {
                const RemDbTypedValue* value = &row->values[j];
                
                // 根据数据类型打印值
                switch (value->data_type) {
                    case REMDB_TYPE_UINT64:
                        printf("%llu", value->value.u64);
                        break;
                    case REMDB_TYPE_TIMESTAMP:
                        printf("%llu", value->value.u64);
                        break;
                    case REMDB_TYPE_FLOAT64:
                        printf("%.2f", value->value.float64);
                        break;
                    case REMDB_TYPE_UINT32:
                        printf("%u", value->value.u32);
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
        
        // 释放结果集
        error = remdb_free_result_set(result_set);
        if (error != REMDB_SUCCESS) {
            printf("Failed to free result set: %d\n", error);
        } else {
            printf("\nResult set freed successfully\n");
        }
    }
    
    // 执行查询示例
    printf("\nExecute Query Example...\n");
    
    // 定义要查询的列
    const char* columns[] = {"id", "timestamp", "value"};
    size_t columns_count = sizeof(columns) / sizeof(columns[0]);
    
    // 执行查询
    error = remdb_execute_query(handle, "sensor_data", columns, columns_count, "tag2 > 200", 5, &result_set);
    if (error != REMDB_SUCCESS) {
        printf("Failed to execute query: %d\n", error);
    } else {
        printf("Query executed successfully! %zu rows returned\n", result_set->rows_count);
        
        // 释放结果集
        error = remdb_free_result_set(result_set);
        if (error != REMDB_SUCCESS) {
            printf("Failed to free result set: %d\n", error);
        }
    }
    
    printf("\nC API time series example completed successfully\n");
    
    return 0;
}