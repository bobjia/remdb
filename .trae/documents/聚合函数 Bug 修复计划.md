# 聚合函数 Bug 修复计划

## 问题描述

数据库引擎的聚合函数（COUNT、SUM、AVG、MIN、MAX）没有正确聚合所有行，而是对每行单独计算。

## 根本原因分析

在 [query_executor.rs](d:\workspace\remdb-server\remdb\src\sql\query_executor.rs#L1540-L1603) 中的 `evaluate_expression_for_aggregate` 函数存在严重 bug：

```rust
fn evaluate_expression_for_aggregate(
    args: &[Expression],
    _record_values: &[TypedValue],  // <-- 问题：record_values 被完全忽略！
) -> Result<TypedValue, QueryExecutionError> {
    // ...
    match arg {
        Expression::Constant { value, .. } => {
            // 只处理常量情况
        }
        _ => {
            // 对于 Expression::Field 等情况，直接返回默认值 u64: 1
            // 这是 bug 的根源！
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: 1 },
            })
        }
    }
}
```

**问题核心**：
1. `_record_values` 参数被完全忽略（使用 `_` 前缀表示未使用）
2. 当参数是字段引用（`Expression::Field`）时，函数没有从 `record_values` 中获取实际的字段值
3. 相反，它返回一个默认值 `u64: 1`

**影响**：
- `COUNT(*)`: 恰好正确（每行返回1，累加得到正确计数）
- `SUM(field)`: 错误 - 返回行数而非字段值总和
- `AVG(field)`: 错误 - 返回1.0而非实际平均值
- `MIN(field)`: 错误 - 返回1而非实际最小值
- `MAX(field)`: 错误 - 返回1而非实际最大值

## 修复方案

### 修改文件
- `d:\workspace\remdb-server\remdb\src\sql\query_executor.rs`

### 修改内容

在 `evaluate_expression_for_aggregate` 函数中添加对 `Expression::Field` 的处理：

```rust
fn evaluate_expression_for_aggregate(
    args: &[Expression],
    record_values: &[TypedValue],  // 移除 _ 前缀，使用这个参数
) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Ok(TypedValue {
            value_type: DataType::UInt64,
            value: Value { u64: 1 },
        });
    }

    let arg = &args[0];
    match arg {
        Expression::Constant { value, .. } => {
            // 现有的常量处理逻辑保持不变
            // ...
        }
        Expression::Field { name, .. } => {
            // 新增：从 record_values 中获取字段值
            // 需要根据字段名找到对应的索引，然后返回 record_values[index]
            // 这需要访问表结构信息来确定字段索引
        }
        _ => {
            // 其他情况返回默认值
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: 1 },
            })
        }
    }
}
```

### 实现细节

由于 `evaluate_expression_for_aggregate` 函数当前没有访问表结构信息，需要考虑以下方案之一：

**方案 A**: 修改函数签名，传入表结构信息
- 优点：可以直接根据字段名查找索引
- 缺点：需要修改调用处

**方案 B**: 修改调用处，预先计算字段索引并传入
- 优点：更高效，避免重复查找
- 缺点：需要较大改动

**方案 C**: 在调用 `evaluate_expression_for_aggregate` 之前，先计算好表达式的值
- 观察代码发现，`record_values` 已经包含了所有字段的值
- 需要知道字段在 `record_values` 中的索引

查看调用上下文，`process_aggregate_query` 函数中：
- `rows_to_process` 是 `Vec<Vec<TypedValue>>`，每个 `Vec<TypedValue>` 是一行的所有字段值
- 需要知道聚合函数参数对应的字段索引

### 推荐方案

修改 `evaluate_expression_for_aggregate` 函数，添加一个字段索引映射参数：

```rust
fn evaluate_expression_for_aggregate(
    args: &[Expression],
    record_values: &[TypedValue],
    field_index_map: &HashMap<String, usize>,  // 新增：字段名到索引的映射
) -> Result<TypedValue, QueryExecutionError> {
    // ...
}
```

## 实施步骤

1. **分析现有代码结构**
   - 确认 `record_values` 中字段值的顺序
   - 确认如何获取字段名到索引的映射

2. **修改 `evaluate_expression_for_aggregate` 函数**
   - 添加字段索引映射参数
   - 实现 `Expression::Field` 分支的处理逻辑

3. **修改调用处**
   - 在 `process_aggregate_query` 中构建字段索引映射
   - 更新函数调用

4. **添加/更新测试**
   - 验证 SUM、AVG、MIN、MAX 返回正确的聚合结果
   - 添加边界情况测试（空表、单行、多行）

## 验证方法

运行现有测试并添加新的断言：
```bash
cargo test test_sql_aggregate_functions --features std
cargo test test_sql_statistical_functions --features std
```
