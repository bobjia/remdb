# RemDB AI向量数据库架构设计文档

## 1. 架构概述

### 1.1 设计理念
RemDB AI向量数据库采用"AI原生"设计理念，将向量作为一等公民数据类型，构建集存储、计算、推理于一体的现代向量数据库系统。系统遵循"单一二进制、无外部依赖"原则，提供生产级的可靠性、性能和易用性。

### 1.2 核心设计原则
- **AI原生**：向量作为基础数据类型，支持原生向量操作
- **混合查询**：向量相似度搜索与标量过滤无缝集成
- **计算下推**：模型推理靠近数据存储，减少数据传输
- **生产就绪**：ACID事务、WAL日志、检查点机制
- **可观测性**：全面的监控指标和结构化日志

### 1.3 系统架构图
```
┌─────────────────────────────────────────────────────────────┐
│                   客户端层 (Client Layer)                    │
├─────────────────────────────────────────────────────────────┤
│  Python SDK    Go SDK    Java SDK    HTTP REST    gRPC     │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   查询引擎层 (Query Engine)                  │
├─────────────────────────────────────────────────────────────┤
│  AINQL解析器   查询优化器   执行计划器   结果组装器          │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   存储引擎层 (Storage Engine)                │
├─────────────────────────────────────────────────────────────┤
│  事务管理器   WAL管理器   索引管理器   向量存储引擎          │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   模型运行时层 (Model Runtime)               │
├─────────────────────────────────────────────────────────────┤
│  嵌入模型    重排序模型    UDF管理器   模型工作进程          │
└─────────────────────────────────────────────────────────────┘
```

## 2. 核心组件架构

### 2.1 向量数据模型

#### 2.1.1 向量数据类型定义
```sql
-- 向量作为一等公民数据类型
CREATE TABLE documents (
    id INT PRIMARY KEY,
    embedding VECTOR(768) WITH DISTANCE=COSINE QUANTIZATION=PQ,
    content STRING,
    category STRING,
    timestamp TIMESTAMP
);
```

#### 2.1.2 向量存储格式
- **内存布局**：连续内存块存储，支持SIMD优化
- **磁盘格式**：列式存储，支持向量压缩和量化
- **索引格式**：HNSW图结构、IVF_FLAT倒排列表

#### 2.1.3 距离度量支持
- **余弦相似度** (COSINE)：`1 - (A·B)/(||A||·||B||)`
- **欧氏距离** (L2)：`√Σ(A_i - B_i)²`
- **内积** (IP)：`A·B`

### 2.2 存储引擎架构

#### 2.2.1 事务管理
```rust
// 事务管理器核心接口
trait TransactionManager {
    fn begin_transaction() -> TransactionId;
    fn commit(transaction_id: TransactionId) -> Result<(), Error>;
    fn rollback(transaction_id: TransactionId) -> Result<(), Error>;
    fn write_wal(operation: Operation) -> Result<(), Error>;
}
```

#### 2.2.2 WAL机制设计
- **WAL文件结构**：固定大小文件轮转，避免无限增长
- **刷盘策略**：
  - `per_write`：每次写入都刷盘，最高可靠性
  - `per_N_seconds`：定时刷盘，平衡性能与可靠性
- **恢复机制**：重启时重放WAL日志，保证数据一致性

#### 2.2.3 检查点机制
- **自动检查点**：基于时间或WAL大小阈值触发
- **手动检查点**：支持`CREATE CHECKPOINT`命令
- **增量检查点**：仅同步变更数据，减少IO开销

### 2.3 索引管理器架构

#### 2.3.1 HNSW索引设计
```rust
struct HNSWIndex {
    layers: Vec<GraphLayer>,    // 分层图结构
    ef_construction: usize,     // 构建时候选集大小
    M: usize,                   // 每层最大连接数
    distance: DistanceMetric,   // 距离度量
}
```

#### 2.3.2 IVF_FLAT索引设计
```rust
struct IVFIndex {
    centroids: Vec<Vector>,     // 聚类中心
    inverted_lists: Vec<Vec<VectorId>>, // 倒排列表
    nlist: usize,               // 聚类数量
    nprobe: usize,              // 搜索时探测的聚类数
}
```

