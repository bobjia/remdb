# RemDB C语言对接说明书

## 1. 概述

RemDB是一个轻量级的嵌入式内存数据库，专为资源受限的嵌入式系统设计。本说明书介绍如何使用C语言API对接RemDB数据库，实现数据的存储、查询、更新和删除等操作。

### 1.1 特性

* **轻量级设计**：优化的内存占用和CPU消耗，适合资源受限的嵌入式系统
* **实时响应**：支持事务隔离级别，确保数据一致性和实时响应能力
* **可移植性**：支持不同嵌入式硬件平台和操作系统环境
* **低功耗模式**：支持低功耗运行，延长设备电池寿命
* **快照功能**：支持完整快照和增量快照，便于数据备份和恢复
* **健康监控**：提供实时监控和健康检查功能，便于系统维护

### 1.2 适用场景

* 工业自动化控制系统
* 物联网设备数据存储
* 嵌入式监控系统
* 智能终端设备
* 实时数据采集和处理系统

## 2. 环境准备

### 2.1 编译RemDB库

1. 克隆RemDB仓库

```bash
git clone https://github.com/bobjia/remdb.git
cd remdb
```

2. 编译带有C API支持的RemDB库

```bash
cargo build --release --features c-api
```

3. 生成的静态库文件位于：
   * Windows: `target/release/remdb.lib`
   * Linux: `target/release/libremdb.a`
   * macOS: `target/release/libremdb.a`

### 2.2 包含头文件

将`include/remdb.h`头文件复制到你的项目中，并在C代码中包含：

```c
#include "remdb.h"
```

### 2.3 链接库文件

在编译时链接RemDB静态库：

```bash
gcc -o your_program your_program.c -lremdb -L. -I.
```

## 3. 核心概念

### 3.1 数据类型

RemDB支持以下数据类型：

| 数据类型 | C类型       | 描述               |
| ---- | ---------- | ---------------- |
| INT8 | int8_t     | 8位有符号整数         |
| INT16 | int16_t    | 16位有符号整数        |
| INT32 | int32_t    | 32位有符号整数        |
| INT64 | int64_t    | 64位有符号整数        |
| FLOAT32 | float    | 32位浮点数          |
| FLOAT64 | double   | 64位浮点数          |
| BOOL | uint8_t    | 布尔值（0或1）        |
| TIMESTAMP | uint64_t | 时间戳（毫秒）         |
| STRING | char[]     | 定长字符串（最大64字节）   |

### 3.2 配置

使用`RemDbConfig`结构体配置数据库：

* `tables`：表定义数组
* `tables_count`：表数量
* `time_series_tables`：时序表定义数组
* `time_series_tables_count`：时序表数量
* `total_memory`：总内存大小（字节）
* `low_power_mode_supported`：是否支持低功耗模式
* `low_power_max_records`：低功耗模式下的最大记录数
* `ha_config`：高可用性配置（可选）

#### 3.2.1 HA配置结构体

使用`RemDbHAConfig`结构体配置高可用性：

* `ha_role`：HA角色（Master、Slave或Auto）
* `replication_mode`：复制模式（异步或同步）
* `heartbeat_interval_ms`：心跳间隔（毫秒）
* `failure_detection_ms`：故障检测超时时间（毫秒）
* `sync_timeout_ms`：同步超时时间（毫秒）
* `master_address`：主节点地址（字符串形式）
* `master_port`：主节点端口
* `replication_port`：复制端口
* `node_id`：节点ID

### 3.3 表和字段

* **表**：使用`RemDbTableDef`结构体定义，包含表名、字段列表、主键和索引等信息
* **字段**：使用`RemDbFieldDef`结构体定义，包含字段名、数据类型、大小和偏移量等信息
* **主键**：每个表必须有一个主键，用于唯一标识记录
* **索引**：支持主键索引和辅助索引，提高查询效率

### 3.4 事务

* **事务类型**：支持只读事务和读写事务
* **隔离级别**：支持读未提交、读已提交、可重复读和可串行化四种隔离级别
* **事务管理**：通过`begin_transaction`、`commit_transaction`和`rollback_transaction`函数管理事务

### 3.5 快照

* **完整快照**：保存数据库的完整状态
* **增量快照**：只保存自上次快照以来变化的数据
* **快照管理**：通过`save_snapshot`、`restore_snapshot`和`save_incremental_snapshot`函数管理快照

### 3.6 SQL结果集

* **类型化值（TypedValue）**：包含数据类型和对应的值，用于表示结果集中的单个字段值
* **结果行（ResultRow）**：包含多个类型化值，用于表示结果集中的一行数据
* **结果集（ResultSet）**：包含列名和多行数据，用于表示SQL查询的结果
* **结果集管理**：通过`remdb_sql_query`、`remdb_execute_query`执行查询获取结果集，通过`remdb_free_result_set`释放结果集内存

