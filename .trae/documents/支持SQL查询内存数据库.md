# 支持SQL查询内存数据库

## 目标
实现对remdb内存数据库的SQL查询支持，允许用户使用标准SQL语法查询数据库中的数据。

## 实现步骤

### 1. 扩展SQL解析器
- 在`remdb-macros/src/ddl_parser.rs`中添加SELECT语句解析支持
- 支持的SQL语法：
  - 基本SELECT查询：`SELECT * FROM table_name`
  - 带条件的查询：`SELECT * FROM table_name WHERE column = value`
  - 指定列查询：`SELECT column1, column2 FROM table_name`
  - 带排序的查询：`SELECT * FROM table_name ORDER BY column ASC/DESC`
  - 带LIMIT的查询：`SELECT * FROM table_name LIMIT 10`

### 2. 实现查询引擎
- 在`src`目录下创建`sql`模块，包含：
  - `query_parser.rs`：解析SQL查询语句
  - `query_executor.rs`：执行查询并返回结果
  - `result_set.rs`：处理查询结果集

### 3. 扩展RemDb结构体
- 在`src/lib.rs`中的`RemDb`结构体添加`sql_query`方法
- 支持执行SQL SELECT语句并返回结果集

### 4. 实现查询执行逻辑
- 将SQL查询转换为对现有API的调用：
  - 对于简单查询，使用`iterate`方法遍历表
  - 对于带条件的查询，使用索引加速（如果存在合适的索引）
  - 对于带排序的查询，实现内存排序
  - 对于带LIMIT的查询，限制结果数量

### 5. 实现结果集处理
- 支持将查询结果转换为用户友好的格式
- 支持迭代访问结果集
- 支持获取结果集中的字段值

### 6. 添加示例和测试
- 在`examples`目录下添加SQL查询示例
- 在`tests`目录下添加SQL查询单元测试

## 预期效果
用户可以使用如下方式查询数据库：
```rust
let result = db.sql_query("SELECT * FROM users WHERE age > 18 ORDER BY name ASC LIMIT 10")?;
for row in result {
    println!("{}: {}", row.get("id"), row.get("name"));
}
```

## 技术要点
- 保持零外部依赖，使用纯Rust实现
- 支持no_std环境（部分功能可能需要std特性）
- 利用现有索引提高查询性能
- 实现高效的内存排序算法
- 提供友好的错误信息

## 注意事项
- 初始版本仅支持SELECT查询，不支持INSERT、UPDATE、DELETE等DML语句
- 支持的SQL语法范围有限，逐步扩展
- 保持与现有代码的兼容性
- 确保线程安全