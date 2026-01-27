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
| `VECTOR(dim)` | Vector | 向量类型，支持指定维度、距离度量及量化算法，dim为向量维度，支持1-4096 |

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
| Vector | VECTOR(dim) | dim * 4 |

## 2. 支持的SQL语法

### 2.1 SELECT语句

```sql
SELECT [DISTINCT] [column1 [AS] alias1, column2 [AS] alias2, ... | *]
FROM table_name [AS] table_alias
[JOIN table_name2 [AS] table_alias2 ON condition]
[WHERE condition]
[GROUP BY column1, column2, ...]
[ORDER BY column [ASC | DESC]]
[LIMIT number];
```

#### DISTINCT子句

DISTINCT子句用于从查询结果中去除重复行，确保返回的每行都是唯一的。

**语法**：
```sql
SELECT DISTINCT column1, column2, ...
FROM table_name;
```

**说明**：
- `DISTINCT`：关键字，用于指定去重操作
- `column1, column2, ...`：要查询的列，可以是一个或多个
- 当指定多个列时，DISTINCT会根据所有指定列的组合来判断是否重复

**使用规则**：
- DISTINCT必须紧跟在SELECT关键字之后
- DISTINCT作用于所有指定的列，而不仅仅是紧跟其后的列
- DISTINCT可以与WHERE、ORDER BY等子句结合使用
- DISTINCT不适用于聚合函数的结果，因为聚合函数本身会产生唯一结果

**示例**：
```sql
-- 单列去重：查询所有不同的用户名
SELECT DISTINCT name FROM users;

-- 多列组合去重：查询不同的姓名和年龄组合
SELECT DISTINCT name, age FROM users;

-- 结合WHERE条件：查询年龄大于25的不同用户名
SELECT DISTINCT name FROM users WHERE age > 25;

-- 结合ORDER BY：查询不同的城市，并按城市名称排序
SELECT DISTINCT city FROM users ORDER BY city;

-- 结合WHERE和ORDER BY：查询年龄大于25的不同城市，并按城市名称排序
SELECT DISTINCT city FROM users WHERE age > 25 ORDER BY city;
```

#### 别名支持

RemDB支持列别名和表别名，允许用户在查询中为列和表指定替代名称，使查询结果更易读或简化查询语法。

##### 列别名

列别名用于为查询结果中的列指定一个更具描述性的名称，或简化列名。

**语法**：
```sql
column_name [AS] alias_name
```

**说明**：
- `column_name`：原始列名或表达式
- `AS`：可选的关键字，用于分隔列名和别名
- `alias_name`：自定义的列别名

**示例**：
```sql
-- 使用AS关键字指定列别名
SELECT id AS user_id, name AS user_name FROM users;

-- 不使用AS关键字指定列别名
SELECT id user_id, name user_name FROM users;

-- 为函数调用结果指定别名
SELECT COUNT(*) total_users FROM users;
```

##### 表别名

表别名用于为表指定一个简短的替代名称，通常在查询中多次引用同一表时使用，或简化查询语法。

**语法**：
```sql
table_name [AS] table_alias
```

**说明**：
- `table_name`：原始表名
- `AS`：可选的关键字，用于分隔表名和别名
- `table_alias`：自定义的表别名

**示例**：
```sql
-- 使用AS关键字指定表别名
SELECT u.id, u.name FROM users AS u;

-- 不使用AS关键字指定表别名
SELECT u.id, u.name FROM users u;

-- 在WHERE子句中使用表别名
SELECT u.id, u.name FROM users u WHERE u.age > 18;

-- 在ORDER BY子句中使用表别名
SELECT u.id, u.name FROM users u ORDER BY u.name DESC;
```

#### GROUP BY子句

GROUP BY子句用于将结果集按照一个或多个列或表达式进行分组，通常与聚合函数（如COUNT、SUM、AVG等）一起使用，对每个分组进行聚合计算。

**语法**：
```sql
GROUP BY column1, column2, ... | expression1, expression2, ...
```

**说明**：
- 可以按多个列或表达式进行分组，列之间用逗号分隔
- 分组可以是原始列名、表达式或函数调用（如TIME_BUCKET）
- GROUP BY子句通常位于WHERE子句之后，ORDER BY子句之前
- 可以与HAVING子句结合使用，对分组结果进行过滤
- 支持ORDER BY子句，可按分组列或聚合结果排序

**示例**：