### 3.6 高可用性(HA)

* **角色**：支持Master、Slave和Auto三种角色
* **复制模式**：支持异步和同步两种复制模式
* **心跳机制**：Master节点发送心跳，Slave节点接收并更新状态
* **故障检测**：当Slave节点长时间未收到Master心跳时，触发故障检测
* **节点提升**：支持从Slave节点提升为Master节点

## 4. API参考

### 4.1 数据库初始化

#### 4.1.1 `remdb_init_global`

**功能**：初始化全局数据库实例

**原型**：

```c
enum RemDbError remdb_init_global(const RemDbConfig* config, RemDbHandle* handle);
```

**参数**：
* `config`：数据库配置
* `handle`：输出参数，返回数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.1.2 `remdb_get_global`

**功能**：获取全局数据库实例

**原型**：

```c
enum RemDbError remdb_get_global(RemDbHandle* handle);
```

**参数**：
* `handle`：输出参数，返回数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.2 低功耗模式

#### 4.2.1 `remdb_enter_low_power_mode`

**功能**：进入低功耗模式

**原型**：

```c
enum RemDbError remdb_enter_low_power_mode(RemDbHandle handle);
```

**参数**：
* `handle`：数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.2.2 `remdb_exit_low_power_mode`

**功能**：退出低功耗模式

**原型**：

```c
enum RemDbError remdb_exit_low_power_mode(RemDbHandle handle);
```

**参数**：
* `handle`：数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.2.3 `remdb_is_low_power_mode`

**功能**：检查是否处于低功耗模式

**原型**：

```c
enum RemDbError remdb_is_low_power_mode(RemDbHandle handle, uint8_t* is_enabled);
```

**参数**：
* `handle`：数据库句柄
* `is_enabled`：输出参数，返回低功耗模式状态（0：关闭，1：开启）

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.3 事务管理

#### 4.3.1 `remdb_begin_transaction`

**功能**：开始事务

**原型**：

```c
enum RemDbError remdb_begin_transaction(RemDbHandle handle, 
                                       enum RemDbTransactionType tx_type,
                                       enum RemDbIsolationLevel isolation_level);
```

**参数**：
* `handle`：数据库句柄
* `tx_type`：事务类型（`REMDB_TX_READ`或`REMDB_TX_WRITE`）
* `isolation_level`：隔离级别

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.3.2 `remdb_commit_transaction`

**功能**：提交事务

**原型**：

```c
enum RemDbError remdb_commit_transaction(RemDbHandle handle);
```

**参数**：
* `handle`：数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.3.3 `remdb_rollback_transaction`

**功能**：回滚事务

**原型**：

```c
enum RemDbError remdb_rollback_transaction(RemDbHandle handle);
```

**参数**：
* `handle`：数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.4 快照管理

#### 4.4.1 `remdb_save_snapshot`

**功能**：保存快照到文件

**原型**：

```c
enum RemDbError remdb_save_snapshot(RemDbHandle handle, const char* path);
```

**参数**：
* `handle`：数据库句柄
* `path`：快照文件路径

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.4.2 `remdb_restore_snapshot`

**功能**：从文件恢复快照

**原型**：

```c
enum RemDbError remdb_restore_snapshot(RemDbHandle handle, const char* path);
```

**参数**：
* `handle`：数据库句柄
* `path`：快照文件路径

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.4.3 `remdb_save_incremental_snapshot`

**功能**：保存增量快照到文件

**原型**：

```c
enum RemDbError remdb_save_incremental_snapshot(RemDbHandle handle, const char* path);
```

**参数**：
* `handle`：数据库句柄
* `path`：增量快照文件路径

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.5 监控与健康检查

#### 4.5.1 `remdb_get_metrics_snapshot`

**功能**：获取指标快照

**原型**：

```c
enum RemDbError remdb_get_metrics_snapshot(RemDbHandle handle, RemDbMetricsSnapshot* snapshot);
```

**参数**：
* `handle`：数据库句柄
* `snapshot`：输出参数，返回指标快照

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.5.2 `remdb_reset_metrics`

**功能**：重置所有指标

**原型**：

```c
enum RemDbError remdb_reset_metrics(RemDbHandle handle);
```

**参数**：
* `handle`：数据库句柄

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.5.3 `remdb_health_check`

**功能**：执行健康检查

**原型**：

```c
enum RemDbError remdb_health_check(RemDbHandle handle, RemDbHealthCheckResult* result);
```

**参数**：
* `handle`：数据库句柄
* `result`：输出参数，返回健康检查结果

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.5.4 `remdb_dump_metrics`

**功能**：将指标输出到字符串

**原型**：

```c
enum RemDbError remdb_dump_metrics(RemDbHandle handle, char* buffer, size_t buffer_size, size_t* written);
```

**参数**：
* `handle`：数据库句柄
* `buffer`：输出缓冲区
* `buffer_size`：缓冲区大小
* `written`：输出参数，返回写入的字节数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.6 表操作

