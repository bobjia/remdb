# 实现B-Tree和T-Tree索引类型

## 需求分析
根据US-212需求，需要添加两种新的索引类型：
- B-Tree索引：适用于范围查询和顺序访问
- T-Tree索引：适用于频繁更新的场景

## 实现策略
1. **添加新的索引数据结构**
   - 为B-Tree定义节点结构和树结构
   - 为T-Tree定义节点结构和树结构
   - 保持与现有索引系统的兼容性

2. **实现核心方法**
   - `new`：创建新索引
   - `calculate_memory_size`：计算所需内存
   - `insert`：插入索引项
   - `delete`：删除索引项
   - `find`：查找索引项
   - `find_range`/`find_range_all`：范围查询

3. **确保系统集成**
   - 与现有TableDef和Table结构兼容
   - 支持内存管理系统
   - 支持事务系统
   - 支持统计信息收集

## 实现步骤

### 1. 定义B-Tree数据结构
```rust
// B-Tree节点结构
#[repr(C)]
pub struct BTreeNode {
    // 节点类型（内部节点/叶子节点）
    is_leaf: bool,
    // 当前键数量
    key_count: u8,
    // 键数据（每个键64字节）
    keys: [SecondaryIndexItem; BTREE_ORDER],
    // 子节点指针（仅内部节点使用）
    children: [Option<NonNull<BTreeNode>>; BTREE_ORDER + 1],
}

// B-Tree索引结构
pub struct BTreeIndex {
    // 表定义
    def: &'static TableDef,
    // 根节点
    root: Option<NonNull<BTreeNode>>,
    // 节点池
    nodes: NonNull<BTreeNode>,
    // 空闲节点链表
    free_nodes: Option<NonNull<BTreeNode>>,
    // 最大节点数量
    max_nodes: usize,
    // 索引统计信息
    stats: IndexStats,
    // 自旋锁
    lock: u32,
}
```

### 2. 定义T-Tree数据结构
```rust
// T-Tree节点结构
#[repr(C)]
pub struct TTreeNode {
    // 当前键数量
    key_count: u8,
    // 键数据（每个键64字节）
    keys: [SecondaryIndexItem; TTREE_ORDER],
    // 左子节点
    left: Option<NonNull<TTreeNode>>,
    // 中子节点（用于T-Tree的三元分支）
    middle: Option<NonNull<TTreeNode>>,
    // 右子节点
    right: Option<NonNull<TTreeNode>>,
}

// T-Tree索引结构
pub struct TTreeIndex {
    // 表定义
    def: &'static TableDef,
    // 根节点
    root: Option<NonNull<TTreeNode>>,
    // 节点池
    nodes: NonNull<TTreeNode>,
    // 空闲节点链表
    free_nodes: Option<NonNull<TTreeNode>>,
    // 最大节点数量
    max_nodes: usize,
    // 索引统计信息
    stats: IndexStats,
    // 自旋锁
    lock: u32,
}
```

### 3. 实现B-Tree核心方法
- `new`：初始化B-Tree索引
- `calculate_memory_size`：计算所需内存大小
- `insert`：插入索引项，自动平衡树
- `delete`：删除索引项，自动平衡树
- `find`：查找索引项
- `find_range`/`find_range_all`：范围查询

### 4. 实现T-Tree核心方法
- `new`：初始化T-Tree索引
- `calculate_memory_size`：计算所需内存大小
- `insert`：插入索引项
- `delete`：删除索引项
- `find`：查找索引项
- `find_range`/`find_range_all`：范围查询

### 5. 优化和测试
- 确保线程安全
- 优化内存使用
- 添加统计信息收集
- 确保与现有系统兼容

## 预期效果
- 支持B-Tree和T-Tree索引类型
- 索引类型可在编译时配置
- B-Tree索引在范围查询时比有序数组索引快5倍以上
- T-Tree索引在随机更新场景下比B-Tree快3倍以上
- 新索引类型的内存开销不超过数据大小的50%
- 索引维护与数据操作保持原子性

## 技术约束
- 每表最多支持3个索引（包括主键索引）
- 索引键最大长度：64字节
- 支持的索引类型：哈希（主键）、有序数组、B-Tree、T-Tree