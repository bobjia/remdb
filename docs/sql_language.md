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
[GROUP BY column1, column2, ...]
[ORDER BY column [ASC | DESC]]
[LIMIT number];
```

#### GROUP BY子句

GROUP BY子句用于将结果集按照一个或多个列进行分组，通常与聚合函数（如COUNT、SUM、AVG等）一起使用，对每个分组进行聚合计算。

**语法**：
```sql
GROUP BY column1, column2, ...
```

**说明**：
- 可以按多个列进行分组，列之间用逗号分隔
- 分组列可以是原始列名或表达式
- GROUP BY子句通常位于WHERE子句之后，ORDER BY子句之前

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

### 2.10 函数支持

RemDB支持在SELECT语句中使用内嵌函数，包括聚合函数和窗口函数。

```sql
SELECT function_name(arg1, arg2, ...) [AS alias] FROM table_name;
```

#### 函数调用语法

- `function_name`：函数名称
- `arg1, arg2, ...`：函数参数，可以是字段名、常量或其他函数调用
- `alias`：可选的列别名

#### 示例

```sql
-- 使用聚合函数
SELECT COUNT(*) FROM sensor_data;
SELECT AVG(temperature) AS avg_temp FROM sensor_data;

-- 使用时间窗口函数
SELECT TIME_BUCKET('5m', timestamp), SUM(value) FROM sensor_data GROUP BY 1;

-- 带条件的函数调用
SELECT COUNT(*) FROM sensor_data WHERE temperature > 25;
```

#### 支持的函数列表

##### 基础统计聚合函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `COUNT` | 统计记录数量 | `*` 或字段名 | `INTEGER` | `COUNT(*)` |
| `SUM` | 计算数值总和 | 数值字段 | 数值类型 | `SUM(value)` |
| `AVG` | 计算平均值 | 数值字段 | `REAL` | `AVG(temperature)` |
| `MIN` | 计算最小值 | 数值字段 | 数值类型 | `MIN(value)` |
| `MAX` | 计算最大值 | 数值字段 | 数值类型 | `MAX(value)` |
| `VAR` | 计算总体方差 | 数值字段 | `REAL` | `VAR(temperature)` |
| `STDDEV` | 计算总体标准差 | 数值字段 | `REAL` | `STDDEV(temperature)` |
| `VAR_SAMP` | 计算样本方差 | 数值字段 | `REAL` | `VAR_SAMP(temperature)` |
| `STDDEV_SAMP` | 计算样本标准差 | 数值字段 | `REAL` | `STDDEV_SAMP(temperature)` |

##### 滑动窗口函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `MOVING_SUM` | 计算滑动窗口内的数值总和 | 数值字段, 窗口大小 | `REAL` | `MOVING_SUM(temperature, 3)` |
| `MOVING_AVERAGE` | 计算滑动窗口内的平均值 | 数值字段, 窗口大小 | `REAL` | `MOVING_AVERAGE(temperature, 3)` |

##### 字符串函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `CONCAT` | 连接多个字符串 | 字符串1, 字符串2, ... | `TEXT` | `CONCAT('Hello', ' ', 'World')` |
| `SUBSTRING` | 截取字符串 | 字符串, 起始位置, [长度] | `TEXT` | `SUBSTRING('Hello', 1, 3)` |
| `UPPER` | 转换为大写 | 字符串 | `TEXT` | `UPPER('hello')` |
| `LOWER` | 转换为小写 | 字符串 | `TEXT` | `LOWER('HELLO')` |

##### 数学函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `ABS` | 计算绝对值 | 数值 | 数值类型 | `ABS(-10)` |
| `SQRT` | 计算平方根 | 数值 | `REAL` | `SQRT(16)` |
| `POWER` | 计算幂 | 底数, 指数 | `REAL` | `POWER(2, 3)` |
| `SIN` | 计算正弦值 | 弧度 | `REAL` | `SIN(0)` |
| `COS` | 计算余弦值 | 弧度 | `REAL` | `COS(0)` |
| `LOG` | 计算自然对数 | 数值 | `REAL` | `LOG(10)` |
| `EXP` | 计算指数值 | 数值 | `REAL` | `EXP(1)` |
| `ROUND` | 四舍五入 | 数值, [小数位数] | 数值类型 | `ROUND(3.14159, 2)` |
| `CEIL` | 向上取整 | 数值 | 数值类型 | `CEIL(3.14)` |
| `FLOOR` | 向下取整 | 数值 | 数值类型 | `FLOOR(3.99)` |
| `MOD` | 计算模运算 | 被除数, 除数 | 数值类型 | `MOD(10, 3)` |

##### 时间窗口函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `TIME_BUCKET` | 将时间戳分组到指定的时间窗口 | `interval` (字符串或数值), `time_field` (TIMESTAMP), `origin` (可选，字符串或数值，默认1970-01-01 00:00:00) | `TIMESTAMP` | `TIME_BUCKET('5m', timestamp)` 或 `TIME_BUCKET('1h', timestamp, '2020-01-01')` |

##### 时间间隔格式

`TIME_BUCKET` 函数支持多种时间间隔格式：

| 格式 | 描述 | 示例 |
|------|------|------|
| 数值 | 微秒数 | `TIME_BUCKET(300000000, timestamp)` (5分钟) |
| `ns` | 纳秒 | `TIME_BUCKET('1000ns', timestamp)` |
| `us` | 微秒 | `TIME_BUCKET('1000us', timestamp)` |
| `ms` | 毫秒 | `TIME_BUCKET('500ms', timestamp)` |
| `s`/`sec`/`second` | 秒 | `TIME_BUCKET('10s', timestamp)` |
| `m`/`min`/`minute` | 分钟 | `TIME_BUCKET('5min', timestamp)` |
| `h`/`hr`/`hour` | 小时 | `TIME_BUCKET('1h', timestamp)` |
| `d`/`day` | 天 | `TIME_BUCKET('7d', timestamp)` |
| `w`/`week` | 周 | `TIME_BUCKET('2w', timestamp)` |

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

## 9. 函数使用示例

### 9.1 基础聚合函数示例

```sql
-- 统计传感器数据总数
SELECT COUNT(*) AS total_readings FROM sensor_readings;