#### 2.3.3 在线索引构建
- **后台构建**：索引构建不阻塞前端读写操作
- **进度监控**：`SHOW INDEX BUILD STATUS`命令
- **增量更新**：支持向量数据的增量索引更新

## 3. 查询引擎架构

### 3.1 AINQL查询语言设计

#### 3.1.1 查询语法规范
```sql
-- 混合查询示例
SELECT 
    id, 
    content,
    cosine_distance(embedding, $query_vec) AS similarity
FROM documents
WHERE 
    VECTOR_SIMILAR(embedding, $query_vec, K=10) 
    AND category = '科技' 
    AND timestamp > '2023-01-01'
ORDER BY similarity DESC
LIMIT 10
TIMEOUT 500ms;
```

#### 3.1.2 查询优化器
- **代价模型**：基于统计信息的查询代价估算
- **执行计划**：向量搜索与标量过滤的最优执行顺序
- **索引选择**：自动选择最优索引类型和参数

### 3.2 查询执行流程

#### 3.2.1 查询解析阶段
1. **语法解析**：将AINQL转换为抽象语法树(AST)
2. **语义分析**：验证表结构、字段类型、权限
3. **查询重写**：优化查询逻辑，消除冗余操作

#### 3.2.2 执行计划生成
1. **逻辑计划**：生成逻辑执行计划
2. **物理计划**：转换为具体的物理操作符
3. **计划优化**：基于代价模型优化执行计划

#### 3.2.3 查询执行
1. **向量搜索**：使用索引执行相似度搜索
2. **标量过滤**：应用WHERE条件过滤
3. **结果排序**：按相似度排序返回Top-K结果

## 4. 模型运行时架构

### 4.1 模型工作进程设计

#### 4.1.1 进程隔离架构
```
主数据库进程 (remdb-server)
    ├── 存储引擎
    ├── 查询引擎
    └── 模型管理器
        └── 模型工作进程 (model-worker)
            ├── 嵌入模型 (BGE-M3)
            ├── 重排序模型
            └── 自定义模型
```

#### 4.1.2 资源管理
- **CPU隔离**：可配置模型工作进程的CPU核心数
- **内存限制**：设置模型推理的内存上限
- **容错机制**：模型进程崩溃不影响主服务

### 4.2 计算下推机制

#### 4.2.1 模型UDF定义
```sql
-- 注册嵌入模型UDF
CREATE MODEL bge_embedding 
USING 'bge-m3.onnx' 
AS (text STRING) 
RETURNS VECTOR(768);

-- 在查询中使用UDF
SELECT bge_embedding(content) AS embedding FROM documents;
```

#### 4.2.2 数据传输优化
- **列式处理**：批量处理文本数据，减少函数调用开销
- **流水线执行**：模型推理与向量搜索并行执行
- **结果缓存**：缓存常用查询的模型推理结果

## 5. 客户端协议架构

### 5.1 多协议支持

#### 5.1.1 gRPC协议（高性能）
```protobuf
service VectorDB {
    rpc Query(QueryRequest) returns (QueryResponse);
    rpc Insert(InsertRequest) returns (InsertResponse);
    rpc CreateIndex(CreateIndexRequest) returns (CreateIndexResponse);
}

message QueryRequest {
    string query = 1;
    repeated float query_vector = 2;
    int32 limit = 3;
    map<string, string> filters = 4;
}
```

#### 5.1.2 RESTful HTTP协议（易调试）
```http
POST /v1/query
Content-Type: application/json

{
    "query": "SELECT * FROM docs WHERE VECTOR_SIMILAR(embedding, $1)",
    "params": [[0.1, 0.2, 0.3, ...]],
    "limit": 10
}
```

### 5.2 SDK设计

#### 5.2.1 Python SDK（功能完整）
```python
import remdb

# 同步客户端
client = remdb.Client(host='localhost', port=9000)
result = client.query(
    "SELECT * FROM docs WHERE VECTOR_SIMILAR(embedding, $1)",
    params=[query_vector]
)

# 异步客户端
async with remdb.AsyncClient() as client:
    result = await client.query(...)
```

