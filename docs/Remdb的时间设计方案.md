Remdb的时间设计方案

一、核心设计目标

1.1 基本要求

· 类型简化: 提供两种核心时间类型，避免功能冗余
· 精度可调: 支持从秒到纳秒的多级精度控制
· 时区感知: 内置时区支持，简化跨时区应用开发
· 存储高效: 根据精度智能选择存储方案

1.2 设计原则

```
1. 默认安全: 推荐使用TIMESTAMPTZ，避免时区错误
2. 精度分级: 按需选择精度，平衡存储和业务需求
3. 存储优化: 自动压缩存储，减少空间占用
4. 查询友好: 内置时间分区和索引优化支持
```

二、时间类型规格

2.1 TIMESTAMP类型

```sql
-- 语法格式
TIMESTAMP[(p)]  -- p为精度值，0-9

-- 精度分级
TIMESTAMP(0)    -- 秒级（4字节存储）
TIMESTAMP(3)    -- 毫秒级（6字节）
TIMESTAMP(6)    -- 微秒级（8字节，默认）
TIMESTAMP(9)    -- 纳秒级（10字节）

-- 特性
- 时间范围: 1000-01-01 到 9999-12-31
- 无时区信息，按字面值存储
- 内部纪元: 2000-01-01 00:00:00 UTC
- 存储: 64位整数，表示微秒偏移
```

2.2 TIMESTAMPTZ类型

```sql
-- 语法格式
TIMESTAMPTZ[(p)]  -- 带时区版本

-- 特性
- 同TIMESTAMP的时间范围和精度
- 存储时转换为UTC
- 查询时按会话时区显示
- 额外存储时区偏移信息
```

三、函数规格

3.1 时间获取函数

```sql
-- 当前时间（带时区）
NOW([p])                 -- p:精度，默认6
CURRENT_TIMESTAMP([p])   -- 标准SQL别名

-- 当前时间（不带时区）
LOCALTIMESTAMP([p])      -- 按会话时区计算

-- 示例
NOW(0)     -- 返回: 2024-01-15 08:30:45+00
NOW(3)     -- 返回: 2024-01-15 08:30:45.123+00
NOW(6)     -- 返回: 2024-01-15 08:30:45.123456+00
```

3.2 时区函数

```sql
-- 时区转换
timestamp AT TIME ZONE 'timezone'

-- 时区设置
SET timezone = 'Asia/Shanghai'
SET timezone = '+08:00'

-- 时区函数
TIMEZONE('zone', timestamp)
```

四、存储引擎规格

4.1 内部存储格式

```c
// 底层数据结构
struct db_timestamp {
    int64_t value;     // 自2000-01-01的微秒数
    int16_t tz_offset; // 时区偏移（秒），TIMESTAMPTZ专用
    uint8_t precision; // 精度标记(0-9)
    uint8_t flags;     // 标志位
}

// 存储优化策略
精度 ≤ 0: 使用4字节 (秒级)
精度 ≤ 3: 使用6字节 (毫秒级)
精度 ≤ 6: 使用8字节 (微秒级)
精度 ≤ 9: 使用10字节(纳秒级)
```

4.2 分区支持

```sql
-- 自动时间分区（语法糖）
CREATE TABLE logs (
    id BIGINT,
    event_time TIMESTAMPTZ(6)
) PARTITION BY TIME(event_time) INTERVAL '1 month';

-- 手动分区语法
CREATE TABLE logs PARTITION BY RANGE (event_time);
```

五、索引优化规格

5.1 索引类型支持

```sql
-- B-tree索引（默认）
CREATE INDEX idx_time ON table (timestamp_column);

-- BRIN索引（时间序列专用）
CREATE INDEX idx_time_brin ON table 
USING BRIN(timestamp_column) 
WITH (pages_per_range = 32);

-- 部分索引（热数据优化）
CREATE INDEX idx_recent ON table (timestamp_column)
WHERE timestamp_column > NOW() - INTERVAL '30 days';
```

5.2 表达式索引

```sql
-- 时区转换索引
CREATE INDEX idx_local_time ON table 
((timestamp_column AT TIME ZONE 'Asia/Shanghai'));

-- 日期部分索引
CREATE INDEX idx_date ON table (DATE(timestamp_column));
```

六、输入输出规格

6.1 输入格式

```sql
-- 支持的格式（自动识别）
'2024-01-15 10:30:45'            -- 标准SQL
'2024-01-15T10:30:45.123Z'       -- ISO 8601
'2024-01-15 10:30:45.123+08'     -- 带时区偏移
1673778645123456                 -- 微秒时间戳

-- 显式转换
TIMESTAMP '2024-01-15 10:30:45'
CAST('2024-01-15 10:30:45' AS TIMESTAMPTZ(3))
```