```sql
-- 基本GROUP BY
SELECT sensor_id, COUNT(*) AS reading_count FROM sensor_readings GROUP BY sensor_id;

-- GROUP BY与WHERE条件
SELECT sensor_id, AVG(temperature) AS avg_temp FROM sensor_readings WHERE temperature > 20 GROUP BY sensor_id;

-- GROUP BY与ORDER BY
SELECT sensor_id, AVG(temperature) AS avg_temp FROM sensor_readings GROUP BY sensor_id ORDER BY avg_temp DESC;

-- 多列GROUP BY
SELECT sensor_id, location, AVG(temperature) AS avg_temp FROM sensor_readings GROUP BY sensor_id, location;

-- GROUP BY与多个聚合函数
SELECT sensor_id, AVG(temperature) AS avg_temp, MIN(temperature) AS min_temp, MAX(temperature) AS max_temp FROM sensor_readings GROUP BY sensor_id;

-- GROUP BY与HAVING子句
SELECT sensor_id, AVG(temperature) AS avg_temp FROM sensor_readings GROUP BY sensor_id HAVING avg_temp > 23;

-- 单列GROUP BY
SELECT location, COUNT(*) AS location_count FROM sensor_readings GROUP BY location;
```

#### JOIN子句

RemDB支持多种JOIN操作，用于将两个或多个表中的行根据相关列的值组合起来。

**语法**：
```sql
SELECT columns
FROM table1
[INNER] JOIN table2 ON join_condition
[WHERE condition]
[ORDER BY columns]
[LIMIT number];

-- 或使用其他JOIN类型
SELECT columns
FROM table1
LEFT [OUTER] JOIN table2 ON join_condition
WHERE condition;

SELECT columns
FROM table1
RIGHT [OUTER] JOIN table2 ON join_condition
WHERE condition;

SELECT columns
FROM table1
FULL [OUTER] JOIN table2 ON join_condition
WHERE condition;
```

**支持的JOIN类型**：

| JOIN类型 | 描述 |
|---------|------|
| `INNER JOIN` | 只返回两个表中匹配的行 |
| `LEFT JOIN` | 返回左表的所有行和右表中匹配的行，右表没有匹配时返回NULL |
| `RIGHT JOIN` | 返回右表的所有行和左表中匹配的行，左表没有匹配时返回NULL |
| `FULL JOIN` | 返回左表和右表的所有行，没有匹配时返回NULL |

**说明**：
- `ON join_condition`：指定JOIN条件，通常是两个表之间的列匹配关系
- JOIN操作可以用于连接两个或多个表
- 可以与WHERE、ORDER BY和LIMIT子句结合使用
- 支持使用表别名简化查询

**示例**：

```sql
-- INNER JOIN：查询用户及其订单
SELECT users.id, users.name, orders.product, orders.amount
FROM users
INNER JOIN orders ON users.id = orders.user_id;

-- LEFT JOIN：查询所有用户及其订单，没有订单的用户也会显示
SELECT users.id, users.name, orders.product, orders.amount
FROM users
LEFT JOIN orders ON users.id = orders.user_id;

-- RIGHT JOIN：查询所有订单及其对应的用户，没有用户的订单也会显示
SELECT users.id, users.name, orders.product, orders.amount
FROM users
RIGHT JOIN orders ON users.id = orders.user_id;

-- FULL JOIN：查询所有用户和所有订单，没有匹配的地方显示NULL
SELECT users.id, users.name, orders.product, orders.amount
FROM users
FULL JOIN orders ON users.id = orders.user_id;

-- 带有WHERE条件的JOIN
SELECT users.id, users.name, orders.product, orders.amount
FROM users
INNER JOIN orders ON users.id = orders.user_id
WHERE orders.amount > 100;

-- 带有ORDER BY的JOIN
SELECT users.id, users.name, orders.product, orders.amount
FROM users
INNER JOIN orders ON users.id = orders.user_id
ORDER BY orders.amount DESC;

-- 使用表别名的JOIN
SELECT u.id, u.name, o.product, o.amount
FROM users u
INNER JOIN orders o ON u.id = o.user_id;
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
    [PRIMARY KEY (column1, column2, ...)]
);

#### 支持的约束

- `PRIMARY KEY`：主键约束，支持单列主键和复合主键
- `NOT NULL`：非空约束
- `UNIQUE`：唯一约束
- `AUTOINCREMENT`/`AUTO_INCREMENT`：自增约束
- `DEFAULT value`：默认值约束

### 2.5.1 动态表结构管理

RemDB支持动态表结构管理，允许在表创建后添加或修改列。

#### ALTER TABLE语句

**语法**：
```sql
ALTER TABLE table_name 
    ADD COLUMN column_name datatype [constraints] | 
    MODIFY COLUMN column_name datatype [constraints] | 
    DROP COLUMN column_name;
```

**说明**：
- `ADD COLUMN`：添加新列到表中
- `MODIFY COLUMN`：修改现有列的类型或约束
- `DROP COLUMN`：从表中删除列

#### 示例

```sql
-- 添加新列到现有表
ALTER TABLE users ADD COLUMN phone TEXT;

-- 修改现有列的数据类型
ALTER TABLE users MODIFY COLUMN age INTEGER;

