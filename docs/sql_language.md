# SQL Language Documentation

## 1. 支持的SQL数据类型

RemDB支持以下SQL数据类型：

| SQL类型 | 内部类型 | 描述 |
|---------|----------|------|
| `INTEGER` | Int8/Int16/Int32/Int64/UInt8/UInt16/UInt32/UInt64 | 整数类型，根据实际使用自动选择合适的内部类型 |
| `REAL` | Float32/Float64 | 浮点数类型 |
| `TEXT` | Text | 文本字符串类型 |
| `VARCHAR(n)` | VarChar | 可变长度字符串类型，n为最大长度 |
| `CHAR(n)` | Char | 固定长度字符串类型，n为长度 |
| `BOOLEAN`/`BOOL` | Bool | 布尔类型，存储为0或1 |
| `TIMESTAMP` | Timestamp | 时间戳类型，支持微秒精度 |
| `TIMESTAMPTZ` | TimestampTZ | 带时区的时间戳类型 |
| `INTERVAL` | Interval | 时间间隔类型 |
| `VECTOR(dim)` | Vector | 向量类型，支持指定维度、距离度量及量化算法，dim为向量维度，支持1-4096 |
| `JSON` | Json | JSON数据类型，支持复杂嵌套结构 |

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
| Bool | BOOL | 1 |
| Timestamp | TIMESTAMP | 8 (默认，根据精度调整) |
| TimestampTZ | TIMESTAMPTZ | 8 (默认，根据精度调整) |
| Interval | INTERVAL | 8 (默认，根据精度调整) |
| Text | TEXT | 可变 |
| VarChar | VARCHAR(n) | 可变 |
| Char | CHAR(n) | 固定 |
| Vector | VECTOR(dim) | dim * 4 |
| Json | JSON | 可变 |

### 1.2 UTF8 字符支持

RemDB 完全支持 UTF8 字符编码，包括：

- **字符串存储**：TEXT、VARCHAR 和 CHAR 类型使用 UTF8 编码存储字符串
- **字符函数**：所有字符串函数（如 CONCAT、SUBSTRING、UPPER、LOWER）都支持 UTF8 字符
- **LIKE 运算符**：支持 UTF8 字符的模式匹配
- **排序**：字符串排序基于 UTF8 编码的字典序
- **长度计算**：字符串长度计算基于 UTF8 字符数，而非字节数

**示例**：

```sql
-- 存储包含 UTF8 字符的字符串
INSERT INTO users (name) VALUES ('测试用户');

-- 使用字符串函数处理 UTF8 字符
SELECT UPPER('测试用户') AS upper_name;

-- 使用 LIKE 运算符匹配 UTF8 字符
SELECT * FROM users WHERE name LIKE '%测试%';

-- 排序包含 UTF8 字符的字符串
SELECT * FROM users ORDER BY name;
```

## 2. 支持的SQL语法

### 2.0 数据库管理语句

RemDB支持基本的数据库管理语句，用于创建和管理数据库。

#### CREATE DATABASE语句

用于创建一个新的数据库。

**语法**：
```sql
CREATE DATABASE [IF NOT EXISTS] database_name [USING SCHEMA schema_name] [WITH CONFIGURATION (parameter=value, ...)];
```

**参数说明**：
- `IF NOT EXISTS`：可选，指定如果数据库已存在，操作不会报错
- `database_name`：要创建的数据库名称
- `USING SCHEMA schema_name`：可选，指定使用的模式
- `WITH CONFIGURATION`：可选，指定数据库配置参数

**示例**：

```sql
-- 创建一个新的数据库
CREATE DATABASE my_database;

-- 创建一个数据库（如果不存在）
CREATE DATABASE IF NOT EXISTS my_database;

-- 使用指定模式创建数据库
CREATE DATABASE my_database USING SCHEMA public;

-- 带配置参数创建数据库
CREATE DATABASE my_database WITH CONFIGURATION (max_connections=100, cache_size=1024);
```

#### USE DATABASE语句

用于切换到指定的数据库。

**语法**：
```sql
USE DATABASE database_name;
```

**参数说明**：
- `database_name`：要切换到的数据库名称

**示例**：

```sql
-- 切换到指定数据库
USE DATABASE my_database;
```

#### CLOSE DATABASE语句

用于关闭指定的数据库。

**语法**：
```sql
CLOSE DATABASE database_name;
```

**参数说明**：
- `database_name`：要关闭的数据库名称

**示例**：

```sql
-- 关闭指定数据库
CLOSE DATABASE my_database;
```

#### DROP DATABASE语句

用于删除一个现有的数据库。

**语法**：
```sql
DROP DATABASE [IF EXISTS] database_name;
```

**参数说明**：
- `IF EXISTS`：可选，指定如果数据库不存在，操作不会报错
- `database_name`：要删除的数据库名称

**示例**：

```sql
-- 删除一个数据库
DROP DATABASE my_database;

-- 删除一个数据库（如果存在）
DROP DATABASE IF EXISTS my_database;
```

### 2.1 SELECT语句

