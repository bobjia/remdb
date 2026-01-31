pub mod builder; mod hnsw; mod ivf; use crate::platform::{memcpy, memset}; use crate::types::{DataType, DistanceType, IndexType, RemDbError, Result, TableDef, VectorIndexType}; use core::ptr::NonNull;

/// B-Tree阶数（每个节点的最大键数量）
const BTREE_ORDER: usize = 4; // 阶数为4的B-Tree

/// T-Tree阶数（每个节点的最大键数量）
const TTREE_ORDER: usize = 3; // 阶数为3的T-Tree

/// 索引统计信息
pub struct IndexStats {
    /// 索引使用次数
    pub access_count: usize,
    /// 索引命中次数
    pub hit_count: usize,
    /// 索引大小（字节）
    pub size: usize,
    /// 索引项数量
    pub item_count: usize,
}

impl Default for IndexStats {
    fn default() -> Self {
        IndexStats {
            access_count: 0,
            hit_count: 0,
            size: 0,
            item_count: 0,
        }
    }
}

/// 主键哈希索引项
#[repr(C)]
pub struct PrimaryIndexItem {
    /// 下一个项的指针
    pub next: Option<NonNull<PrimaryIndexItem>>,
    /// 记录ID
    pub record_id: u16,
    /// 键大小
    pub key_size: u8,
    /// 键数据
    pub key_data: [u8; 128], // 最大键大小128字节，支持复合键
}

/// 辅助索引项
#[derive(Copy, Clone)]
#[repr(C)]
pub struct SecondaryIndexItem {
    /// 键大小
    pub key_size: u8,
    /// 记录ID
    pub record_id: u16,
    /// 键数据
    pub key_data: [u8; 128], // 最大键大小128字节，支持复合键
}

impl Default for SecondaryIndexItem {
    fn default() -> Self {
        SecondaryIndexItem {
            key_size: 0,
            record_id: 0,
            key_data: [0u8; 128],
        }
    }
}

/// B-Tree节点结构
#[repr(C)]
pub struct BTreeNode {
    /// 节点类型（内部节点或叶子节点）
    pub is_leaf: bool,
    /// 当前键数量
    pub key_count: u8,
    /// 键数据（每个键4字节）
    pub keys: [SecondaryIndexItem; BTREE_ORDER],
    /// 子节点指针（仅内部节点使用）
    pub children: [Option<NonNull<BTreeNode>>; BTREE_ORDER + 1],
}

/// B-Tree索引结构
pub struct BTreeIndex {
    /// 表定义
    pub def: alloc::sync::Arc<TableDef>,
    /// 根节点
    pub root: Option<NonNull<BTreeNode>>,
    /// 节点池
    pub nodes: NonNull<BTreeNode>,
    /// 空闲节点链表
    pub free_nodes: Option<NonNull<BTreeNode>>,
    /// 最大节点数量
    pub max_nodes: usize,
    /// 索引统计信息
    pub stats: IndexStats,
    /// 自旋锁
    pub lock: u32,
}

/// T-Tree节点结构
#[repr(C)]
pub struct TTreeNode {
    /// 当前键数量
    pub key_count: u8,
    /// 键数据（每个键4字节）
    pub keys: [SecondaryIndexItem; TTREE_ORDER],
    /// 左子节点
    pub left: Option<NonNull<TTreeNode>>,
    /// 中子节点（用于T-Tree的三元分支）
    pub middle: Option<NonNull<TTreeNode>>,
    /// 右子节点
    pub right: Option<NonNull<TTreeNode>>,
}

/// T-Tree索引结构
pub struct TTreeIndex {
    /// 表定义
    pub def: alloc::sync::Arc<TableDef>,
    /// 根节点
    pub root: Option<NonNull<TTreeNode>>,
    /// 节点池
    pub nodes: NonNull<TTreeNode>,
    /// 空闲节点链表
    pub free_nodes: Option<NonNull<TTreeNode>>,
    /// 最大节点数量
    pub max_nodes: usize,
    /// 索引统计信息
    pub stats: IndexStats,
    /// 自旋锁
    pub lock: u32,
}

/// 主键哈希索引
pub struct PrimaryIndex {
    /// 表定义
    def: alloc::sync::Arc<TableDef>,
    /// 哈希表数据
    hash_table: NonNull<Option<NonNull<PrimaryIndexItem>>>,
    /// 哈希表大小
    hash_table_size: usize,
    /// 索引项数据
    items: NonNull<PrimaryIndexItem>,
    /// 可用索引项指针
    free_items: Option<NonNull<PrimaryIndexItem>>,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: u32,
}

/// 复合键编码辅助函数
/// 将多个字段值编码为单一字节数组
unsafe fn encode_composite_key(
    record_ptr: *const u8,
    primary_key_fields: &[&crate::types::FieldDef],
) -> (Vec<u8>, usize) {
    let mut encoded_key = Vec::new();
    
    for field in primary_key_fields {
        let field_ptr = record_ptr.add(field.offset);
        match field.data_type {
            crate::types::DataType::UInt8 => {
                let value = core::ptr::read_unaligned(field_ptr as *const u8);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::UInt16 => {
                let value = core::ptr::read_unaligned(field_ptr as *const u16);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::UInt32 => {
                let value = core::ptr::read_unaligned(field_ptr as *const u32);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::UInt64 => {
                let value = core::ptr::read_unaligned(field_ptr as *const u64);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Int8 => {
                let value = core::ptr::read_unaligned(field_ptr as *const i8);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Int16 => {
                let value = core::ptr::read_unaligned(field_ptr as *const i16);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Int32 => {
                let value = core::ptr::read_unaligned(field_ptr as *const i32);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Int64 => {
                let value = core::ptr::read_unaligned(field_ptr as *const i64);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Float32 => {
                let value = core::ptr::read_unaligned(field_ptr as *const f32);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Float64 => {
                let value = core::ptr::read_unaligned(field_ptr as *const f64);
                encoded_key.extend_from_slice(&value.to_le_bytes());
            },
            crate::types::DataType::Bool => {
                let value = core::ptr::read_unaligned(field_ptr as *const bool);
                encoded_key.push(value as u8);
            },
            crate::types::DataType::Timestamp => {
                let value = core::ptr::read_unaligned(field_ptr as *const crate::types::db_timestamp);
                encoded_key.extend_from_slice(&value.value.to_le_bytes());
            },
            crate::types::DataType::String => {
                // 字符串类型：编码为长度前缀 + 内容
                let str_slice = core::slice::from_raw_parts(field_ptr, field.size);
                let str_len = str_slice.iter().position(|&c| c == 0).unwrap_or(field.size);
                encoded_key.push(str_len as u8);
                encoded_key.extend_from_slice(&str_slice[0..str_len]);
            },
            _ => {
                // 其他类型暂不支持作为主键
            }
        }
    }
    
    let len = encoded_key.len();
    (encoded_key, len)
}

impl PrimaryIndex {
    /// 创建新的主键索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        hash_table_start: *mut Option<NonNull<PrimaryIndexItem>>,
        items_start: *mut PrimaryIndexItem,
        hash_table_size: usize,
        max_items: usize,
    ) -> Self {
        // 初始化哈希表
        let hash_table = NonNull::new_unchecked(hash_table_start);
        for i in 0..hash_table_size {
            let slot_ptr = hash_table.as_ptr().add(i);
            *slot_ptr = None;
        }

        // 初始化索引项链表
        let items = NonNull::new_unchecked(items_start);
        let mut free_items = None;
        for i in (0..max_items).rev() {
            let item_ptr = items.as_ptr().add(i);
            (*item_ptr).next = free_items;
            (*item_ptr).record_id = 0;
            (*item_ptr).key_size = 0;
            memset((*item_ptr).key_data.as_mut_ptr(), 0, 128);
            free_items = Some(NonNull::new_unchecked(item_ptr));
        }

        PrimaryIndex {
            def,
            hash_table,
            hash_table_size,
            items,
            free_items,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: hash_table_size * core::mem::size_of::<Option<NonNull<PrimaryIndexItem>>>()
                    + max_items * core::mem::size_of::<PrimaryIndexItem>(),
                item_count: 0,
            },
            lock: 0,
        }
    }

    /// 计算主键索引所需的内存大小
    pub const fn calculate_memory_size(
        _def: &TableDef,
        hash_table_size: usize,
        max_items: usize,
    ) -> usize {
        let hash_table_size_bytes =
            hash_table_size * core::mem::size_of::<Option<NonNull<PrimaryIndexItem>>>();
        let items_size_bytes = max_items * core::mem::size_of::<PrimaryIndexItem>();

        hash_table_size_bytes + items_size_bytes
    }

    /// 计算哈希值
    fn hash_key(&self, key: *const u8, key_size: usize) -> usize {
        // 优化的哈希算法：使用MurmurHash3的简化版本，更适合小数据
        let mut hash = 0u64;
        let seed = 0x5bd1e995u64;

        for i in 0..key_size {
            let byte = unsafe { *key.add(i) };
            hash ^= byte as u64;
            hash = hash.wrapping_mul(seed);
            hash ^= hash >> 47;
        }

        (hash as usize) % self.hash_table_size
    }

    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 检查键大小
        if key_size > 128 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }

        // 查找空闲索引项
        let mut item = match self.free_items {
            Some(item_ptr) => {
                // 从空闲列表获取项
                let next = (*item_ptr.as_ptr()).next;
                self.free_items = next;
                item_ptr
            }
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::OutOfMemory);
            }
        };

        // 设置索引�?
        let item_mut = item.as_mut();
        item_mut.record_id = record_id;
        item_mut.key_size = key_size as u8;
        memcpy(item_mut.key_data.as_mut_ptr(), key, key_size);

        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);

        // 插入到哈希表槽位的头�?
        item_mut.next = *slot_ptr;
        *slot_ptr = Some(item);

        // 更新统计信息
        self.stats.item_count += 1;

        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }

    /// 插入复合主键索引�?
    pub unsafe fn insert_composite(&mut self, record_ptr: *const u8, record_id: u16) -> Result<()> {
        // 获取主键字段列表
        let primary_key_fields: Vec<&crate::types::FieldDef> = self.def.primary_key
            .iter()
            .map(|&idx| &self.def.fields[idx])
            .collect();
        
        // 编码复合键
        let (encoded_key, key_size) = encode_composite_key(record_ptr, &primary_key_fields);
        
        // 调用普通插入方法
        self.insert(encoded_key.as_ptr(), key_size, record_id)
    }

    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 计算哈希�?
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);

        // 遍历链表查找
        let mut current = *slot_ptr;
        while let Some(item) = current {
            let item_ref = item.as_ref();

            // 比较键
            if item_ref.key_size == key_size as u8 {
                let mut match_found = true;
                for i in 0..key_size {
                    if item_ref.key_data[i] != *key.add(i) {
                        match_found = false;
                        break;
                    }
                }

                if match_found {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(item_ref.record_id);
                }
            }

            current = item_ref.next;
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 根据记录指针查找复合主键对应的记录ID
    pub unsafe fn find_composite(&mut self, record_ptr: *const u8) -> Result<u16> {
        // 获取主键字段列表
        let primary_key_fields: Vec<&crate::types::FieldDef> = self.def.primary_key
            .iter()
            .map(|&idx| &self.def.fields[idx])
            .collect();
        
        // 编码复合�?
        let (encoded_key, key_size) = encode_composite_key(record_ptr, &primary_key_fields);
        
        // 调用普通查找方�?
        self.find(encoded_key.as_ptr(), key_size)
    }

    /// 删除索引�?
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 计算哈希�?
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);

        // 遍历链表查找并删�?
        let mut current = *slot_ptr;
        let mut prev: Option<NonNull<PrimaryIndexItem>> = None;

        while let Some(mut item) = current {
            let item_ref = item.as_ref();

            // 比较�?
            if item_ref.key_size == key_size as u8 {
                let mut match_found = true;
                for i in 0..key_size {
                    if item_ref.key_data[i] != *key.add(i) {
                        match_found = false;
                        break;
                    }
                }

                if match_found {
                    // 从链表中移除
                    if let Some(mut prev_item) = prev {
                        prev_item.as_mut().next = item_ref.next;
                    } else {
                        *slot_ptr = item_ref.next;
                    }

                    // 归还到空闲列�?
                    let item_mut = item.as_mut();
                    item_mut.next = self.free_items;
                    self.free_items = Some(item);

                    // 更新统计信息
                    self.stats.item_count -= 1;

                    crate::platform::spin_unlock(&mut self.lock);
                    return Ok(());
                }
            }

            prev = Some(item);
            current = item_ref.next;
        }

        crate::platform::spin_unlock(&mut self.lock);
        Err(RemDbError::RecordNotFound)
    }

    /// 根据记录指针删除复合主键索引�?
    pub unsafe fn delete_composite(&mut self, record_ptr: *const u8) -> Result<()> {
        // 获取主键字段列表
        let primary_key_fields: Vec<&crate::types::FieldDef> = self.def.primary_key
            .iter()
            .map(|&idx| &self.def.fields[idx])
            .collect();
        
        // 编码复合�?
        let (encoded_key, key_size) = encode_composite_key(record_ptr, &primary_key_fields);
        
        // 调用普通删除方�?
        self.delete(encoded_key.as_ptr(), key_size)
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats.access_count = 0;
        self.stats.hit_count = 0;
    }
}