#### 5.2.2 Go/Java SDK（基础功能）
- 支持核心CRUD操作
- 向量搜索和混合查询
- 连接池和重试机制

## 6. 部署与运维架构

### 6.1 单一二进制部署

#### 6.1.1 无外部依赖设计
- **静态链接**：所有依赖库静态链接到可执行文件
- **跨平台支持**：Linux、Windows、macOS
- **最小化运行时**：无需Java、Python等外部运行时

#### 6.1.2 配置文件管理
```toml
# remdb.toml
[server]
listen_address = "0.0.0.0:9000"
max_connections = 1000

[storage]
data_directory = "./data"
wal_size_limit = "1GB"

[model_runtime]
worker_cpu_cores = 2
worker_memory_limit = "2GB"
```

### 6.2 安全架构

#### 6.2.1 认证机制
- **API Key认证**：支持多API Key动态管理
- **传输加密**：TLS 1.2+加密通信
- **请求限流**：基于IP和API Key的请求频率限制

#### 6.2.2 网络安全
- **监听地址配置**：支持绑定特定IP和端口
- **防火墙集成**：与系统防火墙规则集成
- **访问日志**：记录所有客户端访问信息

### 6.3 可观测性架构

#### 6.3.1 监控指标
```prometheus
# 查询性能指标
vecdb_query_duration_seconds_bucket{le="0.1"} 1500
vecdb_query_duration_seconds_bucket{le="0.5"} 3000

# 索引性能指标
vecdb_index_cache_hit_rate 0.85
vecdb_index_recall_rate 0.98

# 系统资源指标
vecdb_active_connections 45
vecdb_wal_size_bytes 1073741824
```

#### 6.3.2 结构化日志
```json
{
    "timestamp": "2023-01-01T10:00:00Z",
    "level": "INFO",
    "request_id": "req-123456",
    "user": "api-key-1",
    "operation": "query",
    "duration_ms": 45,
    "query": "SELECT ...",
    "result_count": 10,
    "recall_rate": 0.99
}
```

#### 6.3.3 运维端点
- `/health`：服务健康状态检查
- `/metrics`：Prometheus格式监控指标
- `/config`：当前配置查看
- `/debug/pprof`：性能剖析数据

## 7. 数据流与处理架构

### 7.1 流式数据摄取

#### 7.1.1 Kafka集成
```sql
-- 创建数据流
CREATE STREAM doc_ingestion 
FROM KAFKA TOPIC 'documents' 
WITH CONFIG (
    bootstrap_servers='kafka:9092',
    group_id='remdb-consumer'
);
```

#### 7.1.2 实时索引更新
- **增量索引**：新数据自动添加到现有索引
- **批量优化**：积累一定量数据后批量构建索引
- **一致性保证**：流处理与事务的原子性保证

### 7.2 缓存架构

#### 7.2.1 多级缓存设计
- **查询结果缓存**：缓存常用查询的结果
- **向量数据缓存**：缓存热点向量数据
- **索引元数据缓存**：缓存索引结构和统计信息

#### 7.2.2 缓存策略
- **LRU淘汰**：最近最少使用淘汰策略
- **TTL过期**：基于时间的数据过期机制
- **动态调整**：根据工作负载自动调整缓存大小

## 8. 数据谱系与版本化

### 8.1 数据谱系追踪

#### 8.1.1 谱系信息记录
```sql
-- 每条数据的完整谱系
SELECT lineage FROM data_lineage WHERE data_id = 123;

-- 返回结果示例
{
    "source_file": "documents.pdf",
    "extraction_position": "page_3_paragraph_2",
    "embedding_model": "BGE-M3-v1.0",
    "processing_time": "2023-01-01T10:00:00Z",
    "processing_pipeline": "pdf_extract->text_clean->embedding"
}
```

### 8.2 数据版本化

#### 8.2.1 快照机制
```sql
-- 创建数据快照
CREATE SNAPSHOT docs_20230101 ON documents;

-- 查询历史数据
SELECT * FROM documents@docs_20230101 WHERE id = 123;

-- 从快照恢复
RESTORE TABLE documents FROM SNAPSHOT docs_20230101;
```