```sql
SELECT [DISTINCT] [column1 [AS] alias1, column2 [AS] alias2, ... | *]
FROM table_name [AS] table_alias
[JOIN table_name2 [AS] table_alias2 ON condition]
[WHERE condition]
[GROUP BY column1, column2, ...]
[HAVING condition]
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
- GROUP BY子句通常位于WHERE子句之后，HAVING子句之前
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

#### HAVING子句

HAVING子句用于对GROUP BY子句产生的分组结果进行过滤，类似于WHERE子句但专门用于分组数据。HAVING子句可以使用聚合函数，而WHERE子句不能。

**语法**：
```sql
HAVING condition
```

**说明**：
- `condition`：过滤条件，通常包含聚合函数（如COUNT、SUM、AVG等）
- HAVING子句必须位于GROUP BY子句之后，ORDER BY子句之前
- 可以使用与WHERE子句相同的运算符和逻辑
- 可以引用SELECT语句中定义的列别名

**示例**：

```sql
-- 基本HAVING子句
SELECT sensor_id, COUNT(*) AS reading_count FROM sensor_readings GROUP BY sensor_id HAVING reading_count > 10;

-- HAVING子句与聚合函数
SELECT sensor_id, AVG(temperature) AS avg_temp FROM sensor_readings GROUP BY sensor_id HAVING AVG(temperature) > 25;

-- 复杂HAVING条件
SELECT sensor_id, AVG(temperature) AS avg_temp, MAX(temperature) AS max_temp 
FROM sensor_readings 
GROUP BY sensor_id 
HAVING avg_temp > 20 AND max_temp < 40;

-- HAVING与WHERE结合
SELECT sensor_id, AVG(temperature) AS avg_temp 
FROM sensor_readings 
WHERE timestamp > 1609459200000 
GROUP BY sensor_id 
HAVING avg_temp > 25;
```

#### 窗口函数

窗口函数（Window Functions）是一种特殊的函数，它可以对结果集的一个子集（称为窗口）进行计算，而不会改变结果集的行数。窗口函数在分析型SQL查询中非常有用，特别是用于计算排名、移动平均值、累积总和等。

**语法**：
```sql
window_function([expression]) OVER (
    [PARTITION BY partition_expression]
    [ORDER BY order_expression [ASC | DESC]]
    [ROWS BETWEEN frame_start AND frame_end]
)
```

**说明**：
- `window_function`：窗口函数名称（如ROW_NUMBER、RANK、DENSE_RANK等）
- `expression`：函数参数，根据具体窗口函数而定
- `PARTITION BY`：可选，按指定列或表达式对结果集进行分区
- `ORDER BY`：可选，指定窗口内的排序方式
- `ROWS BETWEEN`：可选，定义窗口的行范围

**支持的窗口函数**：

| 函数名 | 描述 | 参数 | 返回类型 |
|--------|------|------|----------|
| `ROW_NUMBER()` | 为每行分配唯一的序号，从1开始 | 无 | `INTEGER` |
| `RANK()` | 为每行分配排名，相同值的行具有相同排名，后续排名会跳跃 | 无 | `INTEGER` |
| `DENSE_RANK()` | 为每行分配排名，相同值的行具有相同排名，后续排名不会跳跃 | 无 | `INTEGER` |
| `NTILE(n)` | 将结果集分成n个大致相等的桶，为每行分配桶号 | n：桶数 | `INTEGER` |
| `LAG(expression, [offset], [default])` | 返回当前行之前offset行的值 | expression：列或表达式<br>offset：偏移量，默认为1<br>default：当偏移超出范围时的默认值 | 与expression相同 |
| `LEAD(expression, [offset], [default])` | 返回当前行之后offset行的值 | expression：列或表达式<br>offset：偏移量，默认为1<br>default：当偏移超出范围时的默认值 | 与expression相同 |
| `FIRST_VALUE(expression)` | 返回窗口内的第一个值 | expression：列或表达式 | 与expression相同 |
| `LAST_VALUE(expression)` | 返回窗口内的最后一个值 | expression：列或表达式 | 与expression相同 |

**窗口框架定义**：

| 框架定义 | 描述 |
|----------|------|
| `ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW` | 从窗口开始到当前行 |
| `ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING` | 从当前行到窗口结束 |
| `ROWS BETWEEN n PRECEDING AND CURRENT ROW` | 从当前行之前n行到当前行 |
| `ROWS BETWEEN CURRENT ROW AND n FOLLOWING` | 从当前行到当前行之后n行 |
| `ROWS BETWEEN n PRECEDING AND m FOLLOWING` | 从当前行之前n行到当前行之后m行 |

**窗口函数使用示例**：

```sql
-- 基本排名函数
SELECT 
    student_id, 
    name, 
    score, 
    ROW_NUMBER() OVER (ORDER BY score DESC) AS row_num, 
    RANK() OVER (ORDER BY score DESC) AS rank, 
    DENSE_RANK() OVER (ORDER BY score DESC) AS dense_rank
FROM students;

-- 按科目分区排名
SELECT 
    student_id, 
    name, 
    subject, 
    score, 
    RANK() OVER (PARTITION BY subject ORDER BY score DESC) AS subject_rank
FROM exam_results;

-- 使用LAG和LEAD函数
SELECT 
    date, 
    price, 
    LAG(price, 1) OVER (ORDER BY date) AS prev_price, 
    LEAD(price, 1) OVER (ORDER BY date) AS next_price,
    price - LAG(price, 1) OVER (ORDER BY date) AS price_change
FROM stock_prices;

-- 使用FIRST_VALUE和LAST_VALUE
SELECT 
    department, 
    employee_id, 
    name, 
    salary, 
    FIRST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC) AS highest_salary,
    LAST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC) AS lowest_salary
FROM employees;

