# 全局向量压缩配置系统表设计与实现计划

## 1. 系统表设计

### 1.1 系统表结构

**表名**：`__remdb_system_config`

**字段定义**：
| 字段名 | 数据类型 | 约束 | 描述 |
|--------|----------|------|------|
| config_key | STRING(64) | PRIMARY KEY | 配置项键名 |
| config_value | STRING(256) | NOT NULL | 配置项值（JSON格式） |
| description | STRING(128) | NOT NULL | 配置项描述 |
| updated_at | TIMESTAMP | NOT NULL | 最后更新时间 |
| created_at | TIMESTAMP | NOT NULL | 创建时间 |

### 1.2 向量压缩相关配置项

| 配置项键名 | 默认值 | 描述 |
|------------|--------|------|
| `vector_compression_enabled` | `false` | 全局向量压缩开关 |
| `vector_compression_scheme` | `"none"` | 压缩方案：`"none"`=不压缩, `"float16"`=float16, `"zstd"`=ZSTD |
| `vector_compression_params` | `{}` | 压缩参数（JSON格式） |
| `vector_compression_level` | `3` | 压缩级别（1-9），级别越高压缩率越高但速度越慢 |

### 1.3 系统表初始化

- **初始化时机**：数据库启动时自动创建
- **默认配置**：
  - 全局向量压缩禁用（`vector_compression_enabled=false`）
  - 压缩方案为不压缩（`vector_compression_scheme="none"`）
  - 新创建的向量字段默认继承全局配置

## 2. 实现步骤

### 2.1 系统表创建与管理

#### 2.1.1 系统表定义与初始化
- **文件**：`src/system_tables.rs`（新增）
- **内容**：
  - 系统表结构定义
  - 系统表初始化逻辑
  - 系统表读写API
  - 配置缓存机制

#### 2.1.2 数据库启动时创建系统表
- **文件**：`src/lib.rs`
- **修改**：在`RemDb::init()`方法中添加系统表创建逻辑

#### 2.1.3 SQL接口支持
- **文件**：`src/sql/query_executor.rs`
- **修改**：添加系统表的SELECT、INSERT、UPDATE、DELETE支持

### 2.2 向量压缩配置集成

#### 2.2.1 向量元数据扩展
- **文件**：`src/types.rs`
- **修改**：
  - 扩展`VectorMetadata`结构体，添加压缩相关字段
  - 新增`CompressionScheme`枚举：`None`、`Float16`、`Zstd`
  - 新增`CompressionConfig`结构体，存储压缩配置

#### 2.2.2 全局配置读取与缓存
- **文件**：`src/system_tables.rs`
- **内容**：
  - 实现全局配置的读取逻辑
  - 实现配置缓存机制，减少系统表访问次数
  - 配置变更时自动更新缓存

#### 2.2.3 向量字段大小计算修改
- **文件**：`src/lib.rs`（字段大小计算逻辑）
- **修改**：
  - 根据全局配置动态计算向量字段大小
  - 不压缩：维度×4字节（float32）
  - Float16：维度×2字节（float16）
  - Zstd：动态大小（预留空间或按需分配）

### 2.3 向量处理逻辑修改

#### 2.3.1 向量写入逻辑修改
- **文件**：`src/table.rs`（`insert`和相关方法）
- **修改**：
  - 在写入向量前，读取全局压缩配置
  - 根据压缩方案选择合适的压缩方法
  - 支持不压缩方案（直接写入原始数据）

#### 2.3.2 向量读取逻辑修改
- **文件**：`src/table.rs`（`get_field_value`和相关方法）
- **修改**：
  - 在读取向量前，读取全局压缩配置
  - 根据压缩方案选择合适的解压缩方法
  - 支持不压缩方案（直接读取原始数据）

#### 2.3.3 向量压缩/解压缩实现
- **文件**：`src/compression.rs`（新增）
- **内容**：
  - `compress_vector`函数：根据压缩方案压缩向量
  - `decompress_vector`函数：根据压缩方案解压缩向量
  - 支持不压缩、float16等多种方案