#### 4.6.1 `remdb_table_insert`

**功能**：向表中插入记录

**原型**：

```c
enum RemDbError remdb_table_insert(RemDbHandle handle, size_t table_id, const void* record);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：表ID
* `record`：记录数据

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.6.2 `remdb_table_get`

**功能**：从表中获取记录

**原型**：

```c
enum RemDbError remdb_table_get(RemDbHandle handle, size_t table_id, const RemDbValue* key, void* record);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：表ID
* `key`：主键值
* `record`：输出参数，返回记录数据

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.6.3 `remdb_table_update`

**功能**：更新表中的记录

**原型**：

```c
enum RemDbError remdb_table_update(RemDbHandle handle, size_t table_id, const RemDbValue* key, const void* record);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：表ID
* `key`：主键值
* `record`：新的记录数据

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.6.4 `remdb_table_delete`

**功能**：从表中删除记录

**原型**：

```c
enum RemDbError remdb_table_delete(RemDbHandle handle, size_t table_id, const RemDbValue* key);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：表ID
* `key`：主键值

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.6.5 `remdb_table_get_record_count`

**功能**：获取表的记录数

**原型**：

```c
enum RemDbError remdb_table_get_record_count(RemDbHandle handle, size_t table_id, size_t* count);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：表ID
* `count`：输出参数，返回记录数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.6.6 `remdb_table_get_by_name`

**功能**：通过名称获取表

**原型**：

```c
enum RemDbError remdb_table_get_by_name(RemDbHandle handle, const char* name, size_t* table_id);
```

**参数**：
* `handle`：数据库句柄
* `name`：表名
* `table_id`：输出参数，返回表ID

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.7 SQL查询

#### 4.7.1 `remdb_sql_query`

**功能**：执行SQL查询，返回结果集

**原型**：

```c
enum RemDbError remdb_sql_query(RemDbHandle handle, const char* sql, RemDbResultSet** result_set);
```

**参数**：
* `handle`：数据库句柄
* `sql`：SQL查询语句
* `result_set`：输出参数，返回查询结果集

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

**注意**：返回的结果集需要通过`remdb_free_result_set`释放内存

#### 4.7.2 `remdb_execute_query`

**功能**：执行查询操作，返回结果集

**原型**：

```c
enum RemDbError remdb_execute_query(RemDbHandle handle, const char* table_name, const char** columns, size_t columns_count, const char* where_clause, int32_t limit, RemDbResultSet** result_set);
```

**参数**：
* `handle`：数据库句柄
* `table_name`：表名
* `columns`：要查询的列名数组
* `columns_count`：列名数量
* `where_clause`：WHERE子句（可选）
* `limit`：结果限制数量（-1表示无限制）
* `result_set`：输出参数，返回查询结果集

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

**注意**：返回的结果集需要通过`remdb_free_result_set`释放内存

#### 4.7.3 `remdb_free_result_set`

**功能**：释放结果集内存

**原型**：

```c
enum RemDbError remdb_free_result_set(RemDbResultSet* result_set);
```

**参数**：
* `result_set`：要释放的结果集

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.8 数据操作

#### 4.8.1 `remdb_create_table`

**功能**：创建表

**原型**：

```c
enum RemDbError remdb_create_table(RemDbHandle handle, const char* table_name, const RemDbFieldDef* fields, size_t fields_count, int32_t primary_key);
```

**参数**：
* `handle`：数据库句柄
* `table_name`：表名
* `fields`：字段定义数组
* `fields_count`：字段数量
* `primary_key`：主键字段索引（-1表示无主键）

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.2 `remdb_batch_insert_record`

**功能**：批量插入记录

**原型**：

```c
enum RemDbError remdb_batch_insert_record(RemDbHandle handle, const char* table_name, const char** column_names, size_t column_names_count, const char*** records, size_t records_count, size_t values_per_record, size_t* affected_rows);
```

**参数**：
* `handle`：数据库句柄
* `table_name`：表名
* `column_names`：列名数组
* `column_names_count`：列名数量
* `records`：记录数组，每个记录是一个值数组
* `records_count`：记录数量
* `values_per_record`：每条记录的值数量
* `affected_rows`：输出参数，返回插入的记录数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.3 `remdb_update_record`

**功能**：更新记录

**原型**：

```c
enum RemDbError remdb_update_record(RemDbHandle handle, const char* table_name, const char* set_clause, const char* where_clause, size_t* affected_rows);
```

**参数**：
* `handle`：数据库句柄
* `table_name`：表名
* `set_clause`：SET子句
* `where_clause`：WHERE子句（可选）
* `affected_rows`：输出参数，返回更新的记录数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.4 `remdb_delete_record`

**功能**：删除记录

**原型**：

```c
enum RemDbError remdb_delete_record(RemDbHandle handle, const char* table_name, const char* where_clause, size_t* affected_rows);
```

**参数**：
* `handle`：数据库句柄
* `table_name`：表名
* `where_clause`：WHERE子句（可选）
* `affected_rows`：输出参数，返回删除的记录数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.5 `remdb_export_ddl`

**功能**：导出DDL文件

**原型**：

```c
enum RemDbError remdb_export_ddl(RemDbHandle handle, const char* path);
```

**参数**：
* `handle`：数据库句柄
* `path`：DDL文件路径

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.6 `remdb_export_data`

**功能**：导出数据

**原型**：

```c
enum RemDbError remdb_export_data(RemDbHandle handle, const char* path);
```

**参数**：
* `handle`：数据库句柄
* `path`：数据文件路径

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.8 时序表API

#### 4.8.1 `remdb_time_series_batch_write`

**功能**：批量写入时序数据

**原型**：

```c
enum RemDbError remdb_time_series_batch_write(RemDbHandle handle, size_t table_id, const RemDbTimeSeriesRecord* records, size_t count, size_t* written);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：时序表ID
* `records`：时序数据记录数组
* `count`：记录数量
* `written`：输出参数，返回实际写入的记录数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.2 `remdb_time_series_query`