-- 使用NTILE函数
SELECT 
    student_id, 
    name, 
    score, 
    NTILE(4) OVER (ORDER BY score DESC) AS quartile
FROM students;

-- 使用窗口框架
SELECT 
    date, 
    value, 
    AVG(value) OVER (
        ORDER BY date 
        ROWS BETWEEN 2 PRECEDING AND CURRENT ROW
    ) AS moving_avg
FROM sensor_data;

-- 复杂窗口函数查询
SELECT 
    department, 
    employee_id, 
    name, 
    salary, 
    RANK() OVER (PARTITION BY department ORDER BY salary DESC) AS dept_rank,
    PERCENT_RANK() OVER (PARTITION BY department ORDER BY salary DESC) AS dept_percent_rank,
    CUME_DIST() OVER (PARTITION BY department ORDER BY salary DESC) AS dept_cume_dist
FROM employees;
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
) [WITH CONFIGURATION (parameter=value, ...)];

#### 支持的约束

- `PRIMARY KEY`：主键约束，支持单列主键和复合主键
- `NOT NULL`：非空约束
- `UNIQUE`：唯一约束
- `AUTOINCREMENT`/`AUTO_INCREMENT`：自增约束
- `DEFAULT value`：默认值约束

### 2.5.2 表配置参数

RemDB支持在创建表时通过`WITH CONFIGURATION`子句指定表级配置参数。这些参数可以控制表的行为和资源限制。

**语法**：
```sql
WITH CONFIGURATION (parameter=value, ...)
```

**支持的配置参数**：

| 参数名 | 数据类型 | 默认值 | 描述 |
|--------|----------|--------|------|
| `max_records` | 整数 | 1000 | 表的最大记录数限制。当表达到此限制时，新的插入操作可能会失败或触发旧数据的清理策略。 |

**低功耗模式交互**：
当数据库运行在低功耗模式下（通过`low_power_mode_supported`和`low_power_max_records`配置），表的最大记录数将受到进一步限制。实际生效的`max_records`值为配置值与低功耗模式限制值中的较小者。

**示例**：

```sql
-- 创建带有max_records配置的表
CREATE TABLE my_memory_table (
    id INT AUTO_INCREMENT PRIMARY KEY,
    data VARCHAR(255)
) WITH CONFIGURATION (max_records=100);

-- 创建带有多个配置参数的表（支持扩展）
CREATE TABLE sensor_data (
    sensor_id INTEGER,
    timestamp TIMESTAMP,
    value REAL,
    PRIMARY KEY (sensor_id, timestamp)
) WITH CONFIGURATION (max_records=10000, compression_level=2);
```

**注意**：
- 配置参数名称不区分大小写（例如，`MAX_RECORDS`和`max_records`等效）。
- 如果指定的配置参数不被支持，该参数将被忽略。
- 配置参数值可以是字符串、整数、浮点数、布尔值或JSON格式。

### 2.5.1 动态表结构管理

RemDB支持动态表结构管理，允许在表创建后添加或修改列。

#### ALTER TABLE语句

**语法**：
```sql
ALTER TABLE table_name 
    ADD [COLUMN] column_name datatype [constraints] | 
    MODIFY [COLUMN] column_name datatype [constraints] | 
    DROP [COLUMN] column_name |
    RENAME [COLUMN] old_column_name TO new_column_name;
```

**说明**：
- `ADD COLUMN`：添加新列到表中
- `MODIFY COLUMN`：修改现有列的类型或约束
- `DROP COLUMN`：从表中删除列
- `RENAME COLUMN`：重命名表中的列

#### 示例

```sql
-- 添加新列到现有表
ALTER TABLE users ADD COLUMN phone TEXT;

-- 修改现有列的数据类型
ALTER TABLE users MODIFY COLUMN age INTEGER;

-- 从表中删除列
ALTER TABLE users DROP COLUMN email;

-- 重命名表中的列
ALTER TABLE users RENAME COLUMN phone TO contact_phone;

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

-- 创建包含JSON字段的表
CREATE TABLE customer_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    details JSON
);

-- 创建包含VARCHAR和CHAR类型的表
CREATE TABLE contact_info (
    id INTEGER PRIMARY KEY,
    first_name VARCHAR(50) NOT NULL,
    last_name VARCHAR(50) NOT NULL,
    phone CHAR(10),
    email VARCHAR(100) UNIQUE
);

-- 创建包含TIMESTAMPTZ和INTERVAL类型的表
CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    event_name TEXT NOT NULL,
    event_time TIMESTAMPTZ,
    duration INTERVAL
);
```

### 2.6 CREATE INDEX语句