-- 从表中删除列
ALTER TABLE users DROP COLUMN email;

-- 添加带有默认值约束的新列
ALTER TABLE products ADD COLUMN in_stock BOOLEAN DEFAULT false;

-- 添加带有非空约束的新列
ALTER TABLE orders ADD COLUMN shipping_address TEXT NOT NULL;
```

动态表结构管理允许用户根据业务需求的变化灵活调整表结构，无需重新创建表和迁移数据。

#### 创建表示例

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    age INTEGER DEFAULT 0,
    email TEXT UNIQUE
);

-- 创建包含向量字段的表
CREATE TABLE vectors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vec VECTOR(768) WITH DISTANCE=L2,
    meta TEXT
);

-- 创建包含向量字段和其他类型字段的表
CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    embedding VECTOR(512) WITH DISTANCE=COSINE,
    price REAL,
    created_at TIMESTAMP
);

-- 创建带向量压缩的表
CREATE TABLE compressed_vectors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vec VECTOR(128) WITH DISTANCE=COSINE, COMPRESSION=PQ,
    meta TEXT
);

-- 创建带有复合主键的表
CREATE TABLE metrics (
    device_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    value REAL NOT NULL,
    PRIMARY KEY (device_id, metric_id, timestamp)
);
```

### 2.6 CREATE INDEX语句

```sql
CREATE INDEX index_name ON table_name (column1, column2, ...) [USING index_type] [WITH (parameter=value, ...)] [ONLINE | OFFLINE];
```

#### 示例

```sql
-- 创建标量索引
CREATE INDEX idx_users_age ON users (age);
CREATE INDEX idx_users_name ON users (name) USING BTREE;

-- 创建向量索引 - HNSW
CREATE INDEX idx_vectors_vec ON vectors (vec) USING HNSW WITH (M=16, ef_construction=200, ef_search=100, DISTANCE=L2) ONLINE;

-- 创建向量索引 - IVF_PQ
CREATE INDEX idx_products_embedding ON products (embedding) USING IVF_PQ WITH (nlist=128, nprobe=8, M=8, nbits=8) ONLINE;

-- 创建向量索引 - IVF_FLAT
CREATE INDEX idx_vectors_ivf_flat ON vectors (vec) USING IVF_FLAT WITH (nlist=128, nprobe=16, DISTANCE=L2) ONLINE;

-- 离线创建向量索引
CREATE INDEX idx_vectors_offline ON vectors (vec) USING HNSW WITH (M=16, ef_construction=200) OFFLINE;

-- 创建带有持久化的索引
CREATE INDEX idx_users_persistent ON users (name) USING BTREE WITH (STORAGE=DISK);

-- 创建向量索引 - 指定距离度量
CREATE INDEX idx_vectors_cosine ON vectors (vec) USING HNSW WITH (DISTANCE=COSINE);

-- 创建复合索引
CREATE INDEX idx_orders_customer_date ON orders (customer_id, order_date) USING BTREE;
CREATE INDEX idx_metrics_device_metric ON metrics (device_id, metric_id) USING TTREE;
```

#### 生产级索引特性

RemDB支持生产级索引特性，包括在线索引创建、索引构建状态监控、索引持久化等。

##### 索引参数

**标量索引参数**：
| 参数 | 说明 | 默认值 | 适用索引类型 |
|------|------|--------|--------------|
| `STORAGE` | 索引存储位置 | `MEMORY` | 所有索引类型 |
| `COMPRESSION` | 索引压缩类型 | `NONE` | 所有索引类型 |

**向量索引参数**：
| 参数 | 说明 | 默认值 | 适用索引类型 |
|------|------|--------|--------------|
| `DISTANCE` | 距离度量类型 | `L2` | 所有向量索引 |
| `M` | HNSW算法的M参数（每个节点的最大邻居数） | 16 | HNSW、HNSW_SQ、HNSW_BQ |
| `ef_construction` | HNSW构建时的ef参数 | 200 | HNSW、HNSW_SQ、HNSW_BQ |
| `ef_search` | HNSW搜索时的ef参数 | 100 | HNSW、HNSW_SQ、HNSW_BQ |
| `nlist` | IVF算法的簇数量 | 128 | IVF、IVF_FLAT、IVF_PQ |
| `nprobe` | IVF搜索时检查的簇数量 | 8 | IVF、IVF_FLAT、IVF_PQ |
| `M` | PQ算法的子向量数量 | 8 | IVF_PQ |
| `nbits` | PQ算法的量化位数 | 8 | IVF_PQ |

### 2.6.1 索引构建状态监控

RemDB支持监控索引构建的进度和状态。

#### SHOW INDEX BUILD STATUS语句

**语法**：
```sql
SHOW INDEX BUILD STATUS [FOR index_name] [FOR table_name];
```

