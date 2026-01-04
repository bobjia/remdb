# SQL Language Documentation

## 1. 支持的SQL数据类型

RemDB支持以下SQL数据类型：

| SQL类型 | 内部类型 | 描述 |
|---------|----------|------|
| `INTEGER` | Int32/Int64/UInt32/UInt64 | 整数类型，根据实际使用自动选择合适的内部类型 |
| `REAL` | Float32/Float64 | 浮点数类型 |
| `TEXT` | String | 字符串类型，最大长度64字节 |
| `BOOLEAN` | Bool | 布尔类型，存储为0或1 |
| `TIMESTAMP` | Timestamp | 时间戳类型，以毫秒为单位存储 |

### 1.1 数据类型映射

| 内部类型 | SQL类型 | 大小（字节） |
|----------|---------|--------------|
| UInt8 | INTEGER | 1 |
| UInt16 | INTEGER | 2 |
| UInt32 | INTEGER | 4 |
| UInt64 | INTEGER | 8 |
| Int8 | INTEGER | 1 |
| Int16 | INTEGER | 2 |
| Int32 | INTEGER | 4 |
| Int64 | INTEGER | 8 |
| Float32 | REAL | 4 |
| Float64 | REAL | 8 |
| Bool | INTEGER | 1 |
| Timestamp | INTEGER | 8 |
| String | TEXT | 64 |

## 2. 支持的SQL语法

### 2.1 SELECT语句

```sql
SELECT [column1, column2, ... | *]
FROM table_name
[WHERE condition]
[ORDER BY column [ASC | DESC]]
[LIMIT number];
```

#### 示例

```sql
SELECT * FROM users;
SELECT id, name FROM users WHERE age > 18 ORDER BY id DESC LIMIT 10;
```

### 2.2 INSERT语句

```sql
INSERT INTO table_name [(column1, column2, ...)]
VALUES (value1, value2, ...);
```

#### 示例

```sql
INSERT INTO users (name, age) VALUES ('Alice', 25);
INSERT INTO users VALUES (1, 'Bob', 30);
```

### 2.3 UPDATE语句

```sql
UPDATE table_name
SET column1 = value1, column2 = value2, ...
[WHERE condition];
```

#### 示例

```sql
UPDATE users SET age = 26 WHERE name = 'Alice';
```

### 2.4 DELETE语句

```sql
DELETE FROM table_name
[WHERE condition];
```

#### 示例

```sql
DELETE FROM users WHERE id = 1;
```

### 2.5 CREATE TABLE语句

```sql
CREATE TABLE table_name (
    column1 datatype [constraints],
    column2 datatype [constraints],
    ...
);
```

#### 支持的约束

- `PRIMARY KEY`：主键约束
- `NOT NULL`：非空约束
- `UNIQUE`：唯一约束
- `AUTOINCREMENT`/`AUTO_INCREMENT`：自增约束
- `DEFAULT value`：默认值约束

#### 示例

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    age INTEGER DEFAULT 0,
    email TEXT UNIQUE
);
```

### 2.6 CREATE INDEX语句

```sql
CREATE INDEX index_name ON table_name (column) [USING index_type];
```

#### 示例

```sql
CREATE INDEX idx_users_age ON users (age);
CREATE INDEX idx_users_name ON users (name) USING BTREE;
```

### 2.7 DESCRIBE TABLE语句

```sql
DESCRIBE table_name;
-- 或
DESCRIBE TABLE table_name;
```

#### 示例

```sql
DESCRIBE users;
```

### 2.8 CREATE TIMESERIES TABLE语句

RemDB支持专门的时序表创建语法，提供时序数据的优化存储和查询能力。

```sql
CREATE TIMESERIES TABLE table_name (
    time_field TIMESTAMP,
    value_field REAL,
    tag_field1 TEXT,
    tag_field2 INTEGER,
    ...
) [WITH COMPRESSION = (algorithm='algorithm_name', enabled=true)] [, WITH TTL = 'duration'];
```

详细的语法说明和示例请参见[4.2 CREATE TIMESERIES TABLE语句](#42-create-timeseries-table-语句)。

### 2.9 支持的运算符

#### 比较运算符

- `=`：等于
- `<>`/`!=`：不等于
- `>`：大于
- `>=`：大于等于
- `<`：小于
- `<=`：小于等于

#### 逻辑运算符

- `AND`：逻辑与
- `OR`：逻辑或

## 3. 支持的索引

### 3.1 索引类型

RemDB支持以下索引类型：

| 索引类型 | 适用场景 | 特点 |
|---------|----------|------|
| 哈希索引 | 主键 | 仅用于主键，支持快速精确查找 |
| 有序数组索引 | 辅助索引 | 适合小规模数据，支持快速范围查找 |
| B-Tree索引 | 辅助索引 | 适合大规模数据，支持高效的精确查找和范围查找 |
| T-Tree索引 | 辅助索引 | 适合内存数据库，支持高效的插入、删除和查找操作 |

### 3.2 索引功能

- **精确查找**：根据键值精确查找记录
- **范围查找**：查找指定范围内的所有记录
- **索引统计**：支持查看索引的访问次数、命中次数、大小和项数量

### 3.3 索引创建示例

```sql
-- 创建B-Tree索引
CREATE INDEX idx_orders_timestamp ON orders (timestamp) USING BTREE;