```sql
CREATE INDEX [index_name] ON table_name (column1 [ASC|DESC], column2 [ASC|DESC], ...) [USING index_type] [WITH (parameter=value, ...)] [ONLINE | OFFLINE];
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

-- 创建JSON索引
CREATE INDEX idx_customer_details ON customer_data (details) USING BTREE;
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

### 2.10 模型管理语句

RemDB支持AI模型的注册和管理，允许将预训练的模型（如ONNX格式）注册为数据库中的函数，在SQL查询中直接调用。

#### 2.10.1 CREATE MODEL语句

用于注册一个新的AI模型，使其可以在SQL查询中作为用户定义函数（UDF）使用。

**语法**：
```sql
CREATE MODEL model_name USING 'model_path.onnx' AS (input1 datatype1, input2 datatype2, ...) RETURNS output_datatype;
```

**参数说明**：
- `model_name`：模型名称，将作为SQL函数名使用
- `model_path`：模型文件路径（ONNX格式）
- `(input1 datatype1, input2 datatype2, ...)`：模型输入参数定义
- `output_datatype`：模型输出数据类型

**支持的输入数据类型**：
- `STRING`：字符串类型，将被转换为模型所需的向量表示
- `VECTOR(dim)`：向量类型，维度需与模型输入匹配
- `REAL`：浮点数类型
- `INTEGER`：整数类型

**支持的输出数据类型**：
- `VECTOR(dim)`：向量类型，用于返回嵌入向量、特征向量等
- `REAL`：浮点数类型，用于返回分类分数、回归值等
- `INTEGER`：整数类型，用于返回分类标签等

**示例**：
```sql
-- 注册文本嵌入模型，输入为字符串，输出为768维向量
CREATE MODEL bge_embedding USING 'bge-m3.onnx' AS (text STRING) RETURNS VECTOR(768);

-- 注册图像分类模型，输入为512维向量，输出为浮点数分数
CREATE MODEL image_classifier USING 'resnet.onnx' AS (features VECTOR(512)) RETURNS REAL;

-- 注册多输入模型，输入为字符串和整数，输出为向量
CREATE MODEL multimodal_model USING 'multimodal.onnx' AS (text STRING, image_id INTEGER) RETURNS VECTOR(256);
```

**注意事项**：
1. 模型文件必须是有效的ONNX格式
2. 模型输入输出的数据类型和维度必须与实际模型匹配
3. 模型注册后，可以在SQL查询中通过模型名称调用
4. 模型加载需要一定的内存和时间开销
5. 同一模型名称只能注册一次，重复注册会失败

### 2.11 事务相关语句

RemDB支持基本的事务操作，包括开始事务、提交事务和回滚事务。

#### 2.11.1 BEGIN TRANSACTION语句

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

#### 2.11.2 COMMIT语句

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

#### 2.11.3 ROLLBACK语句

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

### 2.12 支持的运算符

#### 比较运算符

- `=`：等于
- `<>`/`!=`：不等于
- `>`：大于
- `>=`：大于等于
- `<`：小于
- `<=`：小于等于
- `LIKE`：模式匹配，支持通配符和转义字符

##### LIKE运算符详细说明

`LIKE`运算符用于在WHERE子句中进行字符串模式匹配，支持以下通配符：

- `%`：匹配任意长度的任意字符序列（包括空序列）
- `_`：匹配单个任意字符
- `\`：转义字符，用于匹配字面上的通配符（如`\%`匹配百分号本身）

**语法**：
```sql
column_name LIKE pattern
```

**示例**：

```sql
-- 匹配以"a"开头的所有字符串
SELECT * FROM users WHERE name LIKE 'a%';

-- 匹配以"end"结尾的所有字符串
SELECT * FROM users WHERE name LIKE '%end';

-- 匹配包含"middle"的所有字符串
SELECT * FROM users WHERE name LIKE '%middle%';

-- 匹配长度为3且以"a"开头的字符串
SELECT * FROM users WHERE name LIKE 'a__';

-- 匹配第二个字符为"b"的所有字符串
SELECT * FROM users WHERE name LIKE '_b%';

-- 匹配包含字面百分号的字符串
SELECT * FROM products WHERE description LIKE '%\%%';

-- 匹配包含字面下划线的字符串
SELECT * FROM products WHERE description LIKE '%\_%';

-- 结合多个通配符
SELECT * FROM users WHERE name LIKE 'a%b_c';
```

**注意事项**：
- LIKE运算符区分大小写
- 对于NULL值，LIKE运算符返回NULL
- 模式匹配可能会影响查询性能，特别是使用前缀通配符时
- 对于复杂模式，建议使用索引来提高性能

#### 逻辑运算符

- `AND`：逻辑与
- `OR`：逻辑或

#### 向量距离运算符

- `<->`：向量L2距离，用于计算欧几里得距离
- `<#>`：向量内积，用于计算向量点积
- `<=>`：向量余弦相似度，用于计算向量夹角余弦值

### 2.13 RBAC 相关语句

RemDB 支持基于角色的访问控制（RBAC），用于管理用户权限。

#### 2.13.1 CREATE ROLE语句

用于创建一个新的角色。

**语法**：
```sql
CREATE ROLE role_name;
```

**参数说明**：
- `role_name`：要创建的角色名称

**示例**：

```sql
-- 创建一个新的角色
CREATE ROLE admin;

-- 创建一个只读角色
CREATE ROLE read_only;
```

#### 2.13.2 DROP ROLE语句

用于删除一个现有的角色。

**语法**：
```sql
DROP ROLE role_name;
```

**参数说明**：
- `role_name`：要删除的角色名称

**示例**：

```sql
-- 删除一个角色
DROP ROLE admin;
```

#### 2.13.3 GRANT PERMISSION语句

用于授予权限给指定的角色。

**语法**：
```sql
GRANT permission ON [table_name [.column_name]] TO role_name;
```

**参数说明**：
- `permission`：要授予的权限，支持 `SELECT`、`INSERT`、`UPDATE`、`DELETE`
- `table_name`：可选，指定表名
- `column_name`：可选，指定列名
- `role_name`：要授予权限的角色名称

**示例**：