**说明**：
- 不带参数：显示所有正在构建的索引状态
- `FOR index_name`：显示指定索引的构建状态
- `FOR table_name`：显示指定表上所有正在构建的索引状态

**返回结果**：
| 字段 | 类型 | 描述 |
|------|------|------|
| `index_name` | TEXT | 索引名称 |
| `table_name` | TEXT | 表名称 |
| `column_name` | TEXT | 索引列名称 |
| `index_type` | TEXT | 索引类型 |
| `status` | TEXT | 构建状态：PENDING、RUNNING、COMPLETED、FAILED |
| `progress` | INTEGER | 构建进度（0-100） |
| `elapsed_time` | INTEGER | 已运行时间（毫秒） |
| `estimated_time` | INTEGER | 估计剩余时间（毫秒） |
| `row_count` | INTEGER | 已处理的行数 |

#### 示例

```sql
-- 显示所有正在构建的索引状态
SHOW INDEX BUILD STATUS;

-- 显示指定索引的构建状态
SHOW INDEX BUILD STATUS FOR idx_vectors_vec;

-- 显示指定表上所有正在构建的索引状态
SHOW INDEX BUILD STATUS FOR vectors;
```

### 2.6.2 索引持久化

RemDB支持索引持久化，将索引数据存储到磁盘，提高系统重启后的恢复速度。

**语法**：
```sql
CREATE INDEX index_name ON table_name (column) USING index_type WITH (STORAGE=DISK);
```

**说明**：
- `STORAGE=DISK`：将索引数据持久化到磁盘
- `STORAGE=MEMORY`：索引仅存储在内存中（默认）

索引持久化可以通过以下方式配置：

1. **创建时指定**：在CREATE INDEX语句中使用WITH (STORAGE=DISK)参数
2. **全局配置**：在数据库配置中设置默认索引存储位置

### 2.6.3 索引重建

RemDB支持索引重建，用于优化索引结构或修复损坏的索引。

**语法**：
```sql
REINDEX index_name [ONLINE | OFFLINE];
```

**说明**：
- 重建索引会重新构建索引结构，优化索引性能
- 支持ONLINE和OFFLINE模式

#### 示例

```sql
-- 在线重建索引
REINDEX idx_vectors_vec ONLINE;

-- 离线重建索引
REINDEX idx_users_age OFFLINE;
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

### 2.8 DROP TABLE语句

```sql
DROP TABLE [IF EXISTS] table_name [CASCADE | RESTRICT] [DEFERRED];
```

#### 语法说明

- `DROP TABLE`：关键字，用于删除表
- `IF EXISTS`：可选，指定如果表不存在，操作不会报错
- `table_name`：要删除的表名
- `CASCADE`：可选，级联删除相关对象（暂未实现）
- `RESTRICT`：可选，限制删除操作（默认行为）
- `DEFERRED`：可选，延迟删除操作（暂未实现）

#### 示例

```sql
-- 删除表
DROP TABLE test_table;

-- 删除表（如果存在）
DROP TABLE IF EXISTS test_table;

-- 使用RESTRICT选项删除表
DROP TABLE test_table RESTRICT;

-- 使用DEFERRED选项删除表
DROP TABLE test_table DEFERRED;
```

#### 注意事项

- 系统表不允许删除
- 删除表会释放表占用的所有内存资源
- 删除表会记录操作到WAL日志，支持崩溃恢复
- 删除表后，表的所有数据和索引都会被清除
- 使用IF EXISTS选项可以避免表不存在时的错误

### 2.9 CREATE TIMESERIES TABLE语句

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

### 2.10 事务相关语句

RemDB支持基本的事务操作，包括开始事务、提交事务和回滚事务。

#### 2.10.1 BEGIN TRANSACTION语句

用于开始一个新的事务。

**语法**：
```sql
BEGIN TRANSACTION;
-- 或简化形式
BEGIN;
```

**说明**：
- 启动一个新的事务上下文
- 后续的SQL操作将在该事务中执行
- 支持的隔离级别为可重复读(Repeatable Read)

#### 2.10.2 COMMIT语句

用于提交当前事务，将所有更改持久化到数据库。

**语法**：
```sql
COMMIT;
-- 或完整形式
COMMIT TRANSACTION;
```

**说明**：
- 提交当前事务中的所有操作
- 将更改从事务日志写入到数据文件
- 释放事务资源

#### 2.10.3 ROLLBACK语句

用于回滚当前事务，撤销所有未提交的更改。

**语法**：
```sql
ROLLBACK;
-- 或完整形式
ROLLBACK TRANSACTION;
```

**说明**：
- 撤销当前事务中的所有操作
- 恢复事务开始前的状态
- 释放事务资源

**事务示例**：

```sql
-- 开始事务
BEGIN TRANSACTION;