-- 统计每个传感器的记录数
SELECT sensor_id, COUNT(*) AS reading_count FROM sensor_readings GROUP BY sensor_id;

-- 计算所有传感器的平均温度
SELECT AVG(temperature) AS avg_temp FROM sensor_readings;

-- 计算每个传感器的平均温度
SELECT sensor_id, AVG(temperature) AS avg_temp FROM sensor_readings GROUP BY sensor_id;

-- 计算温度的总和、平均值、最小值和最大值
SELECT 
    SUM(temperature) AS total_temp,
    AVG(temperature) AS avg_temp,
    MIN(temperature) AS min_temp,
    MAX(temperature) AS max_temp
FROM sensor_readings WHERE sensor_id = 1;

-- 计算温度的总体方差和标准差
SELECT 
    VAR(temperature) AS var_temp,
    STDDEV(temperature) AS stddev_temp
FROM sensor_readings WHERE sensor_id = 1;

-- 计算温度的样本方差和标准差
SELECT 
    VAR_SAMP(temperature) AS var_samp_temp,
    STDDEV_SAMP(temperature) AS stddev_samp_temp
FROM sensor_readings WHERE sensor_id = 1;

-- 结合多个统计函数
SELECT 
    COUNT(*) AS reading_count,
    AVG(temperature) AS avg_temp,
    VAR(temperature) AS var_temp,
    STDDEV(temperature) AS stddev_temp
FROM sensor_readings GROUP BY sensor_id;
```

### 9.3 滑动窗口函数示例

```sql
-- 计算温度的滑动窗口总和
SELECT MOVING_SUM(temperature, 3) AS moving_sum_temp FROM sensor_readings WHERE sensor_id = 1;

-- 计算温度的滑动窗口平均值
SELECT MOVING_AVERAGE(temperature, 3) AS moving_avg_temp FROM sensor_readings WHERE sensor_id = 1;

-- 结合多个滑动窗口函数
SELECT 
    timestamp,
    temperature,
    MOVING_SUM(temperature, 3) AS moving_sum,
    MOVING_AVERAGE(temperature, 3) AS moving_avg
FROM sensor_readings WHERE sensor_id = 1 ORDER BY timestamp;
```

### 9.4 时间窗口函数示例

```sql
-- 使用TIME_BUCKET函数按5分钟窗口分组数据
SELECT 
    TIME_BUCKET('5m', timestamp) AS time_window,
    AVG(temperature) AS avg_temp,
    COUNT(*) AS reading_count
FROM sensor_readings 
GROUP BY time_window;

-- 使用TIME_BUCKET函数按1小时窗口分组数据
SELECT 
    TIME_BUCKET('1h', timestamp) AS time_window,
    AVG(temperature) AS avg_temp,
    AVG(humidity) AS avg_humidity,
    AVG(pressure) AS avg_pressure
FROM sensor_readings 
WHERE sensor_id = 1
GROUP BY time_window
ORDER BY time_window;

-- 使用数值形式的时间间隔（5分钟 = 300000000微秒）
SELECT 
    TIME_BUCKET(300000000, timestamp) AS time_window,
    SUM(temperature) AS sum_temp
FROM sensor_readings 
GROUP BY time_window;

-- 使用不同的时间间隔单位
SELECT 
    TIME_BUCKET('1h', timestamp) AS hour_window,
    TIME_BUCKET('1d', timestamp) AS day_window,
    COUNT(*) AS reading_count