```sql
-- 授予所有权限给admin角色
GRANT ALL ON * TO admin;

-- 授予SELECT权限给read_only角色
GRANT SELECT ON users TO read_only;

-- 授予INSERT和UPDATE权限给user_manager角色
GRANT INSERT, UPDATE ON users TO user_manager;
```

#### 2.13.4 REVOKE PERMISSION语句

用于从指定的角色中撤销权限。

**语法**：
```sql
REVOKE permission ON [table_name [.column_name]] FROM role_name;
```

**参数说明**：
- `permission`：要撤销的权限
- `table_name`：可选，指定表名
- `column_name`：可选，指定列名
- `role_name`：要撤销权限的角色名称

**示例**：

```sql
-- 撤销admin角色的所有权限
REVOKE ALL ON * FROM admin;

-- 撤销read_only角色的SELECT权限
REVOKE SELECT ON users FROM read_only;
```

#### 2.13.5 CREATE USER语句

用于创建一个新的用户。

**语法**：
```sql
CREATE USER user_name;
```

**参数说明**：
- `user_name`：要创建的用户名称

**示例**：

```sql
-- 创建一个新的用户
CREATE USER alice;

-- 创建一个新的用户
CREATE USER bob;
```

#### 2.13.6 DROP USER语句

用于删除一个现有的用户。

**语法**：
```sql
DROP USER user_name;
```

**参数说明**：
- `user_name`：要删除的用户名称

**示例**：

```sql
-- 删除一个用户
DROP USER alice;
```

#### 2.13.7 GRANT ROLE语句

用于授予角色给指定的用户。

**语法**：
```sql
GRANT ROLE role_name TO user_name;
```

**参数说明**：
- `role_name`：要授予的角色名称
- `user_name`：要授予角色的用户名称

**示例**：

```sql
-- 授予admin角色给alice用户
GRANT ROLE admin TO alice;

-- 授予read_only角色给bob用户
GRANT ROLE read_only TO bob;
```

#### 2.13.8 REVOKE ROLE语句

用于从指定的用户中撤销角色。

**语法**：
```sql
REVOKE ROLE role_name FROM user_name;
```

**参数说明**：
- `role_name`：要撤销的角色名称
- `user_name`：要撤销角色的用户名称

**示例**：

```sql
-- 撤销admin角色从alice用户
REVOKE ROLE admin FROM alice;

-- 撤销read_only角色从bob用户
REVOKE ROLE read_only FROM bob;
```

### 2.14 函数支持

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

RemDB支持以下类型的函数：
1. **基础统计聚合函数**：COUNT、SUM、AVG等
2. **滑动窗口函数**：MOVING_SUM、MOVING_AVERAGE等
3. **窗口函数**：ROW_NUMBER、RANK、DENSE_RANK、NTILE、LAG、LEAD、FIRST_VALUE、LAST_VALUE等
4. **字符串函数**：字符串处理相关函数
5. **数学函数**：数学运算函数
6. **时间窗口函数**：TIME_BUCKET等
7. **时间转换函数**：时间格式转换函数
8. **AI模型UDF函数**：通过CREATE MODEL注册的AI模型函数
9. **向量函数**：向量距离和相似度计算函数
10. **JSON函数**：JSON数据处理函数