-- 执行多个操作
INSERT INTO users VALUES (1, 'Alice', 25);
UPDATE users SET age = 26 WHERE name = 'Bob';

-- 提交事务
COMMIT;

-- 开始另一个事务
BEGIN;

-- 执行操作
DELETE FROM users WHERE id = 3;

-- 回滚事务，撤销删除操作
ROLLBACK;
```

### 2.10 支持的运算符

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

#### 向量距离运算符

- `<->`：向量L2距离，用于计算欧几里得距离
- `<#>`：向量内积，用于计算向量点积
- `<=>`：向量余弦相似度，用于计算向量夹角余弦值

### 2.11 函数支持

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

##### 时间转换函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `TO_ISO8601` | 将时间戳转换为ISO8601格式字符串 | `time_field` (TIMESTAMP) | `TEXT` | `TO_ISO8601(timestamp)` |
| `TO_CHAR` | 将时间戳按照指定格式转换为字符串 | `time_field` (TIMESTAMP), `format` (TEXT) | `TEXT` | `TO_CHAR(timestamp, 'YYYY-MM-DD HH24:MI:SS')` |
| `TO_EPOCH` | 将时间戳转换为UNIX时间戳（秒） | `time_field` (TIMESTAMP) | `REAL` | `TO_EPOCH(timestamp)` |

##### TIME_BUCKET与GROUP BY组合使用

`TIME_BUCKET`函数最常见的用法是与`GROUP BY`子句结合，用于对时序数据进行聚合分析。通过将时间戳分组到固定大小的时间窗口中，可以方便地计算每个窗口内的统计指标。

**语法**：
```sql
SELECT TIME_BUCKET(interval, time_field [, origin]) AS time_window,
       aggregation_function(column) AS alias
FROM table_name
[WHERE condition]
GROUP BY time_window [, other_columns]
[ORDER BY time_window [ASC | DESC]];
```

**说明**：
- `TIME_BUCKET`函数的结果可以直接用于`GROUP BY`子句
- 可以为`TIME_BUCKET`函数的结果指定别名，使查询更易读
- 支持与`WHERE`条件结合，过滤数据后再进行时间窗口聚合
- 支持与多列`GROUP BY`结合，实现更复杂的分组分析
- 支持与`ORDER BY`子句结合，按时间窗口排序结果

**示例**：

```sql
-- 按5分钟窗口聚合温度数据
SELECT TIME_BUCKET('5m', timestamp) AS time_window,
       AVG(temperature) AS avg_temp,
       COUNT(*) AS reading_count
FROM sensor_readings 
GROUP BY time_window;

-- 按1小时窗口聚合特定传感器数据
SELECT TIME_BUCKET('1h', timestamp) AS time_window,
       AVG(temperature) AS avg_temp,
       AVG(humidity) AS avg_humidity
FROM sensor_readings 
WHERE sensor_id = 1
GROUP BY time_window
ORDER BY time_window;

-- 使用数值形式的时间间隔（5分钟 = 300000000微秒）
SELECT TIME_BUCKET(300000000, timestamp) AS time_window,
       SUM(value) AS sum_value
FROM metrics 
GROUP BY time_window;

-- 自定义时间窗口起始点
SELECT TIME_BUCKET('1h', timestamp, '2024-01-01 00:30:00') AS time_window,
       MAX(temperature) AS max_temp
FROM sensor_readings 
GROUP BY time_window;

-- 多列分组：传感器ID + 时间窗口
SELECT sensor_id,
       TIME_BUCKET('15m', timestamp) AS time_window,
       AVG(temperature) AS avg_temp
FROM sensor_readings 
GROUP BY sensor_id, time_window
ORDER BY sensor_id, time_window;
```

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
| HNSW | 向量索引 | 支持高维向量的近似最近邻搜索，支持L2、IP、余弦相似度 |
| HNSW_SQ | 向量索引 | 带标量量化的HNSW索引，支持L2、IP、余弦相似度 |
| HNSW_BQ | 向量索引 | 带二值量化的HNSW索引，支持L2、IP、余弦相似度 |
| IVF | 向量索引 | 倒排文件索引，支持L2、IP、余弦相似度 |
| IVF_FLAT | 向量索引 | 带扁平量化的IVF索引，支持L2、IP、余弦相似度 |
| IVF_PQ | 向量索引 | 带乘积量化的IVF索引，支持L2、IP、余弦相似度 |

### 3.2 索引功能

- **精确查找**：根据键值精确查找记录，支持复合键查找
- **范围查找**：查找指定范围内的所有记录，支持复合键范围查找
- **索引统计**：支持查看索引的访问次数、命中次数、大小和项数量
- **向量精确搜索**：支持向量的精确最近邻搜索
- **向量近似最近邻搜索**：支持高效的向量近似搜索
- **多种距离度量**：支持L2距离、内积(IP)和余弦相似度计算
- **混合搜索**：支持向量搜索与标量过滤、全文搜索的混合查询
- **复合索引**：支持基于多个列创建索引，提高多列查询的性能