6.2 输出格式

```sql
-- 格式化函数
TO_CHAR(timestamp, format)  -- 自定义格式
TO_ISO8601(timestamp)       -- ISO 8601格式
TO_EPOCH(timestamp)         -- Unix时间戳（秒）

-- 默认输出
SELECT NOW(3);  -- 2024-01-15 08:30:45.123+00
```

七、计算与运算规格

7.1 时间运算

```sql
-- 加减运算
timestamp + INTERVAL '1 day'
timestamp - INTERVAL '1 hour'

-- 时间差计算
timestamp1 - timestamp2  -- 返回INTERVAL
AGE(timestamp1, timestamp2)

-- 提取组件
EXTRACT(YEAR FROM timestamp)
EXTRACT(MICROSECOND FROM timestamp)
```

7.2 范围类型

```sql
-- 时间范围类型
TIMESTAMPTZ_RANGE  -- 带时区范围
TIMESTAMP_RANGE    -- 不带时区范围

-- 范围操作符
@>  -- 包含
&&  -- 重叠
<@  -- 被包含
```

八、配置参数

8.1 系统配置

```sql
-- 默认精度配置
SET default_timestamp_precision = 6;  -- 微秒

-- 时区配置
SET default_timezone = 'UTC';
SET timezone_aware_insert = on;      -- 插入时自动转换时区

-- 输出配置
SET timestamp_output_format = 'ISO-8601';
```

可以系统配置可通过remdbcli进行设置，也可以通过remdb的toml配置文件的系统参数栏配置。

九、性能指标要求

9.1 存储性能

```
1. 插入性能: >10万行/秒（单线程）
2. 查询性能: 范围查询<10ms（1亿数据）
3. 索引构建: <1分钟/亿行
4. 存储压缩: 比原始存储节省30-50%
```

9.2 精度性能对比

```
精度   存储大小   写入性能   查询性能
秒级   4字节     +30%       +10%
毫秒   6字节     +15%       +5%
微秒   8字节    基准       基准
纳秒   10字节    -20%       -15%
```

十、扩展功能规格

10.1 时间序列优化

```sql
-- 自动采样
CREATE CONTINUOUS AGGREGATE metrics_5min
AS SELECT
    time_bucket('5 minutes', timestamp) as bucket,
    avg(value) as avg_value
FROM metrics
GROUP BY bucket;
```

10.2 时区数据库

```sql
-- 内置时区支持
SELECT * FROM pg_timezone_names;

-- 自定义时区
CREATE TIME ZONE 'Custom/Zone' AS INTERVAL '+08:30';
```

十一、错误处理规格

11.1 输入验证

```
1. 超范围时间: 返回错误 "timestamp out of range"
2. 无效时区: 返回错误 "invalid time zone"
3. 精度溢出: 自动截断并警告
4. 格式错误: 返回解析错误信息
```

11.2 时区处理

```
1. 时区不存在: 使用最接近的已知时区
2. 时区转换歧义: 返回错误 "ambiguous time zone conversion"
3. 夏令时切换: 自动处理，支持历史规则
```

十二、测试用例示例

12.1 基础功能测试

```sql
-- 精度测试
SELECT NOW(0);  -- 应返回秒级时间
SELECT NOW(3);  -- 应返回毫秒级时间
SELECT NOW(6);  -- 应返回微秒级时间

-- 时区测试
SET timezone = 'Asia/Shanghai';
SELECT NOW();   -- 应显示+08时区时间

-- 存储测试
INSERT INTO test VALUES (NOW(6));
SELECT EXTRACT(MICROSECOND FROM ts) FROM test;
```

12.2 性能测试

```sql
-- 批量插入测试
INSERT INTO perf_test 
SELECT generate_series, NOW(6)
FROM generate_series(1, 1000000);

-- 范围查询测试
SELECT COUNT(*) FROM perf_test
WHERE ts BETWEEN NOW() - INTERVAL '1 day' AND NOW();
```

---

交付物清单

1. 核心类型: TIMESTAMP, TIMESTAMPTZ
2. 精度控制: 0-9级精度可调
3. 时区支持: 完整时区转换功能
4. 存储优化: 自动压缩存储方案
5. 分区支持: 内置时间分区语法
6. 索引优化: BRIN索引和表达式索引
7. 性能指标: 满足高并发时序场景需求

此规格说明书为技术实现提供明确指导，确保时间戳功能满足各类业务场景需求。