/// 辅助索引枚举（用于封装不同类型的辅助索引�?
/// 向量索引�?
#[derive(Copy, Clone)]
struct VectorIndexItem {
    /// 向量在vectors数组中的起始偏移�?
    vector_offset: usize,
    /// 记录ID
    record_id: u16,
}

/// 向量索引实现类型
enum VectorIndexImpl {
    /// 线性搜索索引（默认�?   
    LinearSearch,
    /// HNSW索引    
    HNSW(Option<hnsw::HNSWIndex>),
    /// IVF_FLAT索引    
    IVFFlat(Option<ivf::IVFIndex>),
}

/// 向量索引
pub struct VectorIndex {
    /// 表定�?
    def: alloc::sync::Arc<TableDef>,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋�?
    lock: u32,
    /// 距离度量类型
    distance_type: DistanceType,
    /// 向量维度
    dimension: u16,
    /// 向量索引类型
    vector_index_type: VectorIndexType,
    /// 向量索引项列�?
    items: *mut VectorIndexItem,
    /// 向量数据存储
    vectors: *mut f32,
    /// 最大项数量
    max_items: usize,
    /// 当前项数�?
    item_count: usize,
    /// 当前向量数量
    vector_count: usize,
    /// 索引实现
    index_impl: VectorIndexImpl,
}