### 3.3 索引创建示例

```sql
-- 创建标量索引
CREATE INDEX idx_orders_timestamp ON orders (timestamp) USING BTREE;
CREATE INDEX idx_orders_amount ON orders (amount) USING TTREE;

-- 创建向量索引 - HNSW
CREATE INDEX idx_vectors_vec ON vectors (vec) USING HNSW WITH (M=16, ef_construction=200);

-- 创建向量索引 - HNSW_SQ
CREATE INDEX idx_vectors_sq ON vectors (vec) USING HNSW_SQ WITH (M=16, ef_construction=200, DISTANCE=COSINE);

-- 创建向量索引 - HNSW_BQ
CREATE INDEX idx_vectors_bq ON vectors (vec) USING HNSW_BQ WITH (M=16, ef_construction=200, DISTANCE=IP);

-- 创建向量索引 - IVF
CREATE INDEX idx_vectors_ivf ON vectors (vec) USING IVF WITH (nlist=128, DISTANCE=L2);

-- 创建向量索引 - IVF_FLAT
CREATE INDEX idx_vectors_ivf_flat ON vectors (vec) USING IVF_FLAT WITH (nlist=128, nprobe=16, DISTANCE=COSINE);

-- 创建向量索引 - IVF_PQ
CREATE INDEX idx_vectors_ivfpq ON vectors (vec) USING IVF_PQ WITH (nlist=128, nprobe=8, M=8, nbits=8);
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

### 4.4 时间转换函数示例

```sql
-- 将时间戳转换为ISO8601格式
SELECT TO_ISO8601(timestamp) AS iso_time FROM sensor_data;

-- 将时间戳转换为指定格式字符串
SELECT TO_CHAR(timestamp, 'YYYY-MM-DD') AS date FROM sensor_data;
SELECT TO_CHAR(timestamp, 'HH24:MI:SS') AS time FROM sensor_data;
SELECT TO_CHAR(timestamp, 'YYYY-MM-DD HH24:MI:SS') AS datetime FROM sensor_data;

-- 将时间戳转换为UNIX时间戳（秒）
SELECT TO_EPOCH(timestamp) AS epoch_seconds FROM sensor_data;

-- 结合时间转换函数和其他函数
SELECT 
    sensor_id,
    AVG(temperature) AS avg_temp,
    TO_CHAR(timestamp, 'YYYY-MM-DD') AS date
FROM sensor_data
GROUP BY sensor_id, date;
```

### 4.5 时间辅助功能

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

## 6. 向量相关功能

### 6.1 向量数据类型

向量是RemDB支持的基础数据类型，与INT、STRING等类型同等地位。

**语法**：`VECTOR(dim) [WITH DISTANCE=distance_type] [WITH COMPRESSION=compression_type]`

**参数说明**：
- `dim`：向量维度，支持1-4096
- `distance_type`：距离度量类型，可选值：
  - `L2`：欧几里得距离（默认）
  - `IP`：内积
  - `COSINE`：余弦相似度
- `compression_type`：向量压缩类型，可选值：
  - `NONE`：无压缩（默认）
  - `SQ`：标量量化，将浮点向量压缩为8位整数
  - `PQ`：乘积量化，将高维向量分解为多个低维子向量并分别量化
  - `BQ`：二值量化，将向量压缩为二进制表示

向量压缩可以显著减少存储需求和提高搜索性能，同时保持较高的搜索质量。不同的压缩类型适合不同的应用场景，用户可以根据实际需求选择合适的压缩方式。

### 6.2 向量操作符

RemDB支持向量的基本运算操作符和距离度量操作符：

#### 基本向量操作符

| 操作符 | 描述 | 示例 |
|-------|------|------|
| `+` | 向量加法 | `SELECT vec1 + vec2 FROM vectors` |
| `-` | 向量减法 | `SELECT vec1 - vec2 FROM vectors` |
| `*` | 向量标量乘法 | `SELECT vec * 2 FROM vectors` |
| `<` | 向量比较 | `SELECT * FROM vectors WHERE vec < [0.1, 0.2, ...]` |
| `>` | 向量比较 | `SELECT * FROM vectors WHERE vec > [0.1, 0.2, ...]` |
| `<=` | 向量比较 | `SELECT * FROM vectors WHERE vec <= [0.1, 0.2, ...]` |
| `>=` | 向量比较 | `SELECT * FROM vectors WHERE vec >= [0.1, 0.2, ...]` |
| `=` | 向量相等 | `SELECT * FROM vectors WHERE vec = [0.1, 0.2, ...]` |
| `!=` | 向量不等 | `SELECT * FROM vectors WHERE vec != [0.1, 0.2, ...]` |

#### 向量距离操作符

RemDB支持以下向量距离度量操作符，用于计算两个向量之间的相似度：

| 操作符 | 距离度量 | 描述 | 示例 |
|-------|----------|------|------|
| `<->` | L2距离 | 欧几里得距离，值越小表示越相似 | `SELECT id, vec <-> [0.1, 0.2] AS distance FROM vectors ORDER BY distance LIMIT 10` |
| `<#>` | 内积 | 向量内积，值越大表示越相似 | `SELECT id, vec <#> [0.1, 0.2] AS similarity FROM vectors ORDER BY similarity DESC LIMIT 10` |
| `<=>` | 余弦相似度 | 余弦相似度，值越大表示越相似 | `SELECT id, vec <=> [0.1, 0.2] AS similarity FROM vectors ORDER BY similarity DESC LIMIT 10` |

