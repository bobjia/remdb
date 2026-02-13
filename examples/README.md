# RemDB Examples

本目录包含 RemDB 的示例代码，按照功能分类组织。

## 目录结构

```
examples/
├── api/          # API 使用示例
├── sql/          # SQL 查询示例
├── misc/         # 其他功能示例
└── pubsub_test_system/  # 发布订阅测试系统
```

## API 示例 (api/)

这些示例展示了如何直接使用 RemDB 的 Rust API 和 C API：

| 文件 | 描述 |
|------|------|
| `basic_usage.rs` | 基本的数据库操作示例，包括表定义、记录插入、查询、更新、删除和事务 |
| `c_api_example.c` | C API 完整示例，演示数据库初始化、CRUD 操作、事务、快照管理和健康监控 |
| `c_api_simple_example.c` | C API 简单示例 |
| `c_api_timeseries_example.c` | C API 时序数据示例 |
| `composite_pk_example.rs` | 复合主键示例，展示如何创建和使用复合主键表 |
| `multiple_tables.rs` | 多表操作示例，展示如何在同一数据库中使用多个表 |
| `multi_database_example.rs` | 多数据库管理示例，展示创建、切换、关闭和删除数据库 |
| `time_series.rs` | 时序数据基本操作示例 |
| `time_series_complete.rs` | 时序数据完整示例 |
| `time_series_iot.rs` | IoT 场景时序数据示例 |
| `vector_example.rs` | 向量搜索示例，展示向量字段定义、向量索引和相似性查询 |
| `vector_distance_test.rs` | 向量距离计算测试示例 |
| `varchar_example.rs` | 变长字符串字段示例 |
| `rbac_example.rs` | RBAC 示例，展示角色和用户管理、权限授予和检查 |
| `json_example.rs` | JSON 字段示例，展示 JSON 数据插入、路径查询和函数操作 |
| `alter_table_example.rs` | ALTER TABLE 示例，展示添加、删除、修改和重命名列 |
| `batch_insert_example.rs` | 批量插入示例，展示批量插入 API 和性能对比 |
| `monitoring_example.rs` | 监控示例，展示指标获取、健康检查和指标重置 |

## SQL 示例 (sql/)

这些示例展示了如何使用 SQL 语法操作数据库：

| 文件 | 描述 |
|------|------|
| `sql_query.rs` | SQL 查询示例，包括 SELECT、INSERT、UPDATE、DELETE 语句 |
| `sql_select_advanced.rs` | 高级查询示例，包括 DISTINCT、GROUP BY、HAVING、JOIN |
| `sql_aggregate_functions.rs` | 聚合函数示例，包括 COUNT、SUM、AVG、MIN、MAX |
| `sql_table_management.rs` | 表管理示例，包括 DROP TABLE、DESCRIBE、SHOW TABLES |
| `sql_transactions.rs` | 事务示例，包括 BEGIN、COMMIT、ROLLBACK |
| `sql_time_functions.rs` | 时间函数示例，包括 TIME_BUCKET、时间范围查询 |
| `sql_vector_operations.rs` | 向量操作示例，包括向量字段、距离操作符、相似性搜索 |
| `sql_json_functions.rs` | JSON 函数示例，包括 JSON_EXTRACT、路径查询 |
| `sql_like_operator.rs` | LIKE 运算符示例，包括 % 和 _ 通配符 |
| `ddl_example.rs` | DDL（数据定义语言）示例，展示表和索引的创建 |
| `ddl_full_example.rs` | 完整的 DDL 示例 |
| `ddl_runtime_example.rs` | 运行时 DDL 示例 |
| `timeseries_ddl_core.rs` | 时序表 DDL 核心示例 |
| `timeseries_ddl_usage.rs` | 时序表 DDL 使用示例 |
| `test_auto_increment.rs` | AUTO_INCREMENT 功能测试示例 |
| `test_create_timeseries_table.rs` | 创建时序表测试示例 |
| `test_default_value.rs` | 默认值功能测试示例 |
| `test_null_fix.rs` | NULL 值处理测试示例 |
| `test_parse.rs` | SQL 解析测试示例 |
| `test_system_tables.rs` | 系统表测试示例 |

## 其他示例 (misc/)

这些示例展示了 RemDB 的其他功能：

| 文件 | 描述 |
|------|------|
| `debug_table_creation.rs` | 表创建调试示例 |
| `describe_table.rs` | 表结构描述示例 |
| `drop_table_example.rs` | 删除表示例 |
| `export_example.rs` | 数据导出示例，展示如何导出 DDL 和数据 |
| `generate_snapshot.rs` | 快照生成示例 |
| `incremental_snapshot.rs` | 增量快照示例 |
| `ha_example.rs` | 高可用主从复制示例 |
| `ha_example_master.rs` | 高可用主节点示例 |
| `ha_example_slave.rs` | 高可用从节点示例 |
| `log_example.rs` | 日志功能示例 |
| `low_power_mode.rs` | 低功耗模式示例 |
| `pubsub_example.rs` | 发布/订阅功能示例 |
| `pubsub_sql_test_server.rs` | 发布订阅 SQL 测试服务器 |
| `pubsub_test_system_client.rs` | 发布订阅测试系统客户端 |
| `pubsub_test_system_server.rs` | 发布订阅测试系统服务器 |
| `pubsub_wildcard.rs` | 发布订阅通配符订阅示例 |
| `test_max_records_config.rs` | 最大记录数配置测试 |
| `test_remdb_server.rs` | RemDB 服务器测试 |
| `test_ttl_ringbuffer.rs` | TTL 环形缓冲区测试 |

## 运行示例

### Rust 示例

```bash
cargo run --example <example_name>

# 例如：
cargo run --example basic_usage
cargo run --example sql_query
cargo run --example rbac_example
cargo run --example json_example
```

### C API 示例

#### 编译 RemDB（带 C API 支持）

```bash
cargo build --release --features c-api
```

#### 编译 C 示例

**Linux/macOS (GCC):**
```bash
cd examples/api
gcc -o c_api_example c_api_example.c -lremdb -L../../target/release -I../../include
```

**Windows (MSVC):**
```bash
cd examples\api
cl /I..\..\include c_api_example.c ..\..\target\release\remdb.lib ws2_32.lib mswsock.lib advapi32.lib ntdll.lib userenv.lib bcrypt.lib
```

#### 运行

```bash
# Linux/macOS
./c_api_example

# Windows
c_api_example.exe
```

## 功能特性

- **基本操作**: CRUD 操作、事务、索引
- **时序数据**: 时间窗口查询、聚合、压缩
- **向量搜索**: 向量索引、相似性查询、距离计算
- **高可用**: 主从复制、故障转移
- **发布订阅**: 实时数据推送、通配符订阅
- **持久化**: 快照、增量快照、WAL
- **SQL 支持**: 标准 SQL 语法、DDL/DML 操作
- **RBAC**: 基于角色的访问控制
- **JSON**: JSON 字段支持、路径查询
- **多数据库**: 数据库创建、切换、隔离
- **监控**: 指标收集、健康检查