**功能**：根据时间范围查询时序数据

**原型**：

```c
enum RemDbError remdb_time_series_query(RemDbHandle handle, size_t table_id, u64 start_time, u64 end_time, RemDbTimeSeriesRecord* buffer, size_t buffer_size, size_t* result_count);
```

**参数**：
* `handle`：数据库句柄
* `table_id`：时序表ID
* `start_time`：开始时间戳（毫秒）
* `end_time`：结束时间戳（毫秒）
* `buffer`：输出缓冲区，用于存储查询结果
* `buffer_size`：缓冲区大小（记录数）
* `result_count`：输出参数，返回实际查询到的记录数

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.8.3 `remdb_time_series_table_get_by_name`

**功能**：通过名称获取时序表

**原型**：

```c
enum RemDbError remdb_time_series_table_get_by_name(RemDbHandle handle, const char* name, size_t* table_id);
```

**参数**：
* `handle`：数据库句柄
* `name`：时序表名
* `table_id`：输出参数，返回时序表ID

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.9 发布/订阅API

#### 4.9.1 `remdb_pubsub_init`

**功能**：初始化发布/订阅系统

**原型**：

```c
enum RemDbError remdb_pubsub_init(const RemDbPubSubConfig* config);
```

**参数**：
* `config`：发布/订阅配置

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.9.2 `remdb_pubsub_subscribe`

**功能**：订阅主题

**原型**：

```c
enum RemDbError remdb_pubsub_subscribe(u16 topic_id, RemDbPubSubCallback callback, size_t* subscription_id);
```

**参数**：
* `topic_id`：主题ID
* `callback`：订阅回调函数，当收到消息时调用
* `subscription_id`：输出参数，返回订阅ID

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.9.3 `remdb_pubsub_unsubscribe`

**功能**：取消订阅

**原型**：

```c
enum RemDbError remdb_pubsub_unsubscribe(size_t subscription_id);
```

**参数**：
* `subscription_id`：订阅ID

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.9.4 `remdb_pubsub_publish`

**功能**：发布数据

**原型**：

```c
enum RemDbError remdb_pubsub_publish(u16 topic_id, const u8* data, size_t data_len);
```

**参数**：
* `topic_id`：主题ID
* `data`：要发布的数据
* `data_len`：数据长度

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.9.5 `remdb_pubsub_start_receiver`

**功能**：启动接收线程

**原型**：

```c
enum RemDbError remdb_pubsub_start_receiver(void);
```

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.9.6 `remdb_pubsub_shutdown`

**功能**：停止发布/订阅系统

**原型**：

```c
enum RemDbError remdb_pubsub_shutdown(void);
```

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

### 4.10 高可用性(HA) API

#### 4.10.1 `remdb_ha_get_role`

**功能**：获取当前HA角色

**原型**：

```c
enum RemDbError remdb_ha_get_role(enum RemDbHARole* role);
```

**参数**：
* `role`：输出参数，返回当前HA角色

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.10.2 `remdb_ha_promote_to_master`

**功能**：将当前节点提升为Master节点

**原型**：

```c
enum RemDbError remdb_ha_promote_to_master(void);
```

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.10.3 `remdb_ha_demote_to_slave`

**功能**：将当前节点降级为Slave节点

**原型**：

```c
enum RemDbError remdb_ha_demote_to_slave(void);
```

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.10.4 `remdb_ha_check_status`

**功能**：检查HA状态

**原型**：

```c
enum RemDbError remdb_ha_check_status(void);
```

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

#### 4.10.5 `remdb_ha_get_replication_mode`

**功能**：获取当前复制模式

**原型**：