**说明**：
- 这些操作符可以用于SELECT列表、WHERE子句和ORDER BY子句
- 在WHERE子句中，它们可以与BETWEEN条件结合使用
- 支持与标量条件的混合查询

**示例**：

```sql
-- 在SELECT列表中使用向量距离操作符
SELECT id, name, embedding <-> [0.1, 0.2, 0.3] AS distance FROM products;

-- 在WHERE子句中使用向量距离操作符
SELECT id, name FROM vectors WHERE vec <-> [0.5, 0.5] < 0.5;

-- 在BETWEEN条件中使用向量距离操作符
SELECT id, name FROM vectors WHERE vec <-> [0.5, 0.5] BETWEEN 0.1 AND 0.5;

-- 结合向量距离和标量条件
SELECT id, name FROM products WHERE embedding <=> [0.1, 0.2] > 0.8 AND price < 100;

-- 在ORDER BY子句中使用向量距离
SELECT id, name FROM vectors ORDER BY vec <#> [0.5, 0.5] DESC LIMIT 5;
```

### 6.3 向量搜索

RemDB支持多种向量搜索方式，包括使用专用函数和直接使用向量距离操作符。

#### 1. 使用向量距离操作符（推荐）

直接使用向量距离操作符是进行向量搜索的最直观方式：

**语法**：
```sql
SELECT * FROM table_name
WHERE vector_column <-> query_vector [operator] threshold
[AND other_conditions]
ORDER BY vector_column <-> query_vector [ASC | DESC]
LIMIT k;
```

**示例**：

```sql
-- 使用L2距离操作符进行搜索
SELECT id, meta, vec <-> [0.1, 0.2, ...] AS distance
FROM vectors
WHERE vec <-> [0.1, 0.2, ...] < 0.5
ORDER BY distance
LIMIT 10;

-- 使用余弦相似度操作符进行搜索
SELECT id, name, embedding <=> [0.1, 0.2, ...] AS similarity
FROM products
WHERE embedding <=> [0.1, 0.2, ...] > 0.8
AND price < 100
ORDER BY similarity DESC
LIMIT 5;

-- 使用内积操作符进行搜索
SELECT id, name, vec <#> [0.5, 0.5] AS similarity
FROM vectors
WHERE vec <#> [0.5, 0.5] BETWEEN 0.3 AND 0.8
ORDER BY similarity DESC
LIMIT 10;
```

#### 2. 使用向量搜索函数

RemDB也支持使用专用函数进行向量搜索：

**语法**：
```sql
SELECT * FROM table_name
WHERE VECTOR_SIMILAR(column, query_vector [, distance_type]) [AND other_conditions]
ORDER BY VECTOR_DISTANCE(column, query_vector [, distance_type])
LIMIT k;
```

**函数说明**：
- `VECTOR_SIMILAR(column, query_vector [, distance_type])`：判断向量是否相似，返回布尔值
- `VECTOR_DISTANCE(column, query_vector [, distance_type])`：计算向量间的距离，用于排序

**示例**：

```sql
-- 使用VECTOR_SIMILAR和VECTOR_DISTANCE函数
SELECT id, meta, VECTOR_DISTANCE(vec, [0.1, 0.2, ...]) AS distance
FROM vectors
WHERE VECTOR_SIMILAR(vec, [0.1, 0.2, ...], L2)
ORDER BY distance
LIMIT 10;

-- 结合函数和标量条件
SELECT id, name, VECTOR_DISTANCE(embedding, [0.1, 0.2, ...], COSINE) AS similarity
FROM products
WHERE VECTOR_SIMILAR(embedding, [0.1, 0.2, ...], COSINE)
AND price < 100
ORDER BY similarity DESC
LIMIT 5;
```

### 6.4 向量混合搜索

RemDB支持向量数据与标量数据的混合搜索，以及向量搜索与其他搜索条件的结合。