有关AI模型UDF函数的详细信息，请参见[2.10 模型管理语句](#210-模型管理语句)和[AI模型UDF函数](#ai模型udf函数)。

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

##### 窗口函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `ROW_NUMBER()` | 为每行分配唯一的序号，从1开始 | 无 | `INTEGER` | `ROW_NUMBER() OVER (ORDER BY score DESC)` |
| `RANK()` | 为每行分配排名，相同值的行具有相同排名，后续排名会跳跃 | 无 | `INTEGER` | `RANK() OVER (ORDER BY score DESC)` |
| `DENSE_RANK()` | 为每行分配排名，相同值的行具有相同排名，后续排名不会跳跃 | 无 | `INTEGER` | `DENSE_RANK() OVER (ORDER BY score DESC)` |
| `NTILE(n)` | 将结果集分成n个大致相等的桶，为每行分配桶号 | n：桶数 | `INTEGER` | `NTILE(4) OVER (ORDER BY score)` |
| `LAG(expression, [offset], [default])` | 返回当前行之前offset行的值 | expression：列或表达式<br>offset：偏移量，默认为1<br>default：当偏移超出范围时的默认值 | 与expression相同 | `LAG(score, 1, 0) OVER (ORDER BY date)` |
| `LEAD(expression, [offset], [default])` | 返回当前行之后offset行的值 | expression：列或表达式<br>offset：偏移量，默认为1<br>default：当偏移超出范围时的默认值 | 与expression相同 | `LEAD(score, 1, 0) OVER (ORDER BY date)` |
| `FIRST_VALUE(expression)` | 返回窗口内的第一个值 | expression：列或表达式 | 与expression相同 | `FIRST_VALUE(score) OVER (ORDER BY date)` |
| `LAST_VALUE(expression)` | 返回窗口内的最后一个值 | expression：列或表达式 | 与expression相同 | `LAST_VALUE(score) OVER (ORDER BY date)` |

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

##### JSON函数

| 函数名 | 描述 | 参数 | 返回类型 | 示例 |
|--------|------|------|----------|------|
| `JSON_EXTRACT` | 从JSON中提取指定路径的值 | `json_field`, `path` | 与提取值类型相同 | `JSON_EXTRACT(details, '$.address.city')` |
| `JSON_ARRAY_LENGTH` | 计算JSON数组的长度 | `json_field` | `INTEGER` | `JSON_ARRAY_LENGTH(details)` |
| `JSON_TYPE` | 返回JSON值的类型 | `json_field` | `TEXT` | `JSON_TYPE(details)` |
| `JSON_ARRAY` | 创建JSON数组 | 多个值 | `JSON` | `JSON_ARRAY(1, 'text', true)` |
| `JSON_OBJECT` | 创建JSON对象 | 键值对 | `JSON` | `JSON_OBJECT('name', 'John', 'age', 30)` |

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

##### AI模型UDF函数

RemDB支持AI模型作为用户定义函数（UDF），允许在SQL查询中直接调用预训练的AI模型进行推理。模型需要通过CREATE MODEL语句注册后使用。

**模型UDF调用语法**：
```sql
SELECT model_name(arg1, arg2, ...) FROM table_name;
```

**示例**：
```sql
-- 注册的文本嵌入模型
CREATE MODEL bge_embedding USING 'bge-m3.onnx' AS (text STRING) RETURNS VECTOR(768);

-- 在查询中使用模型UDF
SELECT bge_embedding(content) AS embedding FROM documents;
SELECT bge_embedding('Hello world') AS embedding;

-- 结合向量搜索
SELECT id, content, vector_distance(bge_embedding(content), query_vector) AS similarity
FROM documents
ORDER BY similarity DESC
LIMIT 10;

-- 多输入模型
CREATE MODEL multimodal USING 'model.onnx' AS (text STRING, image VECTOR(512)) RETURNS REAL;
SELECT multimodal(title, image_features) AS score FROM images;
```

**模型UDF特性**：
1. **动态加载**：模型在首次调用时加载，后续调用重用已加载的模型
2. **批处理支持**：支持批量处理，提高推理效率
3. **线程安全**：模型推理支持并发调用
4. **内存管理**：自动管理模型内存，防止内存泄漏
5. **错误处理**：提供详细的错误信息和堆栈跟踪

**支持的数据类型转换**：
- `STRING` → 模型输入：自动转换为嵌入向量（需要模型支持文本输入）
- `VECTOR(n)` → 模型输入：直接作为向量输入
- `REAL`/`INTEGER` → 模型输入：转换为浮点数张量
- 模型输出 → `VECTOR(n)`：返回向量结果
- 模型输出 → `REAL`：返回浮点数结果
- 模型输出 → `INTEGER`：返回整数结果

**性能优化建议**：
1. 对于批量处理，使用GROUP BY或窗口函数减少模型调用次数
2. 对于频繁调用的模型，考虑预热加载
3. 使用向量化查询减少数据传输开销
4. 结合索引优化查询性能

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
| JSON索引 | JSON字段 | 支持JSON路径索引和虚拟生成列 |

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

-- 创建JSON索引
CREATE INDEX idx_customer_details ON customer_data (details) USING BTREE;
```

## 4. 时序相关功能

### 4.1 时间戳数据类型

RemDB提供以下时间相关数据类型：

| 数据类型 | 描述 | 精度 | 存储大小 |
|---------|------|------|----------|
| `TIMESTAMP` | 时间戳类型 | 微秒级（默认），可调整 | 4-8字节（根据精度） |
| `TIMESTAMPTZ` | 带时区的时间戳类型 | 微秒级（默认），可调整 | 4-8字节（根据精度） |
| `INTERVAL` | 时间间隔类型 | 微秒级 | 4-8字节（根据精度） |

**示例**：

```sql
CREATE TABLE sensor_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sensor_id INTEGER NOT NULL,
    value REAL NOT NULL,
    timestamp TIMESTAMP NOT NULL
);

-- 创建带有时区的时间戳表
CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    event_name TEXT NOT NULL,
    event_time TIMESTAMPTZ NOT NULL,
    duration INTERVAL
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

### 4.3.1 SAMPLE BY 语法

RemDB 支持 `SAMPLE BY` 语法，用于对时序数据按照指定的时间间隔进行采样，是 `TIME_BUCKET` 函数的一种便捷替代方式。

**语法**：
```sql
SELECT column1, column2, ...
FROM table_name
[WHERE condition]
SAMPLE BY interval [ALIGN TO alignment_time]
[FILL fill_strategy]
[ORDER BY column [ASC | DESC]]
[LIMIT number];
```

**说明**：
- `interval`：时间间隔，支持与 `TIME_BUCKET` 相同的格式
- `ALIGN TO alignment_time`：可选，指定采样的对齐时间点

### 4.3.2 FILL 子句

RemDB 支持 `FILL` 子句，用于处理时序数据中的缺失值，为缺失的时间点填充适当的值。

**填充策略**：

| 填充策略 | 描述 | 示例 |
|---------|------|------|
| `PREV` | 使用前一个非空值填充 | `FILL PREV` |
| `LINEAR` | 使用线性插值填充 | `FILL LINEAR` |
| `NEXT` | 使用后一个非空值填充 | `FILL NEXT` |
| `value` | 使用指定的固定值填充（数值类型） | `FILL 0` 或 `FILL 2.5` |