#### 2.3.4 向量比较逻辑适配
- **文件**：`src/sql/query_executor.rs`（向量比较操作）
- **修改**：
  - 确保向量比较时使用统一的float32格式
  - 解压缩后再进行距离计算

### 2.4 系统表配置的实时生效

#### 2.4.1 配置变更监听
- **文件**：`src/system_tables.rs`
- **内容**：
  - 当系统表配置变更时，触发缓存更新
  - 确保正在处理的事务不受影响
  - 新事务使用最新配置

#### 2.4.2 配置加载时机
- **配置初始化**：数据库启动时加载到缓存
- **配置刷新**：系统表更新时实时刷新缓存
- **事务使用**：每个事务开始时读取最新缓存

## 3. 关键实现细节

### 3.1 压缩方案枚举定义
```rust
/// 压缩方案枚举
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompressionScheme {
    /// 不压缩，直接存储原始float32
    None = 0,
    /// Float16压缩，压缩率50%
    Float16 = 1,
    /// ZSTD压缩，压缩率60-70%
    Zstd = 2,
}
```

### 3.2 系统表初始化SQL
```sql
CREATE TABLE __remdb_system_config (
    config_key STRING(64) NOT NULL,
    config_value STRING(256) NOT NULL,
    description STRING(128) NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (config_key)
);

-- 插入向量压缩默认配置
INSERT INTO __remdb_system_config (config_key, config_value, description) VALUES
('vector_compression_enabled', 'false', '全局向量压缩开关'),
('vector_compression_scheme', '"none"', '向量压缩方案：none=不压缩, float16=float16, zstd=ZSTD'),
('vector_compression_params', '{}', '向量压缩参数（JSON格式）'),
('vector_compression_level', '3', '压缩级别（1-9）');
```

### 3.3 全局配置读取逻辑
```rust
/// 获取全局向量压缩配置
pub fn get_vector_compression_config() -> CompressionConfig {
    // 首先尝试从缓存读取
    if let Some(config) = COMPRESSION_CONFIG_CACHE.get() {
        return config.clone();
    }
    
    // 从系统表读取
    let config = unsafe {
        let db = crate::get_global_db().unwrap();
        let result = db.sql_query("SELECT config_key, config_value FROM __remdb_system_config WHERE config_key LIKE 'vector_compression_%'").unwrap();
        
        // 解析配置
        let mut enabled = false;
        let mut scheme = CompressionScheme::None;
        let mut params = serde_json::from_str("{}").unwrap();
        let mut level = 3;
        
        for row in result {
            let key = row.get_string(0).unwrap();
            let value = row.get_string(1).unwrap();
            
            match key.as_str() {
                "vector_compression_enabled" => {
                    enabled = value.parse::<bool>().unwrap();
                }
                "vector_compression_scheme" => {
                    let scheme_str = serde_json::from_str::<String>(&value).unwrap();
                    scheme = match scheme_str.as_str() {
                        "none" => CompressionScheme::None,
                        "float16" => CompressionScheme::Float16,
                        "zstd" => CompressionScheme::Zstd,
                        _ => CompressionScheme::None,
                    };
                }
                "vector_compression_params" => {
                    params = serde_json::from_str(&value).unwrap();
                }
                "vector_compression_level" => {
                    level = value.parse::<u8>().unwrap();
                }
                _ => {}
            }
        }
        
        CompressionConfig {
            enabled,
            scheme,
            params,
            level,
        }
    };
    
    // 更新缓存
    COMPRESSION_CONFIG_CACHE.set(config.clone());
    
    config
}
```