```c
enum RemDbError remdb_ha_get_replication_mode(enum RemDbReplicationMode* mode);
```

**参数**：
* `mode`：输出参数，返回当前复制模式

**返回值**：
* `REMDB_SUCCESS`：成功
* 其他错误码：失败

## 5. 使用示例

### 5.1 基本示例

以下是一个基本的使用示例，演示如何初始化数据库、创建表、插入记录和查询记录：

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "remdb.h"

// 定义用户结构体
typedef struct User {
    int32_t id;
    char name[32];
    int32_t age;
} User;

int main() {
    printf("RemDB C API Example\n");
    printf("=====================\n\n");

    // 1. 定义字段定义
    RemDbFieldDef user_fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(User, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((User*)0)->name), offsetof(User, name) },
        { "age", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(User, age) }
    };
    size_t user_fields_count = sizeof(user_fields) / sizeof(user_fields[0]);

    // 2. 定义表定义
    RemDbTableDef user_table = {
        .id = 0,
        .name = "users",
        .fields = user_fields,
        .fields_count = user_fields_count,
        .primary_key = 0,  // id是主键
        .secondary_index = -1,  // 没有辅助索引
        .record_size = sizeof(User),
        .max_records = 1000
    };

    // 3. 定义数据库配置
    RemDbTableDef tables[] = { user_table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .total_memory = 1024 * 1024,  // 1 MB
        .low_power_mode_supported = 1,
        .low_power_max_records = 500
    };

    // 4. 初始化数据库
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    if (err != REMDB_SUCCESS) {
        printf("Failed to initialize database: error code %d\n", err);
        return 1;
    }
    printf("Database initialized successfully!\n\n");

    // 5. 插入记录
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

    // 6. 查询记录
    printf("\nQuerying user 1...\n");
    RemDbValue key;
    key.int32 = 1;
    User retrieved_user;
    err = remdb_table_get(handle, 0, &key, &retrieved_user);
    if (err != REMDB_SUCCESS) {
        printf("Failed to get user 1: error code %d\n", err);
    } else {
        printf("Retrieved user: %d, %s, %d\n", retrieved_user.id, retrieved_user.name, retrieved_user.age);
    }

    // 7. 更新记录
    printf("\nUpdating user 2...\n");
    User updated_user = { .id = 2, .name = "Robert", .age = 31 };
    RemDbValue update_key;
    update_key.int32 = 2;
    err = remdb_table_update(handle, 0, &update_key, &updated_user);
    if (err != REMDB_SUCCESS) {
        printf("Failed to update user 2: error code %d\n", err);
    } else {
        printf("Updated user 2 to: %d, %s, %d\n", updated_user.id, updated_user.name, updated_user.age);
    }

    // 8. 删除记录
    printf("\nDeleting user 1...\n");
    RemDbValue delete_key;
    delete_key.int32 = 1;
    err = remdb_table_delete(handle, 0, &delete_key);
    if (err != REMDB_SUCCESS) {
        printf("Failed to delete user 1: error code %d\n", err);
    } else {
        printf("Deleted user 1\n");
    }

    // 9. 获取记录数
    size_t record_count = 0;
    err = remdb_table_get_record_count(handle, 0, &record_count);
    if (err == REMDB_SUCCESS) {
        printf("\nCurrent record count: %zu\n", record_count);
    }

    // 10. 健康检查
    printf("\nPerforming health check...\n");
    RemDbHealthCheckResult health_result;
    err = remdb_health_check(handle, &health_result);
    if (err == REMDB_SUCCESS) {
        const char* status_str = NULL;
        switch (health_result.status) {
            case REMDB_HEALTH_HEALTHY:
                status_str = "Healthy";
                break;
            case REMDB_HEALTH_WARNING:
                status_str = "Warning";
                break;
            case REMDB_HEALTH_UNHEALTHY:
                status_str = "Unhealthy";
                break;
            default:
                status_str = "Unknown";
        }
        printf("Health status: %s\n", status_str);
        printf("Memory usage: %zu / %zu bytes\n", health_result.metrics.used_memory, health_result.metrics.total_memory);
    }

    printf("\nExample completed successfully!\n");
    return 0;
}
```

### 5.2 HA示例

以下是一个HA使用示例，演示如何初始化带有HA配置的数据库，以及如何使用HA相关的API：

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "remdb.h"

// 定义用户结构体
typedef struct User {
    int32_t id;
    char name[32];
    int32_t age;
} User;

int main() {
    printf("RemDB C API HA Example\n");
    printf("======================\n\n");

    // 1. 定义字段定义
    RemDbFieldDef user_fields[] = {
        { "id", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(User, id) },
        { "name", REMDB_TYPE_STRING, sizeof(((User*)0)->name), offsetof(User, name) },
        { "age", REMDB_TYPE_INT32, sizeof(int32_t), offsetof(User, age) }
    };
    size_t user_fields_count = sizeof(user_fields) / sizeof(user_fields[0]);

    // 2. 定义表定义
    RemDbTableDef user_table = {
        .id = 0,
        .name = "users",
        .fields = user_fields,
        .fields_count = user_fields_count,
        .primary_key = 0,  // id是主键
        .secondary_index = -1,  // 没有辅助索引
        .record_size = sizeof(User),
        .max_records = 1000
    };

    // 3. 定义HA配置
    RemDbHAConfig ha_config = {
        .ha_role = REMDB_HA_ROLE_AUTO,
        .replication_mode = REMDB_REPLICATION_MODE_ASYNC,
        .heartbeat_interval_ms = 1000,
        .failure_detection_ms = 5000,
        .sync_timeout_ms = 1000,
        .master_address = NULL,  // 自动模式下不需要指定主节点地址
        .master_port = 0,
        .replication_port = 5556,
        .heartbeat_port = 5557,
        .node_id = 1
    };

    // 4. 定义数据库配置
    RemDbTableDef tables[] = { user_table };
    RemDbConfig config = {
        .tables = tables,
        .tables_count = sizeof(tables) / sizeof(tables[0]),
        .time_series_tables = NULL,
        .time_series_tables_count = 0,
        .total_memory = 1024 * 1024,  // 1 MB
        .low_power_mode_supported = 1,
        .low_power_max_records = 500,
        .ha_config = &ha_config  // 启用HA配置
    };

    // 5. 初始化数据库
    RemDbHandle handle = NULL;
    enum RemDbError err = remdb_init_global(&config, &handle);
    if (err != REMDB_SUCCESS) {
        printf("Failed to initialize database: error code %d\n", err);
        return 1;
    }
    printf("Database initialized successfully!\n\n");

    // 6. 检查当前HA角色
    RemDbHARole current_role;
    err = remdb_ha_get_role(&current_role);
    if (err == REMDB_SUCCESS) {
        const char* role_str = NULL;
        switch (current_role) {
            case REMDB_HA_ROLE_MASTER:
                role_str = "Master";
                break;
            case REMDB_HA_ROLE_SLAVE:
                role_str = "Slave";
                break;
            case REMDB_HA_ROLE_AUTO:
                role_str = "Auto";
                break;
            default:
                role_str = "Unknown";
        }
        printf("Current HA Role: %s\n", role_str);
    }

    // 7. 检查当前复制模式
    RemDbReplicationMode current_mode;
    err = remdb_ha_get_replication_mode(&current_mode);
    if (err == REMDB_SUCCESS) {
        const char* mode_str = NULL;
        switch (current_mode) {
            case REMDB_REPLICATION_MODE_ASYNC:
                mode_str = "Async";
                break;
            case REMDB_REPLICATION_MODE_SYNC:
                mode_str = "Sync";
                break;
            default:
                mode_str = "Unknown";
        }
        printf("Current Replication Mode: %s\n\n", mode_str);
    }

    // 8. 插入一条测试记录（只有Master节点可以执行写操作）
    User user1 = { .id = 1, .name = "Alice", .age = 25 };
    err = remdb_table_insert(handle, 0, &user1);
    if (err != REMDB_SUCCESS) {
        printf("Failed to insert user: error code %d\n", err);
    } else {
        printf("Inserted user: %d, %s, %d\n\n", user1.id, user1.name, user1.age);
    }

    // 9. 检查HA状态
    printf("Performing HA status check...\n");
    err = remdb_ha_check_status();
    if (err == REMDB_SUCCESS) {
        printf("HA status check passed\n");
    } else {
        printf("HA status check failed: error code %d\n", err);
    }

    printf("\nHA Example completed successfully!\n");
    return 0;
}
```