impl VectorIndex {
    /// 创建新的向量索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        memory_start: *mut u8,
        max_items: usize,
    ) -> Result<Self> {
        // 获取向量维度和距离类�?
        let mut dimension = 0;
        let mut distance_type = DistanceType::L2;
        let mut vector_index_type = VectorIndexType::HNSW;

        // 验证是否有有效的辅助索引字段
        let secondary_index = def.secondary_index.as_ref().ok_or(RemDbError::TypeMismatch)?;
        if secondary_index.len() != 1 {
            return Err(RemDbError::TypeMismatch);
        }
        let secondary_index = secondary_index[0];
        if secondary_index >= def.fields.len() {
            return Err(RemDbError::TypeMismatch);
        }

        // 获取被索引的字段
        let field = &def.fields[secondary_index];

        // 验证该字段是向量类型且有有效metadata
        if field.data_type != DataType::Vector {
            #[cfg(feature = "std")]
            eprintln!(
                "TypeMismatch: field.data_type != DataType::Vector, actual: {:?}",
                field.data_type
            );
            return Err(RemDbError::TypeMismatch);
        }

        let vector_meta = match &field.vector_metadata {
            Some(meta) => {
                dimension = meta.dimension;
                distance_type = meta.distance_type;
                vector_index_type = meta.index_type;
                meta
            },
            None => {
                #[cfg(feature = "std")]
                eprintln!("TypeMismatch: field.vector_metadata is None");
                return Err(RemDbError::TypeMismatch);
            }
        };

        // 验证向量维度有效
        if dimension == 0 || dimension > 1024 {
            #[cfg(feature = "std")]
            eprintln!("TypeMismatch: invalid dimension: {}", dimension);
            return Err(RemDbError::TypeMismatch);
        }

        // 计算内存布局
        let items_size = max_items * core::mem::size_of::<VectorIndexItem>();
        let vectors_size = max_items * dimension as usize * core::mem::size_of::<f32>();
        
        // 初始化索引项数组
        let items = memory_start as *mut VectorIndexItem;
        // 使用安全的方式初始化索引项数�?
        for i in 0..max_items {
            let item_ptr = unsafe { items.add(i) };
            *item_ptr = VectorIndexItem {
                vector_offset: 0,
                record_id: 0,
            };
        }

        // 初始化向量数据数�?
        let vectors = (memory_start.add(items_size)) as *mut f32;
        // 使用安全的方式初始化向量数据数组
        for i in 0..(max_items * dimension as usize) {
            let vec_ptr = unsafe { vectors.add(i) };
            *vec_ptr = 0.0;
        }
        
        // 初始化索引实�?
        let index_impl = match vector_index_type {
            VectorIndexType::HNSW | VectorIndexType::HNSW_SQ | VectorIndexType::HNSW_BQ => {
                // 创建HNSW索引
                let hnsw_memory = (memory_start.add(items_size + vectors_size)) as *mut u8;
                let hnsw_index = hnsw::HNSWIndex::new(*vector_meta, vectors, hnsw_memory, max_items)?;
                VectorIndexImpl::HNSW(Some(hnsw_index))
            },
            VectorIndexType::IVF | VectorIndexType::IVF_PQ => {
                // 创建IVF_FLAT索引
                let ivf_index = ivf::IVFIndex::new(*vector_meta, vectors, vector_meta.ivf_nlist, vector_meta.ivf_nprobe)?;
                VectorIndexImpl::IVFFlat(Some(ivf_index))
            },
            _ => {
                // 默认使用线性搜�?
                VectorIndexImpl::LinearSearch
            }
        };

        Ok(VectorIndex {
            def,
            stats: IndexStats::default(),
            lock: 0,
            distance_type,
            dimension,
            vector_index_type,
            items,
            vectors,
            max_items,
            item_count: 0,
            vector_count: 0,
            index_impl,
        })
    }

    /// 计算向量索引所需的内存大�?
    pub fn calculate_memory_size(def: &TableDef, max_items: usize) -> usize {
        // 获取向量维度
        let dimension = match def.secondary_index.as_ref() {
            Some(secondary_index) if !secondary_index.is_empty() => {
                let secondary_index = secondary_index[0];
                if secondary_index < def.fields.len() {
                    let field = &def.fields[secondary_index];
                    if field.data_type == DataType::Vector {
                        if let Some(vector_meta) = &field.vector_metadata {
                            vector_meta.dimension
                        } else {
                            // 向量字段必须有元数据，否则返回合理的默认值避免内存分配失�?
                            128 // 默认维度，避免返�?
                        }
                    } else {
                        128 // 非向量字段作为向量索引使用默认维�?
                    }
                } else {
                    128 // 默认维度
                }
            }
            _ => 128, // 默认维度
        };

        // 确保维度至少�?，避免除�?或分�?内存
        let dimension = core::cmp::max(dimension, 1);

        // 计算内存大小：索引项 + 向量数据
        let items_size = max_items * core::mem::size_of::<VectorIndexItem>();
        let vectors_size = max_items * dimension as usize * core::mem::size_of::<f32>();

        // 确保返回的内存大小至少为1，避免alloc分配0内存
        core::cmp::max(items_size + vectors_size, 1)
    }

    /// 计算两个向量之间的距�?
    unsafe fn calculate_distance(&self, vec1: *const f32, vec2: *const f32) -> f32 {
        match self.distance_type {
            DistanceType::L2 => {
                // L2距离（欧几里得距离）
                let mut sum = 0.0;
                for i in 0..self.dimension {
                    let diff = *vec1.add(i as usize) - *vec2.add(i as usize);
                    sum += diff * diff;
                }
                sum.sqrt()
            }
            DistanceType::InnerProduct => {
                // 内积
                let mut sum = 0.0;
                for i in 0..self.dimension {
                    sum += *vec1.add(i as usize) * *vec2.add(i as usize);
                }
                sum
            }
            DistanceType::Cosine => {
                // 余弦相似�?
                let mut dot = 0.0;
                let mut norm1 = 0.0;
                let mut norm2 = 0.0;
                
                for i in 0..self.dimension {
                    let v1 = *vec1.add(i as usize);
                    let v2 = *vec2.add(i as usize);
                    dot += v1 * v2;
                    norm1 += v1 * v1;
                    norm2 += v2 * v2;
                }
                
                let norm1 = norm1.sqrt();
                let norm2 = norm2.sqrt();
                
                if norm1 == 0.0 || norm2 == 0.0 {
                    0.0
                } else {
                    dot / (norm1 * norm2)
                }
            }
        }
    }
    
    /// 保存索引到磁�?
    #[cfg(feature = "std")]
    pub fn save(&self, file_path: &str) -> Result<()> {
        use std::fs::File;
        use std::io::Write;
        
        // 打开文件用于写入
        let mut file = File::create(file_path)
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存索引元数�?
        // 写入索引类型
        file.write_all(&[self.vector_index_type as u8])
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入距离类型
        file.write_all(&[self.distance_type as u8])
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入向量维度
        file.write_all(&self.dimension.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入最大项数量
        file.write_all(&self.max_items.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入当前项数�?
        file.write_all(&self.item_count.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入当前向量数量
        file.write_all(&self.vector_count.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存索引�?
        unsafe {
            for i in 0..self.item_count {
                let item = self.items.add(i).read();
                // 写入向量偏移�?
                file.write_all(&item.vector_offset.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
                // 写入记录ID
                file.write_all(&item.record_id.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
            }
        }
        
        // 保存向量数据
        unsafe {
            let vector_size = self.vector_count * self.dimension as usize;
            let vec_slice = core::slice::from_raw_parts(self.vectors, vector_size);
            let vec_bytes = unsafe {
                core::slice::from_raw_parts(
                    vec_slice.as_ptr() as *const u8,
                    vector_size * core::mem::size_of::<f32>()
                )
            };
            file.write_all(vec_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
        }
        
        // 保存具体索引实现数据
        match &self.index_impl {
            VectorIndexImpl::HNSW(Some(hnsw_index)) => {
                hnsw_index.save(&mut file)?;
            }
            VectorIndexImpl::IVFFlat(Some(ivf_index)) => {
                ivf_index.save(&mut file)?;
            }
            _ => {
                // 线性搜索不需要保存额外数�?
            }
        }
        
        Ok(())
    }
    
    /// 从磁盘加载索�?
    #[cfg(feature = "std")]
    pub unsafe fn load(
        def: alloc::sync::Arc<TableDef>,
        file_path: &str,
        memory_start: *mut u8,
    ) -> Result<Self> {
        use std::fs::File;
        use std::io::Read;
        
        // 打开文件用于读取
        let mut file = File::open(file_path)
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 读取索引元数�?
        // 读取索引类型
        let mut index_type_byte = [0u8; 1];
        file.read_exact(&mut index_type_byte)
            .map_err(|_| RemDbError::FileIoError)?;
        let vector_index_type = match index_type_byte[0] {
            0 => VectorIndexType::HNSW,
            1 => VectorIndexType::HNSW_SQ,
            2 => VectorIndexType::HNSW_BQ,
            3 => VectorIndexType::IVF,
            4 => VectorIndexType::IVF_PQ,
            _ => VectorIndexType::HNSW, // 默认�?
        };
        
        // 读取距离类型
        let mut distance_type_byte = [0u8; 1];
        file.read_exact(&mut distance_type_byte)
            .map_err(|_| RemDbError::FileIoError)?;
        let distance_type = match distance_type_byte[0] {
                0 => DistanceType::L2,
                1 => DistanceType::InnerProduct,
                2 => DistanceType::Cosine,
                _ => DistanceType::L2, // 默认�?
            };
        
        // 读取向量维度
        let mut dimension_bytes = [0u8; 2];
        file.read_exact(&mut dimension_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let dimension = u16::from_le_bytes(dimension_bytes);
        
        // 读取最大项数量
        let mut max_items_bytes = [0u8; 8];
        file.read_exact(&mut max_items_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let max_items = usize::from_le_bytes(max_items_bytes);
        
        // 读取当前项数�?
        let mut item_count_bytes = [0u8; 8];
        file.read_exact(&mut item_count_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let item_count = usize::from_le_bytes(item_count_bytes);
        
        // 读取当前向量数量
        let mut vector_count_bytes = [0u8; 8];
        file.read_exact(&mut vector_count_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let vector_count = usize::from_le_bytes(vector_count_bytes);
        
        // 计算内存布局
        let items_size = max_items * core::mem::size_of::<VectorIndexItem>();
        let vectors_size = max_items * dimension as usize * core::mem::size_of::<f32>();
        
        // 初始化索引项数组
        let items = memory_start as *mut VectorIndexItem;
        
        // 读取索引�?
        for i in 0..item_count {
            let mut item = VectorIndexItem {
                vector_offset: 0,
                record_id: 0,
            };
            
            // 读取向量偏移�?
            let mut offset_bytes = [0u8; 8];
            file.read_exact(&mut offset_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            item.vector_offset = usize::from_le_bytes(offset_bytes);
            
            // 读取记录ID
            let mut record_id_bytes = [0u8; 2];
            file.read_exact(&mut record_id_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            item.record_id = u16::from_le_bytes(record_id_bytes);
            
            // 保存到内�?
            *items.add(i) = item;
        }
        
        // 初始化向量数据数�?
        let vectors = (memory_start.add(items_size)) as *mut f32;
        
        // 读取向量数据
        let vector_size = vector_count * dimension as usize;
        let vec_slice = core::slice::from_raw_parts_mut(vectors, vector_size);
        let vec_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                vec_slice.as_mut_ptr() as *mut u8,
                vector_size * core::mem::size_of::<f32>()
            )
        };
        file.read_exact(vec_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 加载具体索引实现
        let index_impl = match vector_index_type {
            VectorIndexType::HNSW | VectorIndexType::HNSW_SQ | VectorIndexType::HNSW_BQ => {
                // 加载HNSW索引
                let hnsw_memory = (memory_start.add(items_size + vectors_size)) as *mut u8;
                let vector_meta = crate::types::VectorMetadata {
                    dimension,
                    distance_type,
                    index_type: vector_index_type,
                    compression_enabled: false,
                    compression_scheme: 0,
                    compression_level: 3,
                    hnsw_m: 16, // 默认值，实际值会从文件加�?
                    hnsw_ef_construction: 200, // 默认值，实际值会从文件加�?
                    hnsw_ef_search: 100, // 默认值，实际值会从文件加�?
                    ivf_nlist: 100, // 默认值，实际值会从文件加�?
                    ivf_nprobe: 10, // 默认值，实际值会从文件加�?
                };
                let hnsw_index = hnsw::HNSWIndex::load(
                    vector_meta,
                    vectors,
                    hnsw_memory,
                    max_items,
                    &mut file
                )
                .map_err(|_| RemDbError::FileIoError)?;
                VectorIndexImpl::HNSW(Some(hnsw_index))
            }
            VectorIndexType::IVF | VectorIndexType::IVF_PQ => {
                // 加载IVF索引
                let vector_meta = crate::types::VectorMetadata {
                    dimension,
                    distance_type,
                    index_type: vector_index_type,
                    compression_enabled: false,
                    compression_scheme: 0,
                    compression_level: 3,
                    hnsw_m: 16, // 默认�?
                    hnsw_ef_construction: 200, // 默认�?
                    hnsw_ef_search: 100, // 默认�?
                    ivf_nlist: 100, // 默认值，实际值会从文件加�?
                    ivf_nprobe: 10, // 默认值，实际值会从文件加�?
                };
                let ivf_index = ivf::IVFIndex::load(
                    vector_meta,
                    vectors,
                    &mut file
                )
                .map_err(|_| RemDbError::FileIoError)?;
                VectorIndexImpl::IVFFlat(Some(ivf_index))
            }
            _ => VectorIndexImpl::LinearSearch,
        };
        
        Ok(VectorIndex {
            def,
            stats: IndexStats::default(),
            lock: 0,
            distance_type,
            dimension,
            vector_index_type,
            items,
            vectors,
            max_items,
            item_count,
            vector_count,
            index_impl,
        })
    }
    
    /// 计算两个向量之间的距离（旧实现，保留用于兼容性）
    unsafe fn calculate_distance_old(&self, vec1: *const f32, vec2: *const f32) -> f32 {
        match self.distance_type {
            DistanceType::L2 => {
                // L2距离（欧几里得距离）
                let mut sum = 0.0;
                for i in 0..self.dimension {
                    let diff = *vec1.add(i as usize) - *vec2.add(i as usize);
                    sum += diff * diff;
                }
                sum.sqrt()
            }
            DistanceType::InnerProduct => {
                // 内积
                let mut sum = 0.0;
                for i in 0..self.dimension {
                    sum += *vec1.add(i as usize) * *vec2.add(i as usize);
                }
                -sum // 返回负数，因为内积越大相似度越高
            }
            DistanceType::Cosine => {
                // 余弦相似�?
                let mut dot = 0.0;
                let mut norm1 = 0.0;
                let mut norm2 = 0.0;

                for i in 0..self.dimension {
                    let v1 = *vec1.add(i as usize);
                    let v2 = *vec2.add(i as usize);
                    dot += v1 * v2;
                    norm1 += v1 * v1;
                    norm2 += v2 * v2;
                }

                let norm1 = norm1.sqrt();
                let norm2 = norm2.sqrt();

                if norm1 == 0.0 || norm2 == 0.0 {
                    -1.0 // 相似度最�?
                } else {
                    -(dot / (norm1 * norm2)) // 返回负数，因为余弦相似度越大相似度越�?
                }
            }
        }
    }

    /// 插入向量索引�?
    pub unsafe fn insert(
        &mut self,
        key: *const u8,
        _key_size: usize,
        record_id: u16,
    ) -> Result<()> {
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 检查是否有足够的空�?
        if self.item_count >= self.max_items {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::OutOfMemory);
        }

        // 复制向量数据到预分配的存�?
        let vec_ptr = key as *const f32;
        let vec_len = self.dimension as usize;
        let start_offset = self.vector_count * vec_len;

        // 复制向量数据
        for i in 0..vec_len {
            *self.vectors.add(start_offset + i) = *vec_ptr.add(i);
        }

        // 创建索引�?
        let item_ptr = self.items.add(self.item_count);
        *item_ptr = VectorIndexItem {
            vector_offset: start_offset,
            record_id,
        };

        // 更新计数（确保item_count和vector_count始终同步�?
        self.item_count += 1;
        self.vector_count = self.item_count;

        // 更新统计信息
        self.stats.item_count += 1;
        self.stats.size +=
            core::mem::size_of::<VectorIndexItem>() + vec_len * core::mem::size_of::<f32>();
        
        // 根据索引类型插入到相应的索引实现�?
        match &mut self.index_impl {
            VectorIndexImpl::HNSW(Some(hnsw_index)) => {
                hnsw_index.insert(start_offset, record_id)?;
            },
            VectorIndexImpl::IVFFlat(Some(ivf_index)) => {
                ivf_index.insert(start_offset, record_id)?;
            },
            _ => {
                // 线性搜索不需要额外操�?
            }
        }

        crate::platform::spin_unlock(&mut self.lock);

        Ok(())
    }

    /// 根据向量查找记录ID（支持多种索引类型）
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 解析查询向量
        let query_vec: *const f32;
        let mut query_vec_buf: [f32; 1024];
        let vec_len = self.dimension as usize;

        // 检查key是否是指向字符串的指针（向量字面量）
        if key_size > 4 && *key == b'[' {
            // 解析向量字面�?[x1, x2, ..., xn]
            let vec_str = core::str::from_utf8(core::slice::from_raw_parts(key, key_size))
                .map_err(|_| RemDbError::TypeMismatch)?;

            // 移除首尾的方括号
            let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');

            // 分割逗号，得到每个元素的字符�?
            let elements: Vec<&str> = vec_str.split(',').map(|s| s.trim()).collect();

            // 检查维度是否匹�?
            if elements.len() != vec_len {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TypeMismatch);
            }

            // 解析每个元素为f32
            query_vec_buf = [0.0; 1024];
            for (i, elem) in elements.iter().enumerate() {
                query_vec_buf[i] = elem.parse::<f32>().map_err(|_| RemDbError::TypeMismatch)?;
            }

            query_vec = query_vec_buf.as_ptr();
        } else {
            // 直接使用key作为f32指针
            query_vec = key as *const f32;
        }
        
        // 根据索引类型使用相应的搜索方�?
        let result = match &self.index_impl {
            VectorIndexImpl::HNSW(Some(hnsw_index)) => {
                // 使用HNSW搜索
                let results = hnsw_index.search(query_vec, 1)?;
                if let Some((_, record_id)) = results.first() {
                    Ok(*record_id)
                } else {
                    Err(RemDbError::RecordNotFound)
                }
            },
            VectorIndexImpl::IVFFlat(Some(ivf_index)) => {
                // 使用IVF_FLAT搜索
                let results = ivf_index.search(query_vec, 1)?;
                if let Some((_, record_id)) = results.first() {
                    Ok(*record_id)
                } else {
                    Err(RemDbError::RecordNotFound)
                }
            },
            _ => {
                // 线性搜索查找最相似的向�?
                let mut min_distance = f32::MAX;
                let mut best_record_id = 0;
                let mut found = false;

                for i in 0..self.item_count {
                    let item_ptr = self.items.add(i);
                    let vec_ptr = self.vectors.add((*item_ptr).vector_offset);
                    let distance = self.calculate_distance(query_vec, vec_ptr);
                    if distance < min_distance {
                        min_distance = distance;
                        best_record_id = (*item_ptr).record_id;
                        found = true;
                    }
                }

                if found {
                    Ok(best_record_id)
                } else {
                    Err(RemDbError::RecordNotFound)
                }
            }
        };

        crate::platform::spin_unlock(&mut self.lock);

        if result.is_ok() {
            self.stats.hit_count += 1;
        }

        result
    }

    /// 向量范围查询
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        _start_key_size: usize,
        end_key: *const u8,
        _end_key_size: usize,
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 解析查询向量
        let query_vec: *const f32;
        let mut query_vec_buf: [f32; 1024];
        let vec_len = self.dimension as usize;

        // 检查key是否是指向字符串的指针（向量字面量）
        if _start_key_size > 4 && *start_key == b'[' {
            // 解析向量字面�?[x1, x2, ..., xn]
            let vec_str =
                core::str::from_utf8(core::slice::from_raw_parts(start_key, _start_key_size))
                    .map_err(|_| RemDbError::TypeMismatch)?;

            // 移除首尾的方括号
            let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');

            // 分割逗号，得到每个元素的字符�?
            let elements: Vec<&str> = vec_str.split(',').map(|s| s.trim()).collect();

            // 检查维度是否匹�?
            if elements.len() != vec_len {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TypeMismatch);
            }

            // 解析每个元素为f32
            query_vec_buf = [0.0; 1024];
            for (i, elem) in elements.iter().enumerate() {
                query_vec_buf[i] = elem.parse::<f32>().map_err(|_| RemDbError::TypeMismatch)?;
            }

            query_vec = query_vec_buf.as_ptr();
        } else {
            // 直接使用key作为f32指针
            query_vec = start_key as *const f32;
        }

        // end_key是距离阈值，解析为f32
        let range_value: f32;
        if _end_key_size > 4 && *end_key == b'[' {
            // 解析向量字面量的距离（这种情况不常见，主要用于测试）
            range_value = 1000.0; // 默认大值，返回所有向�?
        } else {
            // end_key是距离阈值的指针，正确转换并读取该�?
            range_value = *(end_key as *const f32);
        }

        // 线性搜索查找第一个匹配的向量
        for i in 0..self.item_count {
            let item_ptr = self.items.add(i);
            let vec_ptr = self.vectors.add((*item_ptr).vector_offset);
            let distance = self.calculate_distance(vec_ptr, query_vec);

            // 检查向量是否在范围�?
            if distance <= range_value {
                crate::platform::spin_unlock(&mut self.lock);
                self.stats.hit_count += 1;
                return Ok((*item_ptr).record_id);
            }
        }

        crate::platform::spin_unlock(&mut self.lock);

        Err(RemDbError::RecordNotFound)
    }

    /// 向量范围查询（返回所有匹配项�?
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        _start_key_size: usize,
        end_key: *const u8,
        _end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize,
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 解析查询向量
        let query_vec: *const f32;
        let mut query_vec_buf: [f32; 1024];
        let vec_len = self.dimension as usize;

        // 检查key是否是指向字符串的指针（向量字面量）
        if _start_key_size > 4 && *start_key == b'[' {
            // 解析向量字面�?[x1, x2, ..., xn]
            let vec_str =
                core::str::from_utf8(core::slice::from_raw_parts(start_key, _start_key_size))
                    .map_err(|_| RemDbError::TypeMismatch)?;

            // 移除首尾的方括号
            let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');

            // 分割逗号，得到每个元素的字符�?
            let elements: Vec<&str> = vec_str.split(',').map(|s| s.trim()).collect();

            // 检查维度是否匹�?
            if elements.len() != vec_len {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TypeMismatch);
            }

            // 解析每个元素为f32
            query_vec_buf = [0.0; 1024];
            for (i, elem) in elements.iter().enumerate() {
                query_vec_buf[i] = elem.parse::<f32>().map_err(|_| RemDbError::TypeMismatch)?;
            }

            query_vec = query_vec_buf.as_ptr();
        } else {
            // 直接使用key作为f32指针
            query_vec = start_key as *const f32;
        }

        // end_key是距离阈值，解析为f32
        let range_value: f32;
        if _end_key_size > 4 && *end_key == b'[' {
            // 解析向量字面量的距离（这种情况不常见，主要用于测试）
            range_value = 1000.0; // 默认大值，返回所有向�?
        } else {
            // end_key是距离阈值的指针，正确转换并读取该�?
            range_value = *(end_key as *const f32);
        }

        // 直接在输出缓冲区中存储结果，避免使用Vec
        let mut match_count = 0;

        // 实现真正的范围查询：返回所有距离小于等于range_value的向�?
        for i in 0..self.item_count {
            if match_count >= max_records {
                break;
            }

            let item_ptr = self.items.add(i);
            let vec_ptr = self.vectors.add((*item_ptr).vector_offset);
            let distance = self.calculate_distance(query_vec, vec_ptr);

            // 检查距离是否在范围�?
            if distance <= range_value {
                *out_record_ids.add(match_count) = (*item_ptr).record_id;
                match_count += 1;
            }
        }

        crate::platform::spin_unlock(&mut self.lock);

        if match_count > 0 {
            self.stats.hit_count += match_count;
        }

        Ok(match_count)
    }

    /// 删除向量索引�?
    pub unsafe fn delete(&mut self, key: *const u8, _key_size: usize) -> Result<()> {
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        let query_vec = key as *const f32;
        let vec_len = self.dimension as usize;

        // 查找要删除的向量
        let mut found_idx = None;
        for i in 0..self.item_count {
            let item_ptr = self.items.add(i);
            let vec_ptr = self.vectors.add((*item_ptr).vector_offset);

            let mut match_found = true;
            for j in 0..vec_len {
                if *query_vec.add(j) != *vec_ptr.add(j) {
                    match_found = false;
                    break;
                }
            }
            if match_found {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            // 获取要删除向量的偏移�?
            let deleted_item_ptr = self.items.add(idx);
            let deleted_offset = (*deleted_item_ptr).vector_offset;

            // 实际删除逻辑：将最后一个元素移动到被删除位�?
            if idx < self.item_count - 1 {
                // 获取最后一个元素和其向量偏移量
                let last_item_ptr = self.items.add(self.item_count - 1);
                let last_offset = (*last_item_ptr).vector_offset;

                // 复制最后一个元素的向量数据到被删除向量的位�?
                let src_vec_ptr = self.vectors.add(last_offset);
                let dst_vec_ptr = self.vectors.add(deleted_offset);
                for i in 0..vec_len {
                    *dst_vec_ptr.add(i) = *src_vec_ptr.add(i);
                }

                // 复制最后一个元素到被删除位置，并更新其向量偏移�?
                *deleted_item_ptr = *last_item_ptr;
                (*deleted_item_ptr).vector_offset = deleted_offset;
            }

            // 更新计数（item_count和vector_count必须保持同步�?
            self.item_count -= 1;
            self.vector_count = self.item_count; // 确保vector_count与item_count同步

            // 更新统计信息
            self.stats.item_count -= 1;
            self.stats.size -=
                core::mem::size_of::<VectorIndexItem>() + vec_len * core::mem::size_of::<f32>();
        }

        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats = IndexStats::default();
    }
}

pub enum AnySecondaryIndex {
    /// 有序数组索引
    SortedArray(SecondaryIndex),
    /// B-Tree索引
    BTree(BTreeIndex),
    /// T-Tree索引
    TTree(TTreeIndex),
    /// 向量索引
    Vector(VectorIndex),
}

impl AnySecondaryIndex {
    /// 创建新的辅助索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        memory_start: *mut u8,
        max_items: usize,
    ) -> Result<Self> {
        match def.secondary_index_type {
            IndexType::SortedArray => {
                // 创建有序数组索引
                let index =
                    SecondaryIndex::new(def, memory_start as *mut SecondaryIndexItem, max_items);
                Ok(AnySecondaryIndex::SortedArray(index))
            }
            IndexType::BTree => {
                // 创建B-Tree索引
                let index = BTreeIndex::new(def, memory_start as *mut BTreeNode, max_items);
                Ok(AnySecondaryIndex::BTree(index))
            }
            IndexType::TTree => {
                // 创建T-Tree索引
                let index = TTreeIndex::new(def, memory_start as *mut TTreeNode, max_items);
                Ok(AnySecondaryIndex::TTree(index))
            }
            IndexType::Vector => {
                // 创建向量索引
                let index = VectorIndex::new(def, memory_start, max_items)?;
                Ok(AnySecondaryIndex::Vector(index))
            }
            _ => Err(RemDbError::UnsupportedOperation),
        }
    }

    /// 计算辅助索引所需的内存大�?
    pub fn calculate_memory_size(def: &TableDef, max_items: usize) -> usize {
        match def.secondary_index_type {
            IndexType::SortedArray => SecondaryIndex::calculate_memory_size(max_items),
            IndexType::BTree => BTreeIndex::calculate_memory_size(max_items),
            IndexType::TTree => TTreeIndex::calculate_memory_size(max_items),
            IndexType::Vector => VectorIndex::calculate_memory_size(def, max_items),
            _ => 0,
        }
    }

    /// 插入索引�?
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::BTree(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::TTree(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::Vector(index) => index.insert(key, key_size, record_id),
        }
    }

    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find(key, key_size),
            AnySecondaryIndex::BTree(index) => index.find(key, key_size),
            AnySecondaryIndex::TTree(index) => index.find(key, key_size),
            AnySecondaryIndex::Vector(index) => index.find(key, key_size),
        }
    }

    /// 范围查询（返回第一个匹配项�?
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
    ) -> Result<u16> {
        match self {
            AnySecondaryIndex::SortedArray(index) => {
                index.find_range(start_key, start_key_size, end_key, end_key_size)
            }
            AnySecondaryIndex::BTree(index) => {
                index.find_range(start_key, start_key_size, end_key, end_key_size)
            }
            AnySecondaryIndex::TTree(index) => {
                index.find_range(start_key, start_key_size, end_key, end_key_size)
            }
            AnySecondaryIndex::Vector(index) => {
                index.find_range(start_key, start_key_size, end_key, end_key_size)
            }
        }
    }

    /// 范围查询（返回所有匹配项�?
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize,
    ) -> Result<usize> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find_range_all(
                start_key,
                start_key_size,
                end_key,
                end_key_size,
                out_record_ids,
                max_records,
            ),
            AnySecondaryIndex::BTree(index) => index.find_range_all(
                start_key,
                start_key_size,
                end_key,
                end_key_size,
                out_record_ids,
                max_records,
            ),
            AnySecondaryIndex::TTree(index) => index.find_range_all(
                start_key,
                start_key_size,
                end_key,
                end_key_size,
                out_record_ids,
                max_records,
            ),
            AnySecondaryIndex::Vector(index) => index.find_range_all(
                start_key,
                start_key_size,
                end_key,
                end_key_size,
                out_record_ids,
                max_records,
            ),
        }
    }

    /// 删除索引�?
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.delete(key, key_size),
            AnySecondaryIndex::BTree(index) => index.delete(key, key_size),
            AnySecondaryIndex::TTree(index) => index.delete(key, key_size),
            AnySecondaryIndex::Vector(index) => index.delete(key, key_size),
        }
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.stats(),
            AnySecondaryIndex::BTree(index) => index.stats(),
            AnySecondaryIndex::TTree(index) => index.stats(),
            AnySecondaryIndex::Vector(index) => index.stats(),
        }
    }

    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.reset_stats(),
            AnySecondaryIndex::BTree(index) => index.reset_stats(),
            AnySecondaryIndex::TTree(index) => index.reset_stats(),
            AnySecondaryIndex::Vector(index) => index.reset_stats(),
        }
    }
}