### 3.4 向量写入时的压缩逻辑
```rust
/// 写入向量字段时的压缩逻辑
fn write_vector_field(
    field_ptr: *mut u8,
    vector: *const f32,
    dimension: usize
) {
    // 获取全局压缩配置
    let config = get_vector_compression_config();
    
    unsafe {
        match config.scheme {
            CompressionScheme::None => {
                // 不压缩：直接拷贝原始float32数据
                let vector_size = dimension * core::mem::size_of::<f32>();
                memcpy(field_ptr, vector as *const u8, vector_size);
            },
            CompressionScheme::Float16 => {
                // Float16压缩
                for i in 0..dimension {
                    let f32_val = *vector.add(i);
                    let f16_val = f32_to_f16(f32_val);
                    let f16_ptr = field_ptr.add(i * 2) as *mut u16;
                    *f16_ptr = f16_val;
                }
            },
            CompressionScheme::Zstd => {
                // ZSTD压缩（预留空间实现）
                let vector_size = dimension * core::mem::size_of::<f32>();
                let compressed_size = zstd_compress(
                    vector as *const u8,
                    vector_size,
                    field_ptr.add(4), // 前4字节存储压缩大小
                    field_ptr.add(4) as usize
                );
                // 存储压缩大小
                let size_ptr = field_ptr as *mut u32;
                *size_ptr = compressed_size as u32;
            },
        }
    }
}
```

### 3.5 向量读取时的解压缩逻辑
```rust
/// 读取向量字段时的解压缩逻辑
fn read_vector_field(
    field_ptr: *const u8,
    dimension: usize,
    output: *mut f32
) {
    // 获取全局压缩配置
    let config = get_vector_compression_config();
    
    unsafe {
        match config.scheme {
            CompressionScheme::None => {
                // 不压缩：直接拷贝原始float32数据
                let vector_size = dimension * core::mem::size_of::<f32>();
                memcpy(output as *mut u8, field_ptr, vector_size);
            },
            CompressionScheme::Float16 => {
                // Float16解压缩
                for i in 0..dimension {
                    let f16_ptr = field_ptr.add(i * 2) as *const u16;
                    let f16_val = *f16_ptr;
                    let f32_val = f16_to_f32(f16_val);
                    *output.add(i) = f32_val;
                }
            },
            CompressionScheme::Zstd => {
                // ZSTD解压缩
                let size_ptr = field_ptr as *const u32;
                let compressed_size = *size_ptr as usize;
                let vector_size = dimension * core::mem::size_of::<f32>();
                zstd_decompress(
                    field_ptr.add(4),
                    compressed_size,
                    output as *mut u8,
                    vector_size
                );
            },
        }
    }
}
```

## 4. 测试计划

### 4.1 系统表测试
- **测试内容**：
  - 系统表的自动创建
  - 系统表的默认配置初始化
  - 系统表的SQL操作
  - 配置缓存的正确性

### 4.2 全局配置测试
- **测试内容**：
  - 全局配置的读取
  - 配置变更的实时生效
  - 配置缓存的刷新机制

### 4.3 向量压缩功能测试
- **测试内容**：
  - 不压缩方案的正确性
  - Float16压缩的正确性
  - 压缩后数据的完整性
  - 不同压缩方案的性能对比
  - 高维向量的压缩效果

### 4.4 集成测试
- **测试内容**：
  - 创建表时的向量字段大小计算
  - 插入向量数据时的压缩
  - 查询向量数据时的解压缩
  - 向量比较操作的正确性
  - 日志系统的适配

## 5. 预期效果

### 5.1 功能特性
- ✅ 支持全局向量压缩配置
- ✅ 支持不压缩方案（默认）
- ✅ 支持Float16压缩方案
- ✅ 支持通过SQL动态配置
- ✅ 配置变更实时生效
- ✅ 与现有架构无缝集成

### 5.2 性能特性
- ✅ 不压缩方案性能最优
- ✅ Float16压缩/解压缩速度快
- ✅ 配置缓存减少系统表访问开销
- ✅ 支持不同场景的压缩方案选择

### 5.3 扩展性
- ✅ 压缩方案枚举支持未来扩展
- ✅ 压缩参数字段支持复杂配置
- ✅ 系统表结构支持未来扩展

## 6. 代码结构变更

### 6.1 新增文件
- `src/system_tables.rs`：系统表管理
- `src/compression.rs`：向量压缩实现

### 6.2 修改文件
- `src/lib.rs`：系统表初始化和全局配置加载
- `src/types.rs`：向量元数据扩展
- `src/table.rs`：向量读写逻辑修改
- `src/sql/query_executor.rs`：系统表SQL支持和向量比较逻辑

通过以上设计和实现，我们将建立一个简洁、高效的全局向量压缩配置系统，支持多种压缩方案和动态配置，同时保持与现有架构的无缝集成。