**示例**：

```sql
-- 使用距离操作符的向量搜索与标量过滤的混合搜索
SELECT id, name, price, embedding <-> query_vec AS distance
FROM products
WHERE price BETWEEN 50 AND 200
AND category = 'electronics'
AND embedding <-> query_vec < 0.5
ORDER BY distance
LIMIT 10;

-- 使用余弦相似度的混合搜索
SELECT id, name, price, embedding <=> [0.1, 0.2, ...] AS similarity
FROM products
WHERE category = 'books'
AND embedding <=> [0.1, 0.2, ...] > 0.7
AND price < 50
ORDER BY similarity DESC
LIMIT 5;

-- 结合向量距离和多个标量条件
SELECT id, meta, vec <-> [0.5, 0.5] AS distance
FROM vectors
WHERE vec <-> [0.5, 0.5] BETWEEN 0.1 AND 0.5
AND category = 'image'
AND created_at > 1609459200000
ORDER BY distance
LIMIT 20;

-- 向量搜索与全文搜索的混合搜索
SELECT id, title, content, embedding <=> query_vec AS relevance
FROM articles
WHERE MATCH(title, content) AGAINST('vector database')
AND embedding <=> query_vec > 0.8
ORDER BY relevance
LIMIT 5;
```

## 7. 注意事项

1. 字符串类型最大长度为64字节，超过将被截断
2. 主键必须是唯一的，支持单列主键和复合主键
3. 自增列只能用于整数类型
4. 索引键最大长度为64字节
5. WHERE子句支持向量搜索条件和标量过滤条件的组合
6. ORDER BY子句支持多个字段排序和位置索引，包括向量距离排序
7. 时序表必须包含一个TIMESTAMP类型的时间字段和一个数值类型的值字段
8. 时序表支持的压缩算法：`none`、`delta`、`runlength`、`delta-runlength`、`delta-delta`
9. 时序表的TTL配置用于自动清理过期数据块，单位支持天、小时、分钟、秒
10. 时序表的WITH子句只能用于CREATE TIMESERIES TABLE语句，不支持普通表
11. 向量维度必须在创建表时指定，不支持动态修改
12. 向量索引列最大维度为1024
13. 默认采用NULL first比较模式，对NULL值进行排序时会将其放至最前，建议查询时加上NOT NULL条件
14. 向量压缩类型可以在创建表时指定，支持SQ、PQ、BQ和NONE
15. 动态表结构修改（ALTER TABLE）会导致表数据重建，可能影响性能
16. 在线索引创建不会阻塞表的读写操作，但索引构建速度会比离线创建慢
17. 索引持久化会增加存储需求，但可以提高系统重启后的恢复速度
18. 向量搜索性能受索引类型和参数配置影响，建议根据实际数据特性调整参数

## 8. 不支持的SQL特性

- 子查询

- 视图和存储过程
- 外键约束
- LIKE运算符

## 9. 示例：完整的时序数据应用

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

## 10. 函数使用示例

### 10.1 基础聚合函数示例

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

### 10.3 滑动窗口函数示例

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

### 10.4 时间窗口函数示例

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

### 10.5 字符串函数示例

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

### 10.6 数学函数示例

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

### 10.7 复合函数示例

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
2. **事务支持**：支持ACID事务，包括BEGIN TRANSACTION、COMMIT和ROLLBACK语句
3. **索引机制**：支持多种索引类型，包括哈希索引、有序数组索引、B-Tree索引和T-Tree索引
4. **时序数据支持**：专门的时序表创建语法，支持数据压缩和TTL自动清理
5. **内嵌函数支持**：
   - 基础统计聚合函数：COUNT、SUM、AVG、MIN、MAX
   - 扩展统计函数：VAR、STDDEV（总体方差和标准差）、VAR_SAMP、STDDEV_SAMP（样本方差和标准差）
   - 滑动窗口函数：MOVING_SUM、MOVING_AVERAGE
   - 字符串函数：CONCAT、SUBSTRING、UPPER、LOWER
   - 数学函数：ABS、SQRT、POWER、SIN、COS、LOG、EXP、ROUND、CEIL、FLOOR、MOD
   - 时间窗口函数：TIME_BUCKET，支持多种时间间隔格式
   - 时间转换函数：TO_ISO8601、TO_CHAR、TO_EPOCH
5. **高效的查询执行**：优化的查询执行器，支持表达式求值和函数调用

## 适用场景

- 嵌入式系统和边缘计算
- IoT设备数据存储和分析
- 实时监控和告警系统
- 传感器数据处理
- 资源受限环境下的数据管理

虽然RemDB不支持复杂的SQL特性如JOIN和子查询，但它提供了高效的索引机制、时序数据处理能力和函数支持，适合对性能要求较高的嵌入式应用场景。