### 5.3 事务示例

以下是一个事务使用示例，演示如何使用事务来保证数据一致性：

```c
// 开始事务
err = remdb_begin_transaction(handle, REMDB_TX_WRITE, REMDB_ISO_READ_COMMITTED);
if (err != REMDB_SUCCESS) {
    printf("Failed to begin transaction: error code %d\n", err);
    return 1;
}

// 执行多个操作
User user3 = { .id = 3, .name = "Charlie", .age = 35 };
err = remdb_table_insert(handle, 0, &user3);
if (err != REMDB_SUCCESS) {
    printf("Failed to insert user 3: error code %d\n", err);
    remdb_rollback_transaction(handle);
    return 1;
}

User user4 = { .id = 4, .name = "David", .age = 40 };
err = remdb_table_insert(handle, 0, &user4);
if (err != REMDB_SUCCESS) {
    printf("Failed to insert user 4: error code %d\n", err);
    remdb_rollback_transaction(handle);
    return 1;
}

// 提交事务
err = remdb_commit_transaction(handle);
if (err != REMDB_SUCCESS) {
    printf("Failed to commit transaction: error code %d\n", err);
    remdb_rollback_transaction(handle);
    return 1;
}

printf("Transaction committed successfully!\n");
```

### 5.3 快照示例