/// 辅助有序索引
pub struct SecondaryIndex {
    /// 表定�?
    def: alloc::sync::Arc<TableDef>,
    /// 索引项数�?
    items: NonNull<SecondaryIndexItem>,
    /// 当前项数�?
    item_count: usize,
    /// 最大项数量
    max_items: usize,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋�?
    lock: u32,
}

impl SecondaryIndex {
    /// 创建新的辅助索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        items_start: *mut SecondaryIndexItem,
        max_items: usize,
    ) -> Self {
        let items = NonNull::new_unchecked(items_start);

        SecondaryIndex {
            def,
            items,
            item_count: 0,
            max_items,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: max_items * core::mem::size_of::<SecondaryIndexItem>(),
                item_count: 0,
            },
            lock: 0,
        }
    }

    /// 计算辅助索引所需的内存大�?
    pub const fn calculate_memory_size(max_items: usize) -> usize {
        max_items * core::mem::size_of::<SecondaryIndexItem>()
    }

    /// 比较两个索引�?
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem,
    ) -> core::cmp::Ordering {
        // 比较键大小
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }

        // 检查是否是字符串字段的索引
        if let Some(secondary_index) = &self.def.secondary_index {
            if let Some(field_index) = secondary_index.first() {
                if *field_index < self.def.fields.len() {
                    let field = &self.def.fields[*field_index];
                    if field.data_type == crate::types::DataType::String {
                        // 尝试使用UTF-8处理器比较字符串
                        if let (Some(str1), Some(str2)) = (
                            crate::utf8::get_global_utf8_processor().to_string(&item1.key_data[..item1.key_size as usize]),
                            crate::utf8::get_global_utf8_processor().to_string(&item2.key_data[..item2.key_size as usize])
                        ) {
                            let cmp = str1.cmp(str2);
                            if cmp != core::cmp::Ordering::Equal {
                                return cmp;
                            }
                        }
                    }
                }
            }
        }

        // 比较键数据（字节级比较，作为回退）
        let key_size = item1.key_size as usize;
        for i in 0..key_size {
            if item1.key_data[i] != item2.key_data[i] {
                return item1.key_data[i].cmp(&item2.key_data[i]);
            }
        }

        // 键相等，比较记录ID
        item1.record_id.cmp(&item2.record_id)
    }

    /// 二分查找索引�?
    fn binary_search(&self, key: *const u8, key_size: usize) -> Result<usize> {
        if self.item_count == 0 {
            return Err(RemDbError::RecordNotFound);
        }

        let mut low = 0;
        let mut high = self.item_count - 1;

        while low <= high {
            let mid = (low + high) / 2;
            let mid_item = unsafe { &*self.items.as_ptr().add(mid) };

            // 比较�?
            let cmp = if mid_item.key_size != key_size as u8 {
                mid_item.key_size.cmp(&(key_size as u8))
            } else {
                let mut equal = true;
                for i in 0..key_size {
                    if mid_item.key_data[i] != unsafe { *key.add(i) } {
                        equal = false;
                        break;
                    }
                }
                if equal {
                    core::cmp::Ordering::Equal
                } else {
                    // 逐个字节比较
                    let mut ordering = core::cmp::Ordering::Equal;
                    for i in 0..key_size {
                        let b1 = mid_item.key_data[i];
                        let b2 = unsafe { *key.add(i) };
                        if b1 != b2 {
                            ordering = b1.cmp(&b2);
                            break;
                        }
                    }
                    ordering
                }
            };

            match cmp {
                core::cmp::Ordering::Equal => return Ok(mid),
                core::cmp::Ordering::Less => low = mid + 1,
                core::cmp::Ordering::Greater => {
                    if mid == 0 {
                        return Err(RemDbError::RecordNotFound);
                    }
                    high = mid - 1;
                }
            }
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 插入索引�?
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 检查是否已�?
        if self.item_count >= self.max_items {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::OutOfMemory);
        }

        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建新索引项
        let new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 128],
        };
        memcpy(new_item.key_data.as_ptr() as *mut u8, key, key_size);

        // 使用二分查找找到插入位置，将O(n)优化为O(log n)
        let mut insert_pos = self.item_count;
        if self.item_count > 0 {
            let mut low = 0;
            let mut high = self.item_count - 1;

            while low <= high {
                let mid = (low + high) / 2;
                let item = &*self.items.as_ptr().add(mid);

                match self.compare_items(&new_item, item) {
                    core::cmp::Ordering::Less => {
                        insert_pos = mid;
                        high = mid - 1;
                    }
                    core::cmp::Ordering::Greater => {
                        low = mid + 1;
                    }
                    core::cmp::Ordering::Equal => {
                        // 插入到相等元素后�?
                        insert_pos = mid + 1;
                        low = mid + 1;
                    }
                }
            }
        }

        // 移动现有项为新项腾出空间
        if insert_pos < self.item_count {
            // 从后往前拷贝，避免覆盖数据
            for i in (insert_pos..self.item_count).rev() {
                let src = self.items.as_ptr().add(i);
                let dest = self.items.as_ptr().add(i + 1);
                *dest = *src;
            }
        }

        // 插入新项
        let insert_ptr = self.items.as_ptr().add(insert_pos);
        *insert_ptr = new_item;

        // 更新统计信息
        self.item_count += 1;
        self.stats.item_count = self.item_count;

        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }

    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        match self.binary_search(key, key_size) {
            Ok(index) => {
                // 更新命中统计
                self.stats.hit_count += 1;
                Ok((*self.items.as_ptr().add(index)).record_id)
            }
            Err(e) => Err(e),
        }
    }

    /// 删除索引�?
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        let result = match self.binary_search(key, key_size) {
            Ok(index) => {
                // 移动后续项覆盖被删除�?
                if index < self.item_count - 1 {
                    let dest_ptr = self.items.as_ptr().add(index);
                    let src_ptr = self.items.as_ptr().add(index + 1);
                    let move_size =
                        (self.item_count - index - 1) * core::mem::size_of::<SecondaryIndexItem>();
                    memcpy(dest_ptr as *mut u8, src_ptr as *const u8, move_size);
                }

                // 清空最后一�?
                let last_ptr = self.items.as_ptr().add(self.item_count - 1);
                memset(
                    last_ptr as *mut u8,
                    0,
                    core::mem::size_of::<SecondaryIndexItem>(),
                );

                // 更新统计信息
                self.item_count -= 1;
                self.stats.item_count = self.item_count;

                Ok(())
            }
            Err(e) => Err(e),
        };

        crate::platform::spin_unlock(&mut self.lock);
        result
    }

    /// 范围查询（返回第一个匹配项�?
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.item_count - 1;

        // 创建临时索引项用于比�?
        let start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(
            start_item.key_data.as_ptr() as *mut u8,
            start_key,
            start_key_size,
        );

        // 二分查找起始位置
        while low <= high {
            let mid = (low + high) / 2;
            let item = &*self.items.as_ptr().add(mid);

            if self.compare_items(item, &start_item) == core::cmp::Ordering::Less {
                start_pos = mid + 1;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }

        // 从起始位置开始遍历，直到找到匹配项或超出范围
        for i in start_pos..self.item_count {
            let item = &*self.items.as_ptr().add(i);

            // 检查是否小于等于end_key
            let mut le_end = false;
            let key_size = item.key_size as usize;

            if key_size > end_key_size {
                // 键大小大于end_key，超出范�?
                break;
            }

            // 比较键数�?
            let min_size = core::cmp::min(key_size, end_key_size);
            let mut all_equal = true;

            for j in 0..min_size {
                if item.key_data[j] < *end_key.add(j) {
                    le_end = true;
                    break;
                } else if item.key_data[j] > *end_key.add(j) {
                    // 超出范围
                    le_end = false;
                    all_equal = false;
                    break;
                }
            }

            if all_equal && key_size == end_key_size {
                le_end = true;
            }

            if le_end {
                // 更新命中统计
                self.stats.hit_count += 1;
                return Ok(item.record_id);
            } else {
                // 已超出范围，结束遍历
                break;
            }
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 范围查询（返回所有匹配项�?
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize,
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 检查输出缓冲区是否为null
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 如果没有记录，直接返�?
        if self.item_count == 0 {
            return Ok(0);
        }

        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.item_count - 1;

        // 创建临时索引项用于比�?
        let start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(
            start_item.key_data.as_ptr() as *mut u8,
            start_key,
            start_key_size,
        );

        // 二分查找起始位置
        while low <= high {
            let mid = (low + high) / 2;
            let item = &*self.items.as_ptr().add(mid);

            if self.compare_items(item, &start_item) == core::cmp::Ordering::Less {
                start_pos = mid + 1;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }

        // 从起始位置开始遍历，收集所有匹配项
        let mut match_count = 0;
        for i in start_pos..self.item_count {
            if match_count >= max_records {
                break;
            }

            let item = &*self.items.as_ptr().add(i);

            // 检查是否小于等于end_key
            let mut le_end = false;
            let key_size = item.key_size as usize;

            if key_size > end_key_size {
                // 键大小大于end_key，超出范�?
                break;
            }

            // 比较键数�?
            let min_size = core::cmp::min(key_size, end_key_size);
            let mut all_equal = true;

            for j in 0..min_size {
                if item.key_data[j] < *end_key.add(j) {
                    le_end = true;
                    break;
                } else if item.key_data[j] > *end_key.add(j) {
                    // 超出范围
                    le_end = false;
                    all_equal = false;
                    break;
                }
            }

            if all_equal && key_size == end_key_size {
                le_end = true;
            }

            if le_end {
                // 保存匹配项的record_id到输出缓冲区
                *out_record_ids.add(match_count) = item.record_id;
                match_count += 1;
            } else {
                // 已超出范围，结束遍历
                break;
            }
        }

        // 更新命中统计
        if match_count > 0 {
            self.stats.hit_count += match_count;
        }

        Ok(match_count)
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats.access_count = 0;
        self.stats.hit_count = 0;
    }

    /// 获取当前项数�?
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// 获取最大项数量
    pub fn max_items(&self) -> usize {
        self.max_items
    }
}

impl BTreeIndex {
    /// 创建新的B-Tree索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        nodes_start: *mut BTreeNode,
        max_nodes: usize,
    ) -> Self {
        // 初始化节点池
        let nodes = NonNull::new_unchecked(nodes_start);
        let mut free_nodes: Option<NonNull<BTreeNode>> = None;

        // 将所有节点链接到空闲列表
        for i in (0..max_nodes).rev() {
            let node_ptr = nodes.as_ptr().add(i);
            let node_mut = &mut *node_ptr;

            // 初始化节�?
            node_mut.is_leaf = true;
            node_mut.key_count = 0;

            // 初始化键
            for j in 0..BTREE_ORDER {
                node_mut.keys[j].key_size = 0;
                node_mut.keys[j].record_id = 0;
                memset(node_mut.keys[j].key_data.as_mut_ptr(), 0, 64);
            }

            // 初始化子节点指针
            for j in 0..(BTREE_ORDER + 1) {
                node_mut.children[j] = None;
            }

            // 添加到空闲列�?
            // 使用第一个键的key_data字段作为下一个节点的指针
            // 由于key_data�?4字节，足够存储一个指�?
            let next_ptr = free_nodes
                .map(|p: NonNull<BTreeNode>| p.as_ptr() as u64)
                .unwrap_or(0);
            memcpy(
                node_mut.keys[0].key_data.as_mut_ptr(),
                &next_ptr as *const u64 as *const u8,
                core::mem::size_of::<u64>(),
            );
            free_nodes = Some(NonNull::new_unchecked(node_ptr));
        }

        BTreeIndex {
            def,
            root: None,
            nodes,
            free_nodes,
            max_nodes,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: max_nodes * core::mem::size_of::<BTreeNode>(),
                item_count: 0,
            },
            lock: 0,
        }
    }

    /// 计算B-Tree索引所需的内存大�?
    pub const fn calculate_memory_size(max_nodes: usize) -> usize {
        max_nodes * core::mem::size_of::<BTreeNode>()
    }

    /// 从空闲列表获取一个节�?
    unsafe fn allocate_node(&mut self) -> Option<NonNull<BTreeNode>> {
        let node_ptr = self.free_nodes?;
        let node_mut = &mut *node_ptr.as_ptr();

        // 从节点的key_data字段获取下一个空闲节点的指针
        let mut next_ptr = 0u64;
        memcpy(
            &mut next_ptr as *mut u64 as *mut u8,
            node_mut.keys[0].key_data.as_ptr(),
            core::mem::size_of::<u64>(),
        );

        self.free_nodes = if next_ptr == 0 {
            None
        } else {
            NonNull::new(next_ptr as *mut BTreeNode)
        };

        // 重置节点
        node_mut.is_leaf = true;
        node_mut.key_count = 0;

        // 初始化键
        for j in 0..BTREE_ORDER {
            node_mut.keys[j].key_size = 0;
            node_mut.keys[j].record_id = 0;
            memset(node_mut.keys[j].key_data.as_mut_ptr(), 0, 64);
        }

        // 初始化子节点指针
        for j in 0..(BTREE_ORDER + 1) {
            node_mut.children[j] = None;
        }

        Some(node_ptr)
    }

    /// 释放节点到空闲列�?
    unsafe fn free_node(&mut self, node_ptr: NonNull<BTreeNode>) {
        let node_mut = &mut *node_ptr.as_ptr();

        // 将当前空闲列表头指针存储到节点的key_data字段
        let next_ptr = self.free_nodes.map(|p| p.as_ptr() as u64).unwrap_or(0);
        memcpy(
            node_mut.keys[0].key_data.as_mut_ptr(),
            &next_ptr as *const u64 as *const u8,
            core::mem::size_of::<u64>(),
        );

        // 添加到空闲列表头
        self.free_nodes = Some(node_ptr);
    }

    /// 比较两个索引�?
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem,
    ) -> core::cmp::Ordering {
        // 比较键大小
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }

        // 检查是否是字符串字段的索引
        if let Some(secondary_index) = &self.def.secondary_index {
            if let Some(field_index) = secondary_index.first() {
                if *field_index < self.def.fields.len() {
                    let field = &self.def.fields[*field_index];
                    if field.data_type == crate::types::DataType::String {
                        // 尝试使用UTF-8处理器比较字符串
                        if let (Some(str1), Some(str2)) = (
                            crate::utf8::get_global_utf8_processor().to_string(&item1.key_data[..item1.key_size as usize]),
                            crate::utf8::get_global_utf8_processor().to_string(&item2.key_data[..item2.key_size as usize])
                        ) {
                            let cmp = str1.cmp(str2);
                            if cmp != core::cmp::Ordering::Equal {
                                return cmp;
                            }
                        }
                    }
                }
            }
        }

        // 比较键数据（字节级比较，作为回退）
        let key_size = item1.key_size as usize;
        for i in 0..key_size {
            if item1.key_data[i] != item2.key_data[i] {
                return item1.key_data[i].cmp(&item2.key_data[i]);
            }
        }

        // 键相等，比较记录ID
        item1.record_id.cmp(&item2.record_id)
    }

    /// 在节点中查找键的插入位置
    fn find_key_position(&self, node: &BTreeNode, key: &SecondaryIndexItem) -> usize {
        let mut pos = 0;
        while pos < node.key_count as usize
            && self.compare_items(&node.keys[pos], key) == core::cmp::Ordering::Less
        {
            pos += 1;
        }
        pos
    }

    /// 分割满节�?
    unsafe fn split_child(
        &mut self,
        mut parent: NonNull<BTreeNode>,
        child_idx: usize,
        mut child: NonNull<BTreeNode>,
    ) {
        let parent_mut = parent.as_mut();
        let child_mut = child.as_mut();

        // 创建新节�?
        let mut new_node = self.allocate_node().expect("Out of memory for B-Tree node");
        let new_node_mut = new_node.as_mut();

        new_node_mut.is_leaf = child_mut.is_leaf;
        new_node_mut.key_count = (BTREE_ORDER / 2) as u8;

        // 复制后半部分键到新节�?
        for i in 0..(BTREE_ORDER / 2) {
            new_node_mut.keys[i] = child_mut.keys[i + (BTREE_ORDER / 2) + 1];
        }

        // 如果是内部节点，复制后半部分子节�?
        if !child_mut.is_leaf {
            for i in 0..(BTREE_ORDER / 2 + 1) {
                new_node_mut.children[i] = child_mut.children[i + (BTREE_ORDER / 2) + 1];
            }
        }

        // 更新原节点的键数�?
        child_mut.key_count = (BTREE_ORDER / 2) as u8;

        // 移动父节点的键和子节点指针，为新节点腾出空间
        for i in (child_idx + 1..=parent_mut.key_count as usize).rev() {
            parent_mut.keys[i] = parent_mut.keys[i - 1];
            parent_mut.children[i + 1] = parent_mut.children[i];
        }

        // 将中间键提升到父节点
        parent_mut.keys[child_idx] = child_mut.keys[BTREE_ORDER / 2];
        parent_mut.children[child_idx + 1] = Some(new_node);
        parent_mut.key_count += 1;
    }

    /// 插入键到非满节点
    unsafe fn insert_non_full(&mut self, mut node: NonNull<BTreeNode>, key: SecondaryIndexItem) {
        let node_mut = node.as_mut();
        let mut pos = self.find_key_position(node_mut, &key);

        if node_mut.is_leaf {
            // 叶子节点，直接插�?
            for i in (pos..node_mut.key_count as usize).rev() {
                node_mut.keys[i + 1] = node_mut.keys[i];
            }
            node_mut.keys[pos] = key;
            node_mut.key_count += 1;
            self.stats.item_count += 1;
        } else {
            // 内部节点，递归插入
            let child = node_mut.children[pos].expect("Child node not found");

            // 如果子节点已满，先分�?
            if child.as_ref().key_count == BTREE_ORDER as u8 {
                self.split_child(node, pos, child);

                // 检查中间键是否大于当前�?
                if self.compare_items(&node_mut.keys[pos], &key) == core::cmp::Ordering::Less {
                    pos += 1;
                }
            }

            self.insert_non_full(
                node_mut.children[pos].expect("Child node not found after split"),
                key,
            );
        }
    }

    /// 插入索引�?
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建索引�?
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 128],
        };
        memcpy(new_item.key_data.as_mut_ptr(), key, key_size);

        if self.root.is_none() {
            // 空树，创建根节点
            let mut root_node = self
                .allocate_node()
                .expect("Out of memory for B-Tree root node");
            let root_mut = root_node.as_mut();

            root_mut.keys[0] = new_item;
            root_mut.key_count = 1;
            self.root = Some(root_node);
        } else {
            let root = self.root.expect("Root node unexpectedly None");

            // 如果根节点已满，分裂根节�?
            if root.as_ref().key_count == BTREE_ORDER as u8 {
                let mut new_root = self
                    .allocate_node()
                    .expect("Out of memory for new B-Tree root");
                let new_root_mut = new_root.as_mut();

                new_root_mut.is_leaf = false;
                new_root_mut.key_count = 0;
                new_root_mut.children[0] = self.root;

                self.split_child(new_root, 0, root);
                self.insert_non_full(new_root, new_item);

                self.root = Some(new_root);
            } else {
                self.insert_non_full(root, new_item);
            }
        }

        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }

    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 检查键大小
        if key_size > 64 {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建临时索引项用于比�?
        let mut search_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(search_item.key_data.as_mut_ptr(), key, key_size);

        let mut current = self.root;
        while let Some(node) = current {
            let node_ref = node.as_ref();
            let mut pos = 0;

            // 查找键位�?
            while pos < node_ref.key_count as usize
                && self.compare_items(&node_ref.keys[pos], &search_item)
                    == core::cmp::Ordering::Less
            {
                pos += 1;
            }

            // 检查是否找到键
            if pos < node_ref.key_count as usize {
                let cmp = self.compare_items(&node_ref.keys[pos], &search_item);
                if cmp == core::cmp::Ordering::Equal {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(node_ref.keys[pos].record_id);
                }
            }

            // 如果是叶子节点，未找�?
            if node_ref.is_leaf {
                break;
            }

            // 继续搜索子节�?
            current = node_ref.children[pos];
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 范围查询（返回第一个匹配项�?
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 实现范围查询逻辑
        // 简化实现：找到起始位置后，遍历直到找到第一个匹配项

        // 创建临时索引项用于比�?
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);

        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(end_item.key_data.as_mut_ptr(), end_key, end_key_size);

        let mut current = self.root;
        let mut stack = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;

        // 遍历树，直到找到叶子节点
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;

            let node_ref = node.as_ref();

            // 如果是叶子节点，跳出循环
            if node_ref.is_leaf {
                break;
            }

            // 找到第一个大于等于start_key的子节点
            let mut pos = 0;
            while pos < node_ref.key_count as usize
                && self.compare_items(&node_ref.keys[pos], &start_item) == core::cmp::Ordering::Less
            {
                pos += 1;
            }

            current = node_ref.children[pos];
        }

        // 从栈中回溯，查找匹配�?
        while stack_size > 0 {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            let node_ref = node.as_ref();

            // 查找起始位置
            let mut start_pos = 0;
            while start_pos < node_ref.key_count as usize
                && self.compare_items(&node_ref.keys[start_pos], &start_item)
                    == core::cmp::Ordering::Less
            {
                start_pos += 1;
            }

            // 遍历当前节点的键
            for i in start_pos..node_ref.key_count as usize {
                let key = &node_ref.keys[i];

                // 检查是否在范围�?
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    continue; // 超出范围，继续查找下一个节�?
                }

                // 找到匹配�?
                self.stats.hit_count += 1;
                return Ok(key.record_id);
            }

            // 如果不是叶子节点，继续搜索右子树
            if !node_ref.is_leaf {
                let mut child = node_ref.children[node_ref.key_count as usize];
                while let Some(child_node) = child {
                    let child_ref = child_node.as_ref();

                    // 遍历子节点的�?
                    for i in 0..child_ref.key_count as usize {
                        let key = &child_ref.keys[i];

                        // 检查是否在范围�?
                        if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                            break; // 超出范围，结束搜�?
                        }

                        // 找到匹配�?
                        self.stats.hit_count += 1;
                        return Ok(key.record_id);
                    }

                    // 如果是叶子节点，结束搜索
                    if child_ref.is_leaf {
                        break;
                    }

                    // 继续搜索第一个子节点
                    child = child_ref.children[0];
                }
            }
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 范围查询（返回所有匹配项�?
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize,
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建临时索引项用于比�?
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);

        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(end_item.key_data.as_mut_ptr(), end_key, end_key_size);

        let mut match_count = 0;
        let mut stack = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;

        // 遍历树，直到找到叶子节点
        let mut current = self.root;
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;

            let node_ref = node.as_ref();

            // 如果是叶子节点，跳出循环
            if node_ref.is_leaf {
                break;
            }

            // 找到第一个大于等于start_key的子节点
            let mut pos = 0;
            while pos < node_ref.key_count as usize
                && self.compare_items(&node_ref.keys[pos], &start_item) == core::cmp::Ordering::Less
            {
                pos += 1;
            }

            current = node_ref.children[pos];
        }

        // 从栈中回溯，收集所有匹配项
        while stack_size > 0 && match_count < max_records {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            let node_ref = node.as_ref();

            // 查找起始位置
            let mut start_pos = 0;
            while start_pos < node_ref.key_count as usize
                && self.compare_items(&node_ref.keys[start_pos], &start_item)
                    == core::cmp::Ordering::Less
            {
                start_pos += 1;
            }

            // 遍历当前节点的键
            for i in start_pos..node_ref.key_count as usize {
                if match_count >= max_records {
                    break;
                }

                let key = &node_ref.keys[i];

                // 检查是否在范围�?
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    continue; // 超出范围，继续查找下一个节�?
                }

                // 添加到结�?
                *out_record_ids.add(match_count) = key.record_id;
                match_count += 1;
            }

            // 如果不是叶子节点，继续搜索右子树
            if !node_ref.is_leaf && match_count < max_records {
                let mut child = node_ref.children[node_ref.key_count as usize];
                while let Some(child_node) = child {
                    let child_ref = child_node.as_ref();

                    // 遍历子节点的�?
                    for i in 0..child_ref.key_count as usize {
                        if match_count >= max_records {
                            break;
                        }

                        let key = &child_ref.keys[i];

                        // 检查是否在范围�?
                        if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                            break; // 超出范围，结束搜�?
                        }

                        // 添加到结�?
                        *out_record_ids.add(match_count) = key.record_id;
                        match_count += 1;
                    }

                    // 如果是叶子节点，结束搜索
                    if child_ref.is_leaf || match_count >= max_records {
                        break;
                    }

                    // 继续搜索第一个子节点
                    child = child_ref.children[0];
                }
            }
        }

        // 更新命中统计
        if match_count > 0 {
            self.stats.hit_count += match_count;
        }

        Ok(match_count)
    }

    /// 删除索引�?
    pub unsafe fn delete(&mut self, _key: *const u8, _key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 简化实现：暂不支持删除操作
        // 完整的B-Tree删除实现比较复杂，需要处理多种情�?
        // 包括合并节点、借键�?

        crate::platform::spin_unlock(&mut self.lock);
        Err(RemDbError::UnsupportedOperation)
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats.access_count = 0;
        self.stats.hit_count = 0;
    }
}