FROM sensor_readings 
GROUP BY hour_window, day_window;

-- 使用origin参数自定义时间窗口起始点
SELECT 
    TIME_BUCKET('1h', timestamp, '2024-01-01 00:30:00') AS time_window,
    AVG(temperature) AS avg_temp
FROM sensor_readings 
WHERE sensor_id = 1
GROUP BY time_window
ORDER BY time_window;
```

### 9.5 字符串函数示例

```sql
-- 连接字符串
SELECT CONCAT('Hello', ' ', 'World') AS greeting;

-- 截取字符串
SELECT SUBSTRING('Hello World', 7, 5) AS substring_result;

-- 转换为大写
SELECT UPPER('hello world') AS uppercase_result;

-- 转换为小写
SELECT LOWER('HELLO WORLD') AS lowercase_result;

-- 结合多个字符串函数
SELECT UPPER(CONCAT('Hello', ' ', 'World')) AS uppercase_greeting;

-- 在WHERE条件中使用字符串函数
SELECT * FROM users WHERE UPPER(name) = 'ALICE';
```

### 9.6 数学函数示例

```sql
-- 计算绝对值
SELECT ABS(-10) AS absolute_value;

-- 计算平方根
SELECT SQRT(16) AS square_root;

-- 计算幂
SELECT POWER(2, 3) AS power_result;

-- 计算正弦和余弦值
SELECT SIN(0) AS sin_zero, COS(0) AS cos_zero;

-- 计算自然对数和指数
SELECT LOG(10) AS natural_log, EXP(1) AS exp_one;

-- 四舍五入
SELECT ROUND(3.14159, 2) AS rounded_pi;

-- 向上取整和向下取整
SELECT CEIL(3.14) AS ceil_result, FLOOR(3.99) AS floor_result;

-- 模运算
SELECT MOD(10, 3) AS mod_result;

-- 结合多个数学函数
SELECT SQRT(ABS(-16)) AS sqrt_abs;

-- 在SELECT列表中使用数学函数
SELECT temperature, ROUND(temperature, 1) AS rounded_temp FROM sensor_readings;
```

### 9.7 复合函数示例

```sql
-- 结合WHERE条件和聚合函数
SELECT 
    sensor_id,
    COUNT(*) AS high_temp_count
FROM sensor_readings 
WHERE temperature > 23.0
GROUP BY sensor_id;

-- 结合时间窗口和聚合函数
SELECT 
    sensor_id,
    TIME_BUCKET('15m', timestamp) AS time_window,
    AVG(temperature) AS avg_temp,
    MIN(temperature) AS min_temp,
    MAX(temperature) AS max_temp
FROM sensor_readings 
WHERE sensor_id IN (1, 2)
GROUP BY sensor_id, time_window
ORDER BY sensor_id, time_window;

-- 结合数学函数和聚合函数
SELECT 
    sensor_id,
    AVG(temperature) AS avg_temp,
    STDDEV(temperature) AS stddev_temp,
    ROUND(AVG(temperature), 2) AS rounded_avg_temp
FROM sensor_readings 
GROUP BY sensor_id;
```

# 总结

RemDB提供了轻量级的SQL支持，适合嵌入式系统和边缘计算场景。它支持基本的SQL操作，包括SELECT、INSERT、UPDATE、DELETE、CREATE TABLE和CREATE INDEX，以及丰富的时序数据相关功能。

## 核心功能

1. **基本SQL操作**：完整支持SELECT、INSERT、UPDATE、DELETE等核心SQL语句
2. **索引机制**：支持多种索引类型，包括哈希索引、有序数组索引、B-Tree索引和T-Tree索引
3. **时序数据支持**：专门的时序表创建语法，支持数据压缩和TTL自动清理
4. **内嵌函数支持**：
   - 基础统计聚合函数：COUNT、SUM、AVG、MIN、MAX
   - 扩展统计函数：VAR、STDDEV（总体方差和标准差）、VAR_SAMP、STDDEV_SAMP（样本方差和标准差）
   - 滑动窗口函数：MOVING_SUM、MOVING_AVERAGE
   - 字符串函数：CONCAT、SUBSTRING、UPPER、LOWER
   - 数学函数：ABS、SQRT、POWER、SIN、COS、LOG、EXP、ROUND、CEIL、FLOOR、MOD
   - 时间窗口函数：TIME_BUCKET，支持多种时间间隔格式
5. **高效的查询执行**：优化的查询执行器，支持表达式求值和函数调用

## 适用场景

- 嵌入式系统和边缘计算
- IoT设备数据存储和分析
- 实时监控和告警系统
- 传感器数据处理
- 资源受限环境下的数据管理

虽然RemDB不支持复杂的SQL特性如JOIN和子查询，但它提供了高效的索引机制、时序数据处理能力和函数支持，适合对性能要求较高的嵌入式应用场景。