以下是一个快照使用示例，演示如何保存和恢复快照：

```c
// 保存快照
printf("Saving snapshot...\n");
err = remdb_save_snapshot(handle, "example_snapshot");
if (err != REMDB_SUCCESS) {
    printf("Failed to save snapshot: error code %d\n", err);
    return 1;
}
printf("Snapshot saved successfully!\n");

// 修改数据
printf("Modifying data...\n");
User user5 = { .id = 5, .name = "Eve", .age = 28 };
err = remdb_table_insert(handle, 0, &user5);
if (err != REMDB_SUCCESS) {
    printf("Failed to insert user 5: error code %d\n", err);
    return 1;
}

// 查看修改后的数据
record_count = 0;
err = remdb_table_get_record_count(handle, 0, &record_count);
printf("Record count after modification: %zu\n", record_count);

// 恢复快照
printf("Restoring snapshot...\n");
err = remdb_restore_snapshot(handle, "example_snapshot");
if (err != REMDB_SUCCESS) {
    printf("Failed to restore snapshot: error code %d\n", err);
    return 1;
}

// 查看恢复后的数据
record_count = 0;
err = remdb_table_get_record_count(handle, 0, &record_count);
printf("Record count after restoration: %zu\n", record_count);
```

### 5.4 SQL查询示例

以下是一个SQL查询使用示例，演示如何执行SQL查询和处理结果集：

```c
// 执行SQL查询
printf("Executing SQL query...\n");
RemDbResultSet* result_set = NULL;
err = remdb_sql_query(handle, "SELECT id, name, age FROM users WHERE age > 25", &result_set);
if (err != REMDB_SUCCESS) {
    printf("Failed to execute SQL query: error code %d\n", err);
    return 1;
}
printf("SQL query executed successfully!\n");

// 处理结果集
printf("Query results: %zu rows\n", result_set->rows_count);
printf("Columns: %zu\n", result_set->columns_count);

// 打印列名
printf("Column names: ");
for (size_t i = 0; i < result_set->columns_count; i++) {
    const char* column_name = result_set->columns[i];
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
            case REMDB_TYPE_INT32:
                printf("%d", value->value.int32);
                break;
            case REMDB_TYPE_STRING:
                printf("%s", value->value.string);
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
printf("\nFreeing result set...\n");
err = remdb_free_result_set(result_set);
if (err != REMDB_SUCCESS) {
    printf("Failed to free result set: error code %d\n", err);
    return 1;
}
printf("Result set freed successfully!\n");

// 使用remdb_execute_query执行查询
printf("\nExecuting query with remdb_execute_query...\n");
const char* columns[] = { "id", "name" };
size_t columns_count = sizeof(columns) / sizeof(columns[0]);
result_set = NULL;
err = remdb_execute_query(handle, "users", columns, columns_count, "age < 30", 10, &result_set);
if (err != REMDB_SUCCESS) {
    printf("Failed to execute query: error code %d\n", err);
    return 1;
}

printf("Query executed successfully! %zu rows returned\n", result_set->rows_count);

// 释放结果集
err = remdb_free_result_set(result_set);
if (err != REMDB_SUCCESS) {
    printf("Failed to free result set: error code %d\n", err);
    return 1;
}
```

### 5.5 批量操作示例

以下是一个批量操作使用示例，演示如何使用批量插入、更新和删除等API：