#### 8.2.2 快照存储优化
- **写时复制**：快照创建不影响当前数据操作
- **增量快照**：仅存储变更数据，节省存储空间
- **分布式存储**：支持快照的分布式存储和备份

## 9. 性能与扩展性设计

### 9.1 性能目标

#### 9.1.1 查询性能
- **单次查询延迟**：P95 < 50ms（千万级数据）
- **吞吐量**：> 10,000 QPS（gRPC协议）
- **混合查询损耗**：< 30%（相较于纯向量查询）

#### 9.1.2 索引性能
- **索引构建**：在线构建，不阻塞读写
- **索引加载**：千万级 < 1分钟，亿级 < 5分钟
- **索引精度**：Recall@10 > 98%

### 9.2 扩展性设计

#### 9.2.1 垂直扩展
- **多核优化**：充分利用多核CPU的并行计算能力
- **内存管理**：高效的内存分配和垃圾回收策略
- **IO优化**：异步IO和批量操作减少系统调用

#### 9.2.2 水平扩展准备
- **数据分片**：为未来分布式版本预留分片接口
- **一致性哈希**：支持数据在多个节点间的分布
- **副本机制**：为数据冗余和故障恢复做准备

## 10. 容错与可靠性设计

### 10.1 故障恢复

#### 10.1.1 进程级容错
- **主进程监控**：监控主数据库进程状态
- **自动重启**：检测到异常时自动重启服务
- **优雅关闭**：收到终止信号时完成当前操作再退出

#### 10.1.2 数据一致性保证
- **WAL重放**：重启时通过WAL日志恢复数据一致性
- **检查点验证**：定期验证检查点的完整性
- **数据校验和**：存储数据时计算校验和，检测数据损坏

### 10.2 负载保护

#### 10.2.1 资源限制
- **连接数限制**：防止过多连接耗尽资源
- **内存使用限制**：设置内存使用上限，防止OOM
- **查询超时**：所有查询支持超时配置

#### 10.2.2 熔断机制
- **查询熔断**：系统负载过高时拒绝新查询
- **优雅降级**：在资源紧张时提供基础功能
- **负载均衡**：为未来多节点部署准备负载均衡策略

## 11. 技术栈选择

### 11.1 编程语言
- **Rust**：系统核心组件，保证内存安全和性能
- **C++**：高性能计算组件（如向量运算）
- **Python**：SDK和工具链开发

### 11.2 关键依赖库
- **Tokio**：异步运行时，处理高并发IO
- **Serde**：序列化/反序列化，用于配置和协议
- **Prost**：gRPC协议生成
- **ONNX Runtime**：模型推理引擎
- **Faiss**：向量索引算法（可选，可能自研）

## 12. 实施路线图

### 12.1 阶段一：MVP实现（当前阶段）
1. **向量数据模型**：实现基础向量类型和CRUD操作
2. **存储引擎**：实现WAL、事务、检查点机制
3. **索引管理**：实现HNSW和IVF_FLAT索引
4. **查询引擎**：实现AINQL解析和执行

### 12.2 阶段二：核心优化
1. **模型运行时**：集成嵌入模型和计算下推
2. **流式处理**：实现Kafka集成和实时索引
3. **数据谱系**：实现数据版本化和谱系追踪
4. **可观测性**：完善监控指标和日志系统

### 12.3 阶段三：生产就绪
1. **安全加固**：完善认证和加密机制
2. **性能优化**：系统级性能调优和压力测试
3. **文档完善**：用户文档和运维指南
4. **生态建设**：丰富SDK和工具链

---

## 总结

RemDB AI向量数据库架构设计遵循"AI原生、生产就绪"的核心原则，通过分层架构设计实现了向量存储、混合查询、模型推理的一体化集成。系统在保证高性能的同时，提供了企业级的可靠性、安全性和可观测性。

该架构为AI应用开发者提供了开箱即用的向量数据库解决方案，同时为未来的功能扩展和技术演进预留了充分的设计空间。