**说明**：
- `FILL` 子句通常与 `SAMPLE BY` 或 `TIME_BUCKET` 一起使用，用于填充时间窗口聚合后产生的缺失值
- 当时间序列数据中存在时间间隔不均匀或缺失的数据点时，`FILL` 子句可以确保结果集中的时间序列是连续的
- 不同的填充策略适用于不同的业务场景：
  - `PREV`：适用于不希望数据突变的场景，如传感器读数
  - `LINEAR`：适用于数据变化较为平滑的场景，如温度变化
  - `NEXT`：适用于需要提前获取未来值的场景
  - `固定值`：适用于明确知道缺失值应该是什么的场景

**示例**：

```sql
-- 使用前一个值填充缺失数据
SELECT TIME_BUCKET('5m', timestamp) AS time_window, AVG(temperature) AS avg_temp
FROM sensor_readings
GROUP BY time_window
FILL PREV
ORDER BY time_window;

-- 使用线性插值填充缺失数据
SELECT timestamp, temperature
FROM sensor_readings
SAMPLE BY '1m'
FILL LINEAR;

-- 使用固定值填充缺失数据
SELECT timestamp, value
FROM metrics
SAMPLE BY '1h'
FILL 0
ORDER BY timestamp;

-- 结合WHERE条件使用FILL子句
SELECT TIME_BUCKET('15m', timestamp) AS time_window, SUM(value) AS total_value
FROM sensor_data
WHERE sensor_id = 1
GROUP BY time_window
FILL 0
ORDER BY time_window;

-- 使用SAMPLE BY与ALIGN TO
SELECT timestamp, temperature
FROM sensor_readings
SAMPLE BY '1h' ALIGN TO '2024-01-01 00:00:00'
FILL PREV;
```

`FILL` 子句为时序数据查询提供了灵活的缺失值处理能力，确保查询结果的连续性和完整性，便于后续的数据分析和可视化。

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

### 4.6 时序数据预聚合

RemDB支持时序数据的预聚合功能，可以在数据写入时自动计算并存储不同时间间隔的聚合结果，提高查询性能。

#### 4.6.1 添加预聚合配置

通过SQL语句为时序表添加预聚合配置：

```sql
-- 为时序表添加预聚合配置
ALTER TABLE timeseries_table ADD PRE_AGGREGATION INTERVAL 60 SECONDS AGGREGATION AVG;
ALTER TABLE timeseries_table ADD PRE_AGGREGATION INTERVAL 300 SECONDS AGGREGATION SUM;
```

#### 4.6.2 预聚合查询

查询预聚合的数据可以显著提高查询性能，特别是对于长时间范围的聚合查询：

```sql
-- 查询预聚合的1分钟平均值数据
SELECT time_bucket, value FROM timeseries_table WHERE time_bucket >= start_time AND time_bucket <= end_time PRE_AGGREGATED INTERVAL 60 SECONDS AGGREGATION AVG;

-- 查询预聚合的5分钟总和数据
SELECT time_bucket, value FROM timeseries_table WHERE time_bucket >= start_time AND time_bucket <= end_time PRE_AGGREGATED INTERVAL 300 SECONDS AGGREGATION SUM;
```

#### 4.6.3 支持的聚合函数

RemDB支持以下聚合函数用于预聚合：

- `SUM`：计算总和
- `AVG`：计算平均值
- `MIN`：计算最小值
- `MAX`：计算最大值

#### 4.6.4 预聚合原理

1. **数据存储**：预聚合数据存储在专用的哈希表中，键为（时间桶，标签哈希），值为聚合结果
2. **自动更新**：当新数据写入时，系统会自动更新所有相关的预聚合数据
3. **线程安全**：使用Mutex确保并发写入时的数据一致性
4. **高效查询**：查询时直接从预聚合存储中读取数据，避免实时计算

#### 4.6.5 预聚合使用场景

- **实时监控**：快速查询最近的聚合数据
- **历史数据分析**：高效查询长时间范围的聚合结果
- **仪表盘展示**：预计算常用时间间隔的聚合数据
- **告警系统**：基于预聚合数据进行阈值判断

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

## 5. JSON 支持

### 5.1 JSON 数据类型

RemDB 支持 `JSON` 数据类型，用于存储和处理 JSON 格式的数据。

**语法**：`JSON`

**说明**：
- `JSON` 类型可以存储任意有效的 JSON 数据，包括对象、数组、字符串、数字、布尔值和 null
- JSON 数据会被自动验证，确保存储的是有效的 JSON 格式
- 支持 JSON 路径查询和修改操作

**示例**：

```sql
-- 创建包含 JSON 字段的表
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    profile JSON
);

-- 插入包含 JSON 数据的记录
INSERT INTO users (name, profile) VALUES (
    'Alice',
    '{"age": 25, "email": "alice@example.com", "hobbies": ["reading", "hiking"]}'
);

-- 插入包含不同类型 JSON 数据的记录
INSERT INTO users (name, profile) VALUES (
    'Bob',
    '{"age": 30, "email": "bob@example.com", "hobbies": [], "active": true}'
);
```

### 5.2 JSON 函数

RemDB 提供了丰富的 JSON 函数，用于查询和修改 JSON 数据。

#### 5.2.1 JSON 提取函数

| 函数名 | 描述 | 示例 |
|--------|------|------|
| `JSON_EXTRACT(json, path)` | 从 JSON 中提取指定路径的值 | `JSON_EXTRACT(profile, '$.age')` |
| `JSON_VALUE(json, path)` | 从 JSON 中提取标量值 | `JSON_VALUE(profile, '$.name')` |
| `JSON_QUERY(json, path)` | 从 JSON 中提取对象或数组 | `JSON_QUERY(profile, '$.hobbies')` |
| `JSON_HAS(json, path)` | 检查 JSON 中是否存在指定路径 | `JSON_HAS(profile, '$.email')` |
| `JSON_TYPE(json, path)` | 返回 JSON 中指定路径的值类型 | `JSON_TYPE(profile, '$.age')` |

