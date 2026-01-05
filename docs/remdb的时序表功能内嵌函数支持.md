# remdb的时序表功能内嵌函数支持

时序数据库（Time-Series Database）的聚合函数是数据分析的核心，它们专为处理按时间顺序产生的数据流而设计。除了标准SQL聚合函数外，时序数据库还提供了一系列时间感知和序列感知的高级聚合功能。

以下是时序场景中常用的聚合函数分类详解：

## 📊 1. 基础统计聚合（与SQL通用）

这些函数是数据分析的基石。

· COUNT()： 返回数据点数。
· SUM()： 求和。
· AVG()： 计算平均值。
· MIN() / MAX()： 查找时间窗口内的最小值和最大值（在监控场景中非常关键，用于发现峰值和谷值）。
· STDDEV() / VARIANCE()： 计算标准差和方差，用于衡量数据的波动性或稳定性。

```sql
-- 计算过去1小时内传感器sensor_001的平均值和峰值
SELECT 
    AVG(temperature) as avg_temp,
    MAX(temperature) as max_temp,
    MIN(temperature) as min_temp
FROM sensor_readings
WHERE sensor_id = 'sensor_001' 
  AND ts >= NOW() - INTERVAL '1 hour';
```

## ⏳ 2. 时间窗口聚合（时序核心）

这是时序数据库最具特色的功能，用于将连续的时间流切分成独立的窗口（桶）进行分段计算。

· 窗口函数： 使用DATE_TRUNC、TIME_BUCKET、date_bin等函数将时间戳对齐到指定的窗口（如5分钟、1小时）。
· GROUP BY： 与窗口函数结合，实现按时间窗口分组聚合。

```sql
-- 按每5分钟分组，聚合所有传感器的温度数据
-- 以下是一个类TimescaleDB的示例语法
SELECT 
    TIME_BUCKET('5 minutes', ts) as five_min_interval,
    sensor_id,
    AVG(temperature) as avg_temp,
    MAX(temperature) as max_temp
FROM sensor_readings
WHERE ts >= NOW() - INTERVAL '1 day'
GROUP BY five_min_interval, sensor_id
ORDER BY five_min_interval DESC;
```

## 📈 3. 专业时序与高级聚合

专业时序数据库提供了更丰富的序列操作函数。

· DERIVATIVE()： 计算时间序列的导数（变化率），常用于分析速度、增长率或趋势变化。
· INTEGRAL()： 计算时间序列的积分，常用于计算流量累计值（如总用电量）。
· MOVING_AVERAGE() / MOVING_SUM()： 计算滑动窗口平均值/和，用于平滑数据、观察趋势。
· NON_NEGATIVE_DERIVATIVE()： 专门处理计数器（只增不减）的导数，避免重置计数器时产生负值。
· DIFFERENCE()： 计算相邻数据点的差值。
· ELAPSED()： 返回相邻时间戳之间的时间间隔。

```sql
-- 计算CPU使用率的5分钟滑动平均，以平滑瞬时尖峰
SELECT 
    ts,
    host,
    MOVING_AVERAGE(cpu_usage, 5) OVER (PARTITION BY host ORDER BY ts) as cpu_smoothed
FROM host_metrics;
```

## 🔍 4. 特殊值与下采样聚合

处理数据缺失和不均匀采样是时序场景的常见挑战。

· FIRST() / LAST()： 返回时间窗口内第一个和最后一个值。
· PERCENTILE() / MEDIAN()： 计算百分位数和中位数，比平均值更能抵抗异常值干扰。
· MODE()： 返回出现频率最高的值（众数）。
· COUNT_DISTINCT()： 统计唯一值的数量。
· DISTINCT()： 返回唯一值列表。
· TOP() / BOTTOM()： 返回值最大/最小的N个序列。
· SAMPLE()： 随机采样，或返回指定数量样本（用于下采样，降低可视化数据量）。
· 聚合选择器： 如 LAST_VALUE(field) AT MAX(time)，用于“在最大值出现的时间点，选取另一个字段的值”。

## 🔄 5. 数据插值与填补

当数据点因各种原因缺失时，某些时序引擎提供插值功能。

· INTERPOLATE() 或 FILL()： 用前值、线性插值或特定值填充缺失的时间点。这通常在查询时指定。

```sql
-- 查询时，对缺失的5分钟窗口用前一个非空值进行填充
SELECT 
    TIME_BUCKET('5 minutes', ts),
    AVG(temperature) as avg_temp
FROM sensor_readings
GROUP BY 1
ORDER BY 1
FILL(PREVIOUS); -- 填充策略：PREVIOUS, LINEAR, NULL, 0等
```