-- 创建T-Tree索引
CREATE INDEX idx_orders_amount ON orders (amount) USING TTREE;
```

## 4. 时序相关功能

### 4.1 时间戳数据类型

RemDB提供`TIMESTAMP`数据类型用于存储时间戳，以毫秒为单位。

```sql
CREATE TABLE sensor_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sensor_id INTEGER NOT NULL,
    value REAL NOT NULL,
    timestamp TIMESTAMP NOT NULL
);
```

### 4.2 CREATE TIMESERIES TABLE语句

RemDB支持专门的时序表创建语法，提供时序数据的优化存储和查询能力。

```sql
CREATE TIMESERIES TABLE table_name (
    time_field TIMESTAMP,
    value_field REAL,
    tag_field1 TEXT,
    tag_field2 INTEGER,
    ...
) [WITH COMPRESSION = (algorithm='algorithm_name', enabled=true)] [, WITH TTL = 'duration'];
```

#### 语法说明

- `time_field`：必须是`TIMESTAMP`类型，用于存储时间戳
- `value_field`：必须是数值类型（`REAL`、`INTEGER`等），用于存储时序数据的值
- `tag_field`：可选的标签字段，用于标识和查询时序数据
- `WITH COMPRESSION`：可选，指定写入时的压缩算法
  - `algorithm`：支持的压缩算法：`none`、`delta`、`runlength`、`delta-runlength`、`delta-delta`
  - `enabled`：是否启用压缩，默认为`true`
- `WITH TTL`：可选，定义数据存活时间，过期数据块可被自动清理
  - `duration`：时间持续时间，格式为`'30 days'`、`'72 hours'`等

#### 示例

```sql
-- 创建带有delta-delta压缩和30天TTL的时序表
CREATE TIMESERIES TABLE test_ts (
    ts TIMESTAMP,
    value FLOAT64,
    tag1 VARCHAR(20),
    tag2 INT
) WITH COMPRESSION = (algorithm='delta-delta', enabled=true), WITH TTL = '30 days';

-- 创建带有delta压缩和7天TTL的时序表
CREATE TIMESERIES TABLE sensor_data (
    timestamp TIMESTAMP,
    temperature FLOAT64,
    location VARCHAR(50)
) WITH COMPRESSION = (algorithm='delta', enabled=true), WITH TTL = '7 days';

-- 创建带有runlength压缩和1天TTL的时序表
CREATE TIMESERIES TABLE metrics (
    time TIMESTAMP,
    value DOUBLE,
    device_id VARCHAR(30),
    type VARCHAR(20)
) WITH COMPRESSION = (algorithm='runlength', enabled=true), WITH TTL = '1 day';
```

### 4.3 时间相关查询

```sql
-- 查询特定时间范围内的数据
SELECT * FROM sensor_data WHERE timestamp BETWEEN 1609459200000 AND 1609545600000;

-- 查询最近的数据
SELECT * FROM sensor_data ORDER BY timestamp DESC LIMIT 100;

-- 查询特定传感器最近的数据
SELECT * FROM sensor_data WHERE sensor_id = 1 ORDER BY timestamp DESC LIMIT 50;
```

### 4.4 时间辅助功能

RemDB内部提供了丰富的时间辅助函数：

- 时间单位转换：秒 ↔ 毫秒 ↔ 微秒 ↔ 纳秒
- 时间差计算
- 时间范围检查
- 当前时间戳获取（需要std特性）

## 5. 示例：时序数据应用

```sql
-- 创建时序数据表
CREATE TABLE metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT NOT NULL,
    value REAL NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    tags TEXT
);

-- 创建时间戳索引
CREATE INDEX idx_metrics_timestamp ON metrics (timestamp) USING BTREE;