**示例**：

```sql
-- 提取 JSON 中的字段
SELECT 
    id,
    name,
    JSON_EXTRACT(profile, '$.age') AS age,
    JSON_EXTRACT(profile, '$.email') AS email
FROM users;

-- 检查 JSON 中是否存在字段
SELECT 
    id,
    name,
    JSON_HAS(profile, '$.hobbies') AS has_hobbies
FROM users;

-- 获取 JSON 值的类型
SELECT 
    id,
    name,
    JSON_TYPE(profile, '$.age') AS age_type,
    JSON_TYPE(profile, '$.hobbies') AS hobbies_type
FROM users;
```

#### 5.2.2 JSON 修改函数

| 函数名 | 描述 | 示例 |
|--------|------|------|
| `JSON_SET(json, path, value)` | 设置 JSON 中指定路径的值 | `JSON_SET(profile, '$.age', 26)` |
| `JSON_REMOVE(json, path)` | 从 JSON 中删除指定路径的值 | `JSON_REMOVE(profile, '$.email')` |
| `JSON_MERGE_PATCH(json1, json2)` | 使用 JSON Merge Patch 合并两个 JSON | `JSON_MERGE_PATCH(profile, '{"age": 26}')` |
| `JSON_ARRAY_APPEND(json, path, value)` | 向 JSON 数组中追加元素 | `JSON_ARRAY_APPEND(profile, '$.hobbies', 'coding')` |

**示例**：

```sql
-- 更新 JSON 中的字段
UPDATE users
SET profile = JSON_SET(profile, '$.age', 26)
WHERE id = 1;

-- 从 JSON 中删除字段
UPDATE users
SET profile = JSON_REMOVE(profile, '$.email')
WHERE id = 1;

-- 合并 JSON 数据
UPDATE users
SET profile = JSON_MERGE_PATCH(profile, '{"age": 27, "city": "New York"}')
WHERE id = 1;

-- 向 JSON 数组中添加元素
UPDATE users
SET profile = JSON_ARRAY_APPEND(profile, '$.hobbies', 'coding')
WHERE id = 1;
```

### 5.3 JSON 路径语法

RemDB 支持标准的 JSON 路径语法，用于指定 JSON 中的位置。

**基本语法**：
- `$`：表示 JSON 根对象
- `$.key`：表示根对象的 key 字段
- `$[index]`：表示数组的第 index 个元素
- `$.key[index]`：表示对象中数组字段的第 index 个元素

**示例**：

| JSON 路径 | 描述 |
|-----------|------|
| `$` | 整个 JSON 对象 |
| `$.name` | 根对象的 name 字段 |
| `$.hobbies[0]` | 根对象 hobbies 数组的第一个元素 |
| `$.address.city` | 根对象 address 对象的 city 字段 |

### 5.4 JSON 索引

RemDB 支持为 JSON 字段创建索引，提高 JSON 路径查询的性能。

**语法**：
```sql
CREATE INDEX index_name ON table_name ((JSON_EXTRACT(json_column, '$.path'))) [USING index_type];
```

**示例**：

```sql
-- 为 JSON 字段的 age 路径创建索引
CREATE INDEX idx_users_profile_age ON users ((JSON_EXTRACT(profile, '$.age')));

-- 为 JSON 字段的 city 路径创建索引
CREATE INDEX idx_users_profile_city ON users ((JSON_EXTRACT(profile, '$.address.city')));
```

### 5.5 JSON 与其他功能的结合

#### 5.5.1 JSON 与聚合函数

```sql
-- 计算 JSON 中年龄的平均值
SELECT AVG(JSON_EXTRACT(profile, '$.age')) AS avg_age
FROM users;

-- 统计不同城市的用户数量
SELECT 
    JSON_EXTRACT(profile, '$.address.city') AS city,
    COUNT(*) AS user_count
FROM users
GROUP BY city;
```

#### 5.5.2 JSON 与 WHERE 条件

```sql
-- 查询年龄大于 25 的用户
SELECT id, name
FROM users
WHERE JSON_EXTRACT(profile, '$.age') > 25;

-- 查询居住在特定城市的用户
SELECT id, name
FROM users
WHERE JSON_EXTRACT(profile, '$.address.city') = 'New York';
```

#### 5.5.3 JSON 与时序数据

```sql
-- 创建包含 JSON 字段的时序表
CREATE TIMESERIES TABLE sensor_data (
    timestamp TIMESTAMP,
    value REAL,
    metadata JSON
);

-- 插入包含 JSON 元数据的时序数据
INSERT INTO sensor_data (timestamp, value, metadata) VALUES (
    1609459200000,
    25.5,
    '{"sensor_id": "s1", "location": "room1", "battery": 90}'
);

-- 按传感器 ID 分组查询
SELECT 
    JSON_EXTRACT(metadata, '$.sensor_id') AS sensor_id,
    AVG(value) AS avg_value
FROM sensor_data
SAMPLE BY '1h'
GROUP BY sensor_id;
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
5. **LIKE运算符支持**：支持模式匹配，包括通配符（%、_）和转义字符
6. **内嵌函数支持**：
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