impl TTreeIndex {
    /// 创建新的T-Tree索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        nodes_start: *mut TTreeNode,
        max_nodes: usize,
    ) -> Self {
        // 初始化节点池
        let nodes = NonNull::new_unchecked(nodes_start);
        let mut free_nodes: Option<NonNull<TTreeNode>> = None;

        // 将所有节点链接到空闲列表
        for i in (0..max_nodes).rev() {
            let node_ptr = nodes.as_ptr().add(i);
            let node_mut = &mut *node_ptr;

            // 初始化节�?
            node_mut.key_count = 0;

            // 初始化键
            for j in 0..TTREE_ORDER {
                node_mut.keys[j].key_size = 0;
                node_mut.keys[j].record_id = 0;
                memset(node_mut.keys[j].key_data.as_mut_ptr(), 0, 64);
            }

            // 初始化子节点指针
            node_mut.left = None;
            node_mut.middle = None;
            node_mut.right = None;

            // 添加到空闲列�?
            // 使用第一个键的key_data字段作为下一个节点的指针
            let next_ptr = free_nodes
                .map(|p: NonNull<TTreeNode>| p.as_ptr() as u64)
                .unwrap_or(0);
            memcpy(
                node_mut.keys[0].key_data.as_mut_ptr(),
                &next_ptr as *const u64 as *const u8,
                core::mem::size_of::<u64>(),
            );
            free_nodes = Some(NonNull::new_unchecked(node_ptr));
        }

        TTreeIndex {
            def,
            root: None,
            nodes,
            free_nodes,
            max_nodes,
            stats: IndexStats {
                access_count: 0,
                hit_count: 0,
                size: max_nodes * core::mem::size_of::<TTreeNode>(),
                item_count: 0,
            },
            lock: 0,
        }
    }

    /// 计算T-Tree索引所需的内存大�?
    pub const fn calculate_memory_size(max_nodes: usize) -> usize {
        max_nodes * core::mem::size_of::<TTreeNode>()
    }

    /// 从空闲列表获取一个节�?
    unsafe fn allocate_node(&mut self) -> Option<NonNull<TTreeNode>> {
        let node_ptr = self.free_nodes?;
        let node_mut = &mut *node_ptr.as_ptr();

        // 从节点的key_data字段获取下一个空闲节点的指针
        let mut next_ptr = 0u64;
        memcpy(
            &mut next_ptr as *mut u64 as *mut u8,
            node_mut.keys[0].key_data.as_ptr(),
            core::mem::size_of::<u64>(),
        );

        self.free_nodes = if next_ptr == 0 {
            None
        } else {
            NonNull::new(next_ptr as *mut TTreeNode)
        };

        // 重置节点
        node_mut.key_count = 0;

        // 初始化键
        for j in 0..TTREE_ORDER {
            node_mut.keys[j].key_size = 0;
            node_mut.keys[j].record_id = 0;
            memset(node_mut.keys[j].key_data.as_mut_ptr(), 0, 64);
        }

        // 初始化子节点指针
        node_mut.left = None;
        node_mut.middle = None;
        node_mut.right = None;

        Some(node_ptr)
    }

    /// 释放节点到空闲列�?
    unsafe fn free_node(&mut self, node_ptr: NonNull<TTreeNode>) {
        let node_mut = &mut *node_ptr.as_ptr();

        // 将当前空闲列表头指针存储到节点的key_data字段
        let next_ptr = self.free_nodes.map(|p| p.as_ptr() as u64).unwrap_or(0);
        memcpy(
            node_mut.keys[0].key_data.as_mut_ptr(),
            &next_ptr as *const u64 as *const u8,
            core::mem::size_of::<u64>(),
        );

        // 添加到空闲列表头
        self.free_nodes = Some(node_ptr);
    }

    /// 比较两个索引�?
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem,
    ) -> core::cmp::Ordering {
        // 比较键大�?
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }

        // 比较键数�?
        let key_size = item1.key_size as usize;
        for i in 0..key_size {
            if item1.key_data[i] != item2.key_data[i] {
                return item1.key_data[i].cmp(&item2.key_data[i]);
            }
        }

        // 键相等，比较记录ID
        item1.record_id.cmp(&item2.record_id)
    }

    /// 在节点中查找键的位置
    fn find_key_position(
        &self,
        node: &TTreeNode,
        key: &SecondaryIndexItem,
    ) -> (usize, core::cmp::Ordering) {
        let mut pos = 0;
        while pos < node.key_count as usize {
            let cmp = self.compare_items(&node.keys[pos], key);
            match cmp {
                core::cmp::Ordering::Less => pos += 1,
                _ => return (pos, cmp),
            }
        }
        (pos, core::cmp::Ordering::Less)
    }

    /// 插入键到节点
    unsafe fn insert_into_node(&mut self, mut node: NonNull<TTreeNode>, key: SecondaryIndexItem) {
        let node_mut = node.as_mut();

        // 查找插入位置
        let (pos, _) = self.find_key_position(node_mut, &key);

        // 移动现有键为新键腾出空间
        for i in (pos..node_mut.key_count as usize).rev() {
            node_mut.keys[i + 1] = node_mut.keys[i];
        }

        // 插入新键
        node_mut.keys[pos] = key;
        node_mut.key_count += 1;
        self.stats.item_count += 1;
    }

    /// 插入索引�?
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建索引�?
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 128],
        };
        memcpy(new_item.key_data.as_mut_ptr(), key, key_size);

        if self.root.is_none() {
            // 空树，创建根节点
            let mut root_node = self
                .allocate_node()
                .expect("Out of memory for T-Tree root node");
            let root_mut = root_node.as_mut();

            root_mut.keys[0] = new_item;
            root_mut.key_count = 1;
            self.root = Some(root_node);
        } else {
            let mut root = self.root.expect("Root node unexpectedly None");

            // 如果根节点已满，需要分�?
            if root.as_ref().key_count == TTREE_ORDER as u8 {
                // 简化实现：创建新根节点，将原根节点作为左子节点
                let mut new_root = self
                    .allocate_node()
                    .expect("Out of memory for new T-Tree root");
                let new_root_mut = new_root.as_mut();

                // 将新键插入到适当的位�?
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];

                // 复制原根节点的键
                for i in 0..TTREE_ORDER {
                    keys[i] = root.as_ref().keys[i];
                }

                // 插入新键
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if self.compare_items(&keys[i], &new_item) == core::cmp::Ordering::Greater {
                        // 移动后续�?
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = new_item;
                        inserted = true;
                        break;
                    }
                }

                if !inserted {
                    keys[TTREE_ORDER] = new_item;
                }

                // 创建右子节点
                let mut right_node = self
                    .allocate_node()
                    .expect("Out of memory for T-Tree right node");
                let right_mut = right_node.as_mut();

                // 分配键到左右子节�?
                let mid = (TTREE_ORDER + 1) / 2;

                // 更新原根节点（左子节点）
                let root_mut = root.as_mut();
                root_mut.key_count = mid as u8;
                for i in 0..mid {
                    root_mut.keys[i] = keys[i];
                }

                // 更新右子节点
                right_mut.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..right_mut.key_count as usize {
                    right_mut.keys[i] = keys[mid + i];
                }

                // 更新新根节点
                new_root_mut.keys[0] = keys[mid - 1];
                new_root_mut.key_count = 1;
                new_root_mut.left = Some(root);
                new_root_mut.right = Some(right_node);

                self.root = Some(new_root);
            } else {
                // 递归插入到适当的子�?
                self.insert_recursive(root, new_item);
            }
        }

        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }

    /// 递归插入索引�?
    unsafe fn insert_recursive(&mut self, mut node: NonNull<TTreeNode>, key: SecondaryIndexItem) {
        let node_mut = node.as_mut();

        // 查找插入位置
        let (pos, cmp) = self.find_key_position(node_mut, &key);

        if cmp == core::cmp::Ordering::Equal {
            // 键已存在，更新记录ID
            node_mut.keys[pos].record_id = key.record_id;
            return;
        }

        // 确定子树方向
        let child = if pos == 0 {
            &mut node_mut.left
        } else if pos < node_mut.key_count as usize {
            &mut node_mut.middle
        } else {
            &mut node_mut.right
        };

        if let Some(mut child_node) = *child {
            // 子节点存�?
            if child_node.as_ref().key_count == TTREE_ORDER as u8 {
                // 子节点已满，需要分�?
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];

                // 复制子节点的�?
                for i in 0..TTREE_ORDER {
                    keys[i] = child_node.as_ref().keys[i];
                }

                // 插入新键
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if self.compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        // 移动后续�?
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }

                if !inserted {
                    keys[TTREE_ORDER] = key;
                }

                // 创建新的右子节点
                let mut new_right = self
                    .allocate_node()
                    .expect("Out of memory for T-Tree new right node");
                let new_right_mut = new_right.as_mut();

                // 分配键到左右子节�?
                let mid = (TTREE_ORDER + 1) / 2;

                // 更新原子节点（左子节点）
                let child_mut = child_node.as_mut();
                child_mut.key_count = mid as u8;
                for i in 0..mid {
                    child_mut.keys[i] = keys[i];
                }

                // 更新新右子节�?
                new_right_mut.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_right_mut.key_count as usize {
                    new_right_mut.keys[i] = keys[mid + i];
                }

                // 将中间键提升到当前节�?
                let promoted_key = keys[mid - 1];

                // 插入提升的键到当前节�?
                self.insert_into_node(node, promoted_key);

                // 更新子节点指�?
                // 简化实现：根据提升键的位置更新子节�?
                let (promoted_pos, _) = self.find_key_position(node_mut, &promoted_key);

                if promoted_pos == 0 {
                    node_mut.left = Some(child_node);
                    node_mut.middle = Some(new_right);
                } else if promoted_pos < node_mut.key_count as usize {
                    node_mut.middle = Some(child_node);
                    node_mut.right = Some(new_right);
                } else {
                    node_mut.right = Some(new_right);
                }
            } else {
                // 子节点未满，递归插入
                self.insert_recursive(child_node, key);
            }
        } else {
            // 子节点不存在，直接插入到当前节点
            if node_mut.key_count < TTREE_ORDER as u8 {
                self.insert_into_node(node, key);
            } else {
                // 节点已满，需要分�?
                // 简化实现：创建新节�?
                let mut new_node = self
                    .allocate_node()
                    .expect("Out of memory for T-Tree new node");
                let new_node_mut = new_node.as_mut();

                // 将当前节点的键和新键合并
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];

                // 复制当前节点的键
                for i in 0..TTREE_ORDER {
                    keys[i] = node_mut.keys[i];
                }

                // 插入新键
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if self.compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        // 移动后续�?
                        for j in (i..TTREE_ORDER).rev() {
                            keys[j + 1] = keys[j];
                        }
                        keys[i] = key;
                        inserted = true;
                        break;
                    }
                }

                if !inserted {
                    keys[TTREE_ORDER] = key;
                }

                // 分配键到当前节点和新节点
                let mid = (TTREE_ORDER + 1) / 2;

                // 更新当前节点
                node_mut.key_count = mid as u8;
                for i in 0..mid {
                    node_mut.keys[i] = keys[i];
                }

                // 更新新节�?
                new_node_mut.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_node_mut.key_count as usize {
                    new_node_mut.keys[i] = keys[mid + i];
                }

                // 更新子节点指�?
                if pos == 0 {
                    node_mut.left = Some(new_node);
                } else if pos < node_mut.key_count as usize {
                    node_mut.middle = Some(new_node);
                } else {
                    node_mut.right = Some(new_node);
                }
            }
        }
    }

    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 检查键大小
        if key_size > 64 {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建临时索引项用于比�?
        let mut search_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(search_item.key_data.as_mut_ptr(), key, key_size);

        let mut current = self.root;
        while let Some(node) = current {
            let node_ref = node.as_ref();

            // 查找键位�?
            let (pos, cmp) = self.find_key_position(node_ref, &search_item);

            if cmp == core::cmp::Ordering::Equal {
                // 更新命中统计
                self.stats.hit_count += 1;
                return Ok(node_ref.keys[pos].record_id);
            }

            // 确定下一个子节点
            current = if pos == 0 {
                node_ref.left
            } else if pos < node_ref.key_count as usize {
                node_ref.middle
            } else {
                node_ref.right
            };
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 范围查询（返回第一个匹配项�?
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 创建临时索引项用于比�?
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);

        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(end_item.key_data.as_mut_ptr(), end_key, end_key_size);

        // 简化实现：遍历树，查找第一个匹配项
        let mut stack = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        let mut current = self.root;

        // 遍历到最左节�?
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;
            current = node.as_ref().left;
        }

        // 中序遍历树，查找第一个匹配项
        while stack_size > 0 {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            let node_ref = node.as_ref();

            // 检查当前节点的�?
            for i in 0..node_ref.key_count as usize {
                let key = &node_ref.keys[i];

                // 检查是否在范围�?
                if self.compare_items(key, &start_item) != core::cmp::Ordering::Less
                    && self.compare_items(key, &end_item) != core::cmp::Ordering::Greater
                {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(key.record_id);
                }

                // 如果已经超出范围，结束搜�?
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    break;
                }
            }

            // 遍历右子�?
            let mut child = node_ref.right;
            while let Some(child_node) = child {
                stack[stack_size] = Some(child_node);
                stack_size += 1;
                child = child_node.as_ref().left;
            }
        }

        Err(RemDbError::RecordNotFound)
    }

    /// 范围查询（返回所有匹配项�?
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize,
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;

        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }

        // 创建临时索引项用于比�?
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);

        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 128],
        };
        memcpy(end_item.key_data.as_mut_ptr(), end_key, end_key_size);

        let mut match_count = 0;
        let mut stack = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        let mut current = self.root;

        // 遍历到最左节�?
        while let Some(node) = current {
            stack[stack_size] = Some(node);
            stack_size += 1;
            current = node.as_ref().left;
        }

        // 中序遍历树，收集所有匹配项
        while stack_size > 0 && match_count < max_records {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            let node_ref = node.as_ref();

            // 检查当前节点的�?
            for i in 0..node_ref.key_count as usize {
                if match_count >= max_records {
                    break;
                }

                let key = &node_ref.keys[i];

                // 检查是否在范围�?
                if self.compare_items(key, &start_item) != core::cmp::Ordering::Less
                    && self.compare_items(key, &end_item) != core::cmp::Ordering::Greater
                {
                    // 添加到结�?
                    *out_record_ids.add(match_count) = key.record_id;
                    match_count += 1;
                }

                // 如果已经超出范围，结束搜�?
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    break;
                }
            }

            // 遍历右子�?
            let mut child = node_ref.right;
            while let Some(child_node) = child {
                stack[stack_size] = Some(child_node);
                stack_size += 1;
                child = child_node.as_ref().left;
            }
        }

        // 更新命中统计
        if match_count > 0 {
            self.stats.hit_count += match_count;
        }

        Ok(match_count)
    }

    /// 删除索引�?
    pub unsafe fn delete(&mut self, _key: *const u8, _key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保�?
        crate::platform::spin_lock(&mut self.lock);

        // 简化实现：暂不支持删除操作
        // 完整的T-Tree删除实现比较复杂，需要处理多种情�?

        crate::platform::spin_unlock(&mut self.lock);
        Err(RemDbError::UnsupportedOperation)
    }

    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        self.stats.access_count = 0;
        self.stats.hit_count = 0;
    }
}