```c
// 批量插入记录
printf("\nBatch inserting records...\n");
const char* column_names[] = { "id", "name", "age" };
size_t column_names_count = sizeof(column_names) / sizeof(column_names[0]);

// 准备批量插入数据
const char* record1[] = { "6", "Frank", "32" };
const char* record2[] = { "7", "Grace", "29" };
const char* record3[] = { "8", "Henry", "35" };
const char* record4[] = { "9", "Ivy", "27" };
const char* record5[] = { "10", "Jack", "31" };
const char*** records = (const char***)malloc(5 * sizeof(const char**));
records[0] = (const char**)record1;
records[1] = (const char**)record2;
records[2] = (const char**)record3;
records[3] = (const char**)record4;
records[4] = (const char**)record5;

size_t affected_rows = 0;
err = remdb_batch_insert_record(handle, "users", column_names, column_names_count, records, 5, 3, &affected_rows);
if (err != REMDB_SUCCESS) {
    printf("Failed to batch insert records: error code %d\n", err);
    free(records);
    return 1;
}
printf("Batch inserted %zu records successfully!\n", affected_rows);
free(records);

// 更新记录
printf("\nUpdating records...\n");
affected_rows = 0;
err = remdb_update_record(handle, "users", "age = age + 1", "age > 30", &affected_rows);
if (err != REMDB_SUCCESS) {
    printf("Failed to update records: error code %d\n", err);
    return 1;
}
printf("Updated %zu records successfully!\n", affected_rows);

// 查询更新后的结果
printf("\nQuerying updated records...\n");
RemDbResultSet* result_set = NULL;
err = remdb_sql_query(handle, "SELECT id, name, age FROM users WHERE age > 30", &result_set);
if (err != REMDB_SUCCESS) {
    printf("Failed to execute SQL query: error code %d\n", err);
    return 1;
}

printf("Updated query results: %zu rows\n", result_set->rows_count);
for (size_t i = 0; i < result_set->rows_count; i++) {
    const RemDbResultRow* row = &result_set->rows[i];
    printf("Row %zu: ", i + 1);
    
    for (size_t j = 0; j < row->values_count; j++) {
        const RemDbTypedValue* value = &row->values[j];
        
        switch (value->data_type) {
            case REMDB_TYPE_INT32:
                printf("%d", value->value.int32);
                break;
            case REMDB_TYPE_STRING:
                printf("%s", value->value.string);
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

remdb_free_result_set(result_set);

// 删除记录
printf("\nDeleting records...\n");
affected_rows = 0;
err = remdb_delete_record(handle, "users", "age > 35", &affected_rows);
if (err != REMDB_SUCCESS) {
    printf("Failed to delete records: error code %d\n", err);
    return 1;
}
printf("Deleted %zu records successfully!\n", affected_rows);

// 导出DDL
printf("\nExporting DDL...\n");
err = remdb_export_ddl(handle, "exported_ddl.sql");
if (err != REMDB_SUCCESS) {
    printf("Failed to export DDL: error code %d\n", err);
    return 1;
}
printf("DDL exported successfully!\n");

// 导出数据
printf("\nExporting data...\n");
err = remdb_export_data(handle, "exported_data.sql");
if (err != REMDB_SUCCESS) {
    printf("Failed to export data: error code %d\n", err);
    return 1;
}
printf("Data exported successfully!\n");
```

## 6. 最佳实践

### 6.1 内存管理

* 根据实际需求合理配置`total_memory`，避免过度分配内存
* 对于频繁更新的数据，考虑使用更大的`max_records`来减少内存碎片
* 定期执行健康检查，监控内存使用情况

### 6.2 性能优化

* 为频繁查询的字段创建索引，提高查询效率
* 合理使用事务，避免长时间占用锁资源
* 对于批量操作，使用事务来减少I/O开销
* 在低功耗设备上，考虑使用低功耗模式

### 6.3 数据安全

* 定期保存快照，防止数据丢失
* 使用事务来保证数据一致性
* 避免在事务中执行长时间运行的操作
* 合理设置事务隔离级别，平衡一致性和性能

### 6.4 低功耗设计

* 在不需要实时响应的场景下，使用低功耗模式
* 合理设置`low_power_max_records`，控制内存使用
* 减少不必要的索引和查询操作
* 优化数据结构，减少内存占用

## 7. 常见问题

### 7.1 编译错误

**问题**：编译时出现"undefined reference to xxx"错误

**解决方案**：确保链接了RemDB库，检查库路径和链接命令是否正确

### 7.2 内存不足

**问题**：执行插入操作时返回`REMDB_OUT_OF_MEMORY`错误

**解决方案**：
* 增加`total_memory`配置
* 减少`max_records`配置
* 清理不需要的数据
* 考虑使用低功耗模式

### 7.3 性能问题

**问题**：查询或更新操作延迟较高

**解决方案**：
* 为频繁查询的字段创建索引
* 优化查询条件，减少扫描的数据量
* 合理使用事务，避免长时间占用锁资源
* 考虑使用更高效的数据结构

### 7.4 数据一致性问题

**问题**：数据更新后查询不到最新结果

**解决方案**：
* 确保事务正确提交
* 检查事务隔离级别设置
* 避免在只读事务中执行写操作
* 确保使用相同的数据库句柄进行操作

## 8. 总结

RemDB C API提供了一套完整的接口，用于在嵌入式系统中使用RemDB数据库。通过合理配置和使用这些API，可以实现高效、可靠的数据存储和管理。

在使用过程中，建议根据实际需求合理配置数据库参数，优化数据结构和查询，确保系统的性能和可靠性。同时，定期执行健康检查和快照备份，防止数据丢失和系统故障。

如需更多帮助或有任何问题，请参考RemDB官方文档或提交Issue到GitHub仓库。

## 9. 参考资料

* [RemDB GitHub仓库](https://github.com/bobjia/remdb)
* [RemDB Rust API文档](https://docs.rs/remdb)
* [嵌入式数据库设计最佳实践](https://en.wikipedia.org/wiki/Embedded_database)
* [事务处理概念](https://en.wikipedia.org/wiki/Database_transaction)
* [索引设计原则](https://en.wikipedia.org/wiki/Database_index)
