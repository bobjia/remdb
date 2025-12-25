# 实现remdb DDL CREATE INDEX支持

## 目标
扩展remdb的DDL语言，支持使用 `CREATE INDEX` 语句创建索引，并能指定索引类型。

## 实现步骤

### 1. 扩展数据结构
- 在 `remdb-macros/src/ddl_parser.rs` 中添加 `IndexDef` 结构体，用于表示索引定义
- 扩展 `TableDef` 结构体，添加 `indices` 字段来存储表的索引列表

### 2. 扩展DDL解析器
- 在 `parse` 方法中添加对 `CREATE INDEX` 语句的支持
- 实现 `parse_create_index` 方法，解析索引创建语句
- 支持解析索引名称、表名、索引类型和索引列

### 3. 支持的索引类型
- Hash
- SortedArray
- BTree
- TTree
- 默认使用BTree

### 4. 语法支持
- 支持 `CREATE INDEX index_name ON table_name USING index_type (column_name);` 语法
- 支持不指定索引类型，默认使用BTree

### 5. 代码生成
- 扩展代码生成逻辑，将索引信息转换为相应的Rust代码
- 确保生成的代码能正确使用指定的索引类型

## 实现细节

### 1. 数据结构扩展
```rust
// 索引定义
#[derive(Debug, Clone)]
pub struct IndexDef {
    pub name: String,
    pub table_name: String,
    pub index_type: String,
    pub column_name: String,
}

// 扩展TableDef
pub struct TableDef {
    // 现有字段...
    pub indices: Vec<IndexDef>,
}
```

### 2. DDL解析器扩展
- 在 `parse` 方法中添加：
  ```rust
  if self.match_keyword("CREATE") {
      self.skip_whitespace();
      if self.match_keyword("TABLE") {
          // 现有CREATE TABLE处理
      } else if self.match_keyword("INDEX") {
          let index = self.parse_create_index()?;
          // 将索引添加到对应的表
          self.add_index_to_table(&index, &mut tables)?;
      }
  }
  ```

- 实现 `parse_create_index` 方法，解析：
  - 索引名称
  - ON 关键字
  - 表名
  - USING 关键字（可选）
  - 索引类型（可选，默认BTree）
  - 索引列

### 3. 索引类型映射
- Hash -> IndexType::Hash
- SortedArray -> IndexType::SortedArray
- BTree -> IndexType::BTree
- TTree -> IndexType::TTree
- 默认 -> IndexType::BTree

## 测试计划
- 添加测试用例，验证CREATE INDEX语句解析
- 测试不同索引类型的指定
- 测试默认索引类型（BTree）的使用
- 测试索引与表的关联

## 预期效果
用户可以使用如下DDL语法创建索引：
```sql
CREATE INDEX idx_name ON users USING hash (name);
CREATE INDEX idx_age ON users USING btree (age);
CREATE INDEX idx_active ON users (active); -- 默认使用BTree
```