-- 插入时序数据
INSERT INTO metrics (metric_name, value, timestamp, tags) VALUES ('cpu_usage', 0.75, 1609459200000, 'host=server1');
INSERT INTO metrics (metric_name, value, timestamp, tags) VALUES ('cpu_usage', 0.82, 1609459260000, 'host=server1');
INSERT INTO metrics (metric_name, value, timestamp, tags) VALUES ('mem_usage', 0.65, 1609459200000, 'host=server1');

-- 查询最近的CPU使用率
SELECT * FROM metrics WHERE metric_name = 'cpu_usage' ORDER BY timestamp DESC LIMIT 10;

-- 查询特定时间范围内的内存使用率
SELECT * FROM metrics WHERE metric_name = 'mem_usage' AND timestamp BETWEEN 1609459200000 AND 1609459320000;
```

## 6. 注意事项

1. 字符串类型最大长度为64字节，超过将被截断
2. 主键必须是唯一的，且只能有一个
3. 自增列只能用于整数类型
4. 索引键最大长度为64字节
5. WHERE子句目前只支持简单的比较条件，复杂条件支持有限
6. ORDER BY子句目前只支持单个字段排序
7. 时序表必须包含一个TIMESTAMP类型的时间字段和一个数值类型的值字段
8. 时序表支持的压缩算法：`none`、`delta`、`runlength`、`delta-runlength`、`delta-delta`
9. 时序表的TTL配置用于自动清理过期数据块，单位支持天、小时、分钟、秒
10. 时序表的WITH子句只能用于CREATE TIMESERIES TABLE语句，不支持普通表

## 7. 不支持的SQL特性

- JOIN操作
- 子查询
- GROUP BY和聚合函数
- DROP TABLE和ALTER TABLE
- 事务（部分支持）
- 视图和存储过程
- 外键约束
- LIKE运算符

## 8. 示例：完整的时序数据应用

```sql
-- 创建数据库和表
CREATE TABLE sensor_readings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sensor_id INTEGER NOT NULL,
    temperature REAL NOT NULL,
    humidity REAL NOT NULL,
    pressure REAL NOT NULL,
    timestamp TIMESTAMP NOT NULL
);

-- 创建索引
CREATE INDEX idx_sensor_timestamp ON sensor_readings (timestamp) USING BTREE;
CREATE INDEX idx_sensor_id ON sensor_readings (sensor_id) USING SORTED_ARRAY;

-- 插入测试数据
INSERT INTO sensor_readings (sensor_id, temperature, humidity, pressure, timestamp) VALUES (1, 23.5, 45.2, 1013.25, 1609459200000);
INSERT INTO sensor_readings (sensor_id, temperature, humidity, pressure, timestamp) VALUES (1, 23.6, 45.3, 1013.20, 1609459260000);
INSERT INTO sensor_readings (sensor_id, temperature, humidity, pressure, timestamp) VALUES (1, 23.7, 45.1, 1013.15, 1609459320000);
INSERT INTO sensor_readings (sensor_id, temperature, humidity, pressure, timestamp) VALUES (2, 22.8, 48.5, 1012.95, 1609459200000);
INSERT INTO sensor_readings (sensor_id, temperature, humidity, pressure, timestamp) VALUES (2, 22.9, 48.3, 1012.90, 1609459260000);

-- 查询传感器1的所有数据
SELECT * FROM sensor_readings WHERE sensor_id = 1;

-- 查询最近的10条数据
SELECT * FROM sensor_readings ORDER BY timestamp DESC LIMIT 10;

-- 查询特定时间范围内的数据
SELECT * FROM sensor_readings WHERE timestamp BETWEEN 1609459200000 AND 1609459300000;

-- 查询温度高于23.5的数据
SELECT * FROM sensor_readings WHERE temperature > 23.5;

-- 查询传感器1在特定时间范围内的温度数据，按时间降序排序
SELECT timestamp, temperature FROM sensor_readings WHERE sensor_id = 1 AND timestamp BETWEEN 1609459200000 AND 1609459320000 ORDER BY timestamp DESC;
```

# 总结

RemDB提供了轻量级的SQL支持，适合嵌入式系统和边缘计算场景。它支持基本的SQL操作，包括SELECT、INSERT、UPDATE、DELETE、CREATE TABLE和CREATE INDEX，以及时序数据相关功能。虽然不支持复杂的SQL特性如JOIN和子查询，但它提供了高效的索引机制和时序数据处理能力，适合对性能要求较高的嵌入式应用。