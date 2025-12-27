use core::ptr::NonNull;
use crate::types::{TableDef, Result, RemDbError, IndexType};
use crate::platform::{memcpy, memset};

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
    pub key_data: [u8; 64], // 最大键大小64字节
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
    pub key_data: [u8; 64], // 最大键大小64字节
}

impl Default for SecondaryIndexItem {
    fn default() -> Self {
        SecondaryIndexItem {
            key_size: 0,
            record_id: 0,
            key_data: [0u8; 64],
        }
    }
}

/// B-Tree节点结构
#[repr(C)]
pub struct BTreeNode {
    /// 节点类型（内部节点/叶子节点）
    pub is_leaf: bool,
    /// 当前键数量
    pub key_count: u8,
    /// 键数据（每个键64字节）
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
    /// 键数据（每个键64字节）
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
    /// 哈希表数组
    hash_table: NonNull<Option<NonNull<PrimaryIndexItem>>>,
    /// 哈希表大小
    hash_table_size: usize,
    /// 索引项数组
    items: NonNull<PrimaryIndexItem>,
    /// 可用索引项指针
    free_items: Option<NonNull<PrimaryIndexItem>>,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: u32,
}

impl PrimaryIndex {
    /// 创建新的主键索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        hash_table_start: *mut Option<NonNull<PrimaryIndexItem>>,
        items_start: *mut PrimaryIndexItem,
        hash_table_size: usize,
        max_items: usize
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
            memset((*item_ptr).key_data.as_mut_ptr(), 0, 64);
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
                size: hash_table_size * core::mem::size_of::<Option<NonNull<PrimaryIndexItem>>>() + 
                      max_items * core::mem::size_of::<PrimaryIndexItem>(),
                item_count: 0,
            },
            lock: 0,
        }
    }
    
    /// 计算主键索引所需的内存大小
    pub const fn calculate_memory_size(
        _def: &TableDef,
        hash_table_size: usize,
        max_items: usize
    ) -> usize {
        let hash_table_size_bytes = hash_table_size * core::mem::size_of::<Option<NonNull<PrimaryIndexItem>>>();
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
        if key_size > 64 {
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
            },
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::OutOfMemory);
            }
        };
        
        // 设置索引项
        let item_mut = item.as_mut();
        item_mut.record_id = record_id;
        item_mut.key_size = key_size as u8;
        memcpy(item_mut.key_data.as_mut_ptr(), key, key_size);
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);
        
        // 插入到哈希表槽位的头部
        item_mut.next = *slot_ptr;
        *slot_ptr = Some(item);
        
        // 更新统计信息
        self.stats.item_count += 1;
        
        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }
    
    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 计算哈希值
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
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 计算哈希值
        let hash = self.hash_key(key, key_size);
        let slot_ptr = self.hash_table.as_ptr().add(hash);
        
        // 遍历链表查找并删除
        let mut current = *slot_ptr;
        let mut prev: Option<NonNull<PrimaryIndexItem>> = None;
        
        while let Some(mut item) = current {
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
                    // 从链表中移除
                    if let Some(mut prev_item) = prev {
                        prev_item.as_mut().next = item_ref.next;
                    } else {
                        *slot_ptr = item_ref.next;
                    }
                    
                    // 归还到空闲列表
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

/// 辅助索引枚举（用于封装不同类型的辅助索引）
pub enum AnySecondaryIndex {
    /// 有序数组索引
    SortedArray(SecondaryIndex),
    /// B-Tree索引
    BTree(BTreeIndex),
    /// T-Tree索引
    TTree(TTreeIndex),
}

impl AnySecondaryIndex {
    /// 创建新的辅助索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        memory_start: *mut u8,
        max_items: usize
    ) -> Result<Self> {
        match def.secondary_index_type {
            IndexType::SortedArray => {
                // 创建有序数组索引
                let index = SecondaryIndex::new(def, memory_start as *mut SecondaryIndexItem, max_items);
                Ok(AnySecondaryIndex::SortedArray(index))
            },
            IndexType::BTree => {
                // 创建B-Tree索引
                let index = BTreeIndex::new(def, memory_start as *mut BTreeNode, max_items);
                Ok(AnySecondaryIndex::BTree(index))
            },
            IndexType::TTree => {
                // 创建T-Tree索引
                let index = TTreeIndex::new(def, memory_start as *mut TTreeNode, max_items);
                Ok(AnySecondaryIndex::TTree(index))
            },
            _ => {
                Err(RemDbError::UnsupportedOperation)
            }
        }
    }
    
    /// 计算辅助索引所需的内存大小
    pub const fn calculate_memory_size(def: &TableDef, max_items: usize) -> usize {
        match def.secondary_index_type {
            IndexType::SortedArray => {
                SecondaryIndex::calculate_memory_size(max_items)
            },
            IndexType::BTree => {
                BTreeIndex::calculate_memory_size(max_items)
            },
            IndexType::TTree => {
                TTreeIndex::calculate_memory_size(max_items)
            },
            _ => {
                0
            }
        }
    }
    
    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::BTree(index) => index.insert(key, key_size, record_id),
            AnySecondaryIndex::TTree(index) => index.insert(key, key_size, record_id),
        }
    }
    
    /// 根据键查找记录ID
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find(key, key_size),
            AnySecondaryIndex::BTree(index) => index.find(key, key_size),
            AnySecondaryIndex::TTree(index) => index.find(key, key_size),
        }
    }
    
    /// 范围查询（返回第一个匹配项）
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find_range(start_key, start_key_size, end_key, end_key_size),
            AnySecondaryIndex::BTree(index) => index.find_range(start_key, start_key_size, end_key, end_key_size),
            AnySecondaryIndex::TTree(index) => index.find_range(start_key, start_key_size, end_key, end_key_size),
        }
    }
    
    /// 范围查询（返回所有匹配项）
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.find_range_all(start_key, start_key_size, end_key, end_key_size, out_record_ids, max_records),
            AnySecondaryIndex::BTree(index) => index.find_range_all(start_key, start_key_size, end_key, end_key_size, out_record_ids, max_records),
            AnySecondaryIndex::TTree(index) => index.find_range_all(start_key, start_key_size, end_key, end_key_size, out_record_ids, max_records),
        }
    }
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.delete(key, key_size),
            AnySecondaryIndex::BTree(index) => index.delete(key, key_size),
            AnySecondaryIndex::TTree(index) => index.delete(key, key_size),
        }
    }
    
    /// 获取索引统计信息
    pub fn stats(&self) -> &IndexStats {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.stats(),
            AnySecondaryIndex::BTree(index) => index.stats(),
            AnySecondaryIndex::TTree(index) => index.stats(),
        }
    }
    
    /// 重置索引统计信息
    pub fn reset_stats(&mut self) {
        match self {
            AnySecondaryIndex::SortedArray(index) => index.reset_stats(),
            AnySecondaryIndex::BTree(index) => index.reset_stats(),
            AnySecondaryIndex::TTree(index) => index.reset_stats(),
        }
    }
}

/// 辅助有序索引
pub struct SecondaryIndex {
    /// 表定义
    def: alloc::sync::Arc<TableDef>,
    /// 索引项数组
    items: NonNull<SecondaryIndexItem>,
    /// 当前项数量
    item_count: usize,
    /// 最大项数量
    max_items: usize,
    /// 索引统计信息
    stats: IndexStats,
    /// 自旋锁
    lock: u32,
}

impl SecondaryIndex {
    /// 创建新的辅助索引
    pub unsafe fn new(
        def: alloc::sync::Arc<TableDef>,
        items_start: *mut SecondaryIndexItem,
        max_items: usize
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
    
    /// 计算辅助索引所需的内存大小
    pub const fn calculate_memory_size(max_items: usize) -> usize {
        max_items * core::mem::size_of::<SecondaryIndexItem>()
    }
    
    /// 比较两个索引项
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem
    ) -> core::cmp::Ordering {
        // 比较键大小
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }
        
        // 比较键数据
        let key_size = item1.key_size as usize;
        for i in 0..key_size {
            if item1.key_data[i] != item2.key_data[i] {
                return item1.key_data[i].cmp(&item2.key_data[i]);
            }
        }
        
        // 键相等，比较记录ID
        item1.record_id.cmp(&item2.record_id)
    }
    
    /// 二分查找索引项
    fn binary_search(
        &self,
        key: *const u8,
        key_size: usize
    ) -> Result<usize> {
        if self.item_count == 0 {
            return Err(RemDbError::RecordNotFound);
        }
        
        let mut low = 0;
        let mut high = self.item_count - 1;
        
        while low <= high {
            let mid = (low + high) / 2;
            let mid_item = unsafe { &*self.items.as_ptr().add(mid) };
            
            // 比较键
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
    
    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()>
    {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查是否已满
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
            key_data: [0u8; 64],
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
                        // 插入到相等元素后面
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
    pub unsafe fn find(&mut self, key: *const u8, key_size: usize) -> Result<u16>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        match self.binary_search(key, key_size) {
            Ok(index) => {
                // 更新命中统计
                self.stats.hit_count += 1;
                Ok((*self.items.as_ptr().add(index)).record_id)
            },
            Err(e) => Err(e),
        }
    }
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()>
    {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        let result = match self.binary_search(key, key_size) {
            Ok(index) => {
                // 移动后续项覆盖被删除项
                if index < self.item_count - 1 {
                    let dest_ptr = self.items.as_ptr().add(index);
                    let src_ptr = self.items.as_ptr().add(index + 1);
                    let move_size = (self.item_count - index - 1) * core::mem::size_of::<SecondaryIndexItem>();
                    memcpy(dest_ptr as *mut u8, src_ptr as *const u8, move_size);
                }
                
                // 清空最后一项
                let last_ptr = self.items.as_ptr().add(self.item_count - 1);
                memset(last_ptr as *mut u8, 0, core::mem::size_of::<SecondaryIndexItem>());
                
                // 更新统计信息
                self.item_count -= 1;
                self.stats.item_count = self.item_count;
                
                Ok(())
            },
            Err(e) => Err(e),
        };
        
        crate::platform::spin_unlock(&mut self.lock);
        result
    }
    
    /// 范围查询（返回第一个匹配项）
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.item_count - 1;
        
        // 创建临时索引项用于比较
        let start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_ptr() as *mut u8, start_key, start_key_size);
        
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
                // 键大小大于end_key，超出范围
                break;
            }
            
            // 比较键数据
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
    
    /// 范围查询（返回所有匹配项）
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize>
    {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区是否为null
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 如果没有记录，直接返回
        if self.item_count == 0 {
            return Ok(0);
        }
        
        // 使用二分查找找到起始位置，优化范围查询性能
        let mut start_pos = 0;
        let mut low = 0;
        let mut high = self.item_count - 1;
        
        // 创建临时索引项用于比较
        let start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_ptr() as *mut u8, start_key, start_key_size);
        
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
                // 键大小大于end_key，超出范围
                break;
            }
            
            // 比较键数据
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
    
    /// 获取当前项数量
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
        max_nodes: usize
    ) -> Self {
        // 初始化节点池
        let nodes = NonNull::new_unchecked(nodes_start);
        let mut free_nodes: Option<NonNull<BTreeNode>> = None;
        
        // 将所有节点链接到空闲列表
        for i in (0..max_nodes).rev() {
            let node_ptr = nodes.as_ptr().add(i);
            let mut node_mut = &mut *node_ptr;
            
            // 初始化节点
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
            
            // 添加到空闲列表
            // 使用第一个键的key_data字段作为下一个节点的指针
            // 由于key_data是64字节，足够存储一个指针
            let next_ptr = free_nodes.map(|p: NonNull<BTreeNode>| p.as_ptr() as u64).unwrap_or(0);
            memcpy(node_mut.keys[0].key_data.as_mut_ptr(), &next_ptr as *const u64 as *const u8, core::mem::size_of::<u64>());
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
    
    /// 计算B-Tree索引所需的内存大小
    pub const fn calculate_memory_size(max_nodes: usize) -> usize {
        max_nodes * core::mem::size_of::<BTreeNode>()
    }
    
    /// 从空闲列表获取一个节点
    unsafe fn allocate_node(&mut self) -> Option<NonNull<BTreeNode>> {
        let node_ptr = self.free_nodes?;
        let mut node_mut = &mut *node_ptr.as_ptr();
        
        // 从节点的key_data字段获取下一个空闲节点的指针
        let mut next_ptr = 0u64;
        memcpy(&mut next_ptr as *mut u64 as *mut u8, node_mut.keys[0].key_data.as_ptr(), core::mem::size_of::<u64>());
        
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
    
    /// 释放节点到空闲列表
    unsafe fn free_node(&mut self, node_ptr: NonNull<BTreeNode>) {
        let node_mut = &mut *node_ptr.as_ptr();
        
        // 将当前空闲列表头指针存储到节点的key_data字段
        let next_ptr = self.free_nodes.map(|p| p.as_ptr() as u64).unwrap_or(0);
        memcpy(node_mut.keys[0].key_data.as_mut_ptr(), &next_ptr as *const u64 as *const u8, core::mem::size_of::<u64>());
        
        // 添加到空闲列表头
        self.free_nodes = Some(node_ptr);
    }
    
    /// 比较两个索引项
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem
    ) -> core::cmp::Ordering {
        // 比较键大小
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }
        
        // 比较键数据
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
    fn find_key_position(
        &self,
        node: &BTreeNode,
        key: &SecondaryIndexItem
    ) -> usize {
        let mut pos = 0;
        while pos < node.key_count as usize && self.compare_items(&node.keys[pos], key) == core::cmp::Ordering::Less {
            pos += 1;
        }
        pos
    }
    
    /// 分割满节点
    unsafe fn split_child(
        &mut self,
        mut parent: NonNull<BTreeNode>,
        child_idx: usize,
        mut child: NonNull<BTreeNode>
    ) {
        let parent_mut = parent.as_mut();
        let child_mut = child.as_mut();
        
        // 创建新节点
        let mut new_node = self.allocate_node().expect("Out of memory for B-Tree node");
        let new_node_mut = new_node.as_mut();
        
        new_node_mut.is_leaf = child_mut.is_leaf;
        new_node_mut.key_count = (BTREE_ORDER / 2) as u8;
        
        // 复制后半部分键到新节点
        for i in 0..(BTREE_ORDER / 2) {
            new_node_mut.keys[i] = child_mut.keys[i + (BTREE_ORDER / 2) + 1];
        }
        
        // 如果是内部节点，复制后半部分子节点
        if !child_mut.is_leaf {
            for i in 0..(BTREE_ORDER / 2 + 1) {
                new_node_mut.children[i] = child_mut.children[i + (BTREE_ORDER / 2) + 1];
            }
        }
        
        // 更新原节点的键数量
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
    unsafe fn insert_non_full(
        &mut self,
        mut node: NonNull<BTreeNode>,
        key: SecondaryIndexItem
    ) {
        let node_mut = node.as_mut();
        let mut pos = self.find_key_position(node_mut, &key);
        
        if node_mut.is_leaf {
            // 叶子节点，直接插入
            for i in (pos..node_mut.key_count as usize).rev() {
                node_mut.keys[i + 1] = node_mut.keys[i];
            }
            node_mut.keys[pos] = key;
            node_mut.key_count += 1;
            self.stats.item_count += 1;
        } else {
            // 内部节点，递归插入
            let child = node_mut.children[pos].expect("Child node not found");
            
            // 如果子节点已满，先分割
            if child.as_ref().key_count == BTREE_ORDER as u8 {
                self.split_child(node, pos, child);
                
                // 检查中间键是否大于当前键
                if self.compare_items(&node_mut.keys[pos], &key) == core::cmp::Ordering::Less {
                    pos += 1;
                }
            }
            
            self.insert_non_full(node_mut.children[pos].expect("Child node not found after split"), key);
        }
    }
    
    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建索引项
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 64],
        };
        memcpy(new_item.key_data.as_mut_ptr(), key, key_size);
        
        if self.root.is_none() {
            // 空树，创建根节点
            let mut root_node = self.allocate_node().expect("Out of memory for B-Tree root node");
            let root_mut = root_node.as_mut();
            
            root_mut.keys[0] = new_item;
            root_mut.key_count = 1;
            self.root = Some(root_node);
        } else {
            let mut root = self.root.expect("Root node unexpectedly None");
            
            // 如果根节点已满，分裂根节点
            if root.as_ref().key_count == BTREE_ORDER as u8 {
                let mut new_root = self.allocate_node().expect("Out of memory for new B-Tree root");
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
        
        // 创建临时索引项用于比较
        let mut search_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(search_item.key_data.as_mut_ptr(), key, key_size);
        
        let mut current = self.root;
        while let Some(node) = current {
            let node_ref = node.as_ref();
            let mut pos = 0;
            
            // 查找键位置
            while pos < node_ref.key_count as usize && self.compare_items(&node_ref.keys[pos], &search_item) == core::cmp::Ordering::Less {
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
            
            // 如果是叶子节点，未找到
            if node_ref.is_leaf {
                break;
            }
            
            // 继续搜索子节点
            current = node_ref.children[pos];
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回第一个匹配项）
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 实现范围查询逻辑
        // 简化实现：找到起始位置后，遍历直到找到第一个匹配项
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
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
            while pos < node_ref.key_count as usize && self.compare_items(&node_ref.keys[pos], &start_item) == core::cmp::Ordering::Less {
                pos += 1;
            }
            
            current = node_ref.children[pos];
        }
        
        // 从栈中回溯，查找匹配项
        while stack_size > 0 {
            stack_size -= 1;
            let node = stack[stack_size].expect("Stack underflow");
            let node_ref = node.as_ref();
            
            // 查找起始位置
            let mut start_pos = 0;
            while start_pos < node_ref.key_count as usize && self.compare_items(&node_ref.keys[start_pos], &start_item) == core::cmp::Ordering::Less {
                start_pos += 1;
            }
            
            // 遍历当前节点的键
            for i in start_pos..node_ref.key_count as usize {
                let key = &node_ref.keys[i];
                
                // 检查是否在范围内
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    continue; // 超出范围，继续查找下一个节点
                }
                
                // 找到匹配项
                self.stats.hit_count += 1;
                return Ok(key.record_id);
            }
            
            // 如果不是叶子节点，继续搜索右子树
            if !node_ref.is_leaf {
                let mut child = node_ref.children[node_ref.key_count as usize];
                while let Some(child_node) = child {
                    let child_ref = child_node.as_ref();
                    
                    // 遍历子节点的键
                    for i in 0..child_ref.key_count as usize {
                        let key = &child_ref.keys[i];
                        
                        // 检查是否在范围内
                        if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                            break; // 超出范围，结束搜索
                        }
                        
                        // 找到匹配项
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
    
    /// 范围查询（返回所有匹配项）
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
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
            while pos < node_ref.key_count as usize && self.compare_items(&node_ref.keys[pos], &start_item) == core::cmp::Ordering::Less {
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
            while start_pos < node_ref.key_count as usize && self.compare_items(&node_ref.keys[start_pos], &start_item) == core::cmp::Ordering::Less {
                start_pos += 1;
            }
            
            // 遍历当前节点的键
            for i in start_pos..node_ref.key_count as usize {
                if match_count >= max_records {
                    break;
                }
                
                let key = &node_ref.keys[i];
                
                // 检查是否在范围内
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    continue; // 超出范围，继续查找下一个节点
                }
                
                // 添加到结果
                *out_record_ids.add(match_count) = key.record_id;
                match_count += 1;
            }
            
            // 如果不是叶子节点，继续搜索右子树
            if !node_ref.is_leaf && match_count < max_records {
                let mut child = node_ref.children[node_ref.key_count as usize];
                while let Some(child_node) = child {
                    let child_ref = child_node.as_ref();
                    
                    // 遍历子节点的键
                    for i in 0..child_ref.key_count as usize {
                        if match_count >= max_records {
                            break;
                        }
                        
                        let key = &child_ref.keys[i];
                        
                        // 检查是否在范围内
                        if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                            break; // 超出范围，结束搜索
                        }
                        
                        // 添加到结果
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
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 简化实现：暂不支持删除操作
        // 完整的B-Tree删除实现比较复杂，需要处理多种情况
        // 包括合并节点、借键等
        
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
        max_nodes: usize
    ) -> Self {
        // 初始化节点池
        let nodes = NonNull::new_unchecked(nodes_start);
        let mut free_nodes: Option<NonNull<TTreeNode>> = None;
        
        // 将所有节点链接到空闲列表
        for i in (0..max_nodes).rev() {
            let node_ptr = nodes.as_ptr().add(i);
            let mut node_mut = &mut *node_ptr;
            
            // 初始化节点
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
            
            // 添加到空闲列表
            // 使用第一个键的key_data字段作为下一个节点的指针
            let next_ptr = free_nodes.map(|p: NonNull<TTreeNode>| p.as_ptr() as u64).unwrap_or(0);
            memcpy(node_mut.keys[0].key_data.as_mut_ptr(), &next_ptr as *const u64 as *const u8, core::mem::size_of::<u64>());
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
    
    /// 计算T-Tree索引所需的内存大小
    pub const fn calculate_memory_size(max_nodes: usize) -> usize {
        max_nodes * core::mem::size_of::<TTreeNode>()
    }
    
    /// 从空闲列表获取一个节点
    unsafe fn allocate_node(&mut self) -> Option<NonNull<TTreeNode>> {
        let node_ptr = self.free_nodes?;
        let mut node_mut = &mut *node_ptr.as_ptr();
        
        // 从节点的key_data字段获取下一个空闲节点的指针
        let mut next_ptr = 0u64;
        memcpy(&mut next_ptr as *mut u64 as *mut u8, node_mut.keys[0].key_data.as_ptr(), core::mem::size_of::<u64>());
        
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
    
    /// 释放节点到空闲列表
    unsafe fn free_node(&mut self, node_ptr: NonNull<TTreeNode>) {
        let node_mut = &mut *node_ptr.as_ptr();
        
        // 将当前空闲列表头指针存储到节点的key_data字段
        let next_ptr = self.free_nodes.map(|p| p.as_ptr() as u64).unwrap_or(0);
        memcpy(node_mut.keys[0].key_data.as_mut_ptr(), &next_ptr as *const u64 as *const u8, core::mem::size_of::<u64>());
        
        // 添加到空闲列表头
        self.free_nodes = Some(node_ptr);
    }
    
    /// 比较两个索引项
    fn compare_items(
        &self,
        item1: &SecondaryIndexItem,
        item2: &SecondaryIndexItem
    ) -> core::cmp::Ordering {
        // 比较键大小
        if item1.key_size != item2.key_size {
            return item1.key_size.cmp(&item2.key_size);
        }
        
        // 比较键数据
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
        key: &SecondaryIndexItem
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
    unsafe fn insert_into_node(
        &mut self,
        mut node: NonNull<TTreeNode>,
        key: SecondaryIndexItem
    ) {
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
    
    /// 插入索引项
    pub unsafe fn insert(&mut self, key: *const u8, key_size: usize, record_id: u16) -> Result<()> {
        // 增加索引插入计数
        crate::get_global_db().map(|db| db.metrics.inc_index_inserts());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查键大小
        if key_size > 64 {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建索引项
        let mut new_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id,
            key_data: [0u8; 64],
        };
        memcpy(new_item.key_data.as_mut_ptr(), key, key_size);
        
        if self.root.is_none() {
            // 空树，创建根节点
            let mut root_node = self.allocate_node().expect("Out of memory for T-Tree root node");
            let root_mut = root_node.as_mut();
            
            root_mut.keys[0] = new_item;
            root_mut.key_count = 1;
            self.root = Some(root_node);
        } else {
            let mut root = self.root.expect("Root node unexpectedly None");
            
            // 如果根节点已满，需要分裂
            if root.as_ref().key_count == TTREE_ORDER as u8 {
                // 简化实现：创建新根节点，将原根节点作为左子节点
                let mut new_root = self.allocate_node().expect("Out of memory for new T-Tree root");
                let new_root_mut = new_root.as_mut();
                
                // 将新键插入到适当的位置
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                
                // 复制原根节点的键
                for i in 0..TTREE_ORDER {
                    keys[i] = root.as_ref().keys[i];
                }
                
                // 插入新键
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if self.compare_items(&keys[i], &new_item) == core::cmp::Ordering::Greater {
                        // 移动后续键
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
                let mut right_node = self.allocate_node().expect("Out of memory for T-Tree right node");
                let right_mut = right_node.as_mut();
                
                // 分配键到左右子节点
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
                // 递归插入到适当的子树
                self.insert_recursive(root, new_item);
            }
        }
        
        crate::platform::spin_unlock(&mut self.lock);
        Ok(())
    }
    
    /// 递归插入索引项
    unsafe fn insert_recursive(
        &mut self,
        mut node: NonNull<TTreeNode>,
        key: SecondaryIndexItem
    ) {
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
            // 子节点存在
            if child_node.as_ref().key_count == TTREE_ORDER as u8 {
                // 子节点已满，需要分裂
                let mut keys = [SecondaryIndexItem::default(); TTREE_ORDER + 1];
                
                // 复制子节点的键
                for i in 0..TTREE_ORDER {
                    keys[i] = child_node.as_ref().keys[i];
                }
                
                // 插入新键
                let mut inserted = false;
                for i in 0..TTREE_ORDER {
                    if self.compare_items(&keys[i], &key) == core::cmp::Ordering::Greater {
                        // 移动后续键
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
                let mut new_right = self.allocate_node().expect("Out of memory for T-Tree new right node");
                let new_right_mut = new_right.as_mut();
                
                // 分配键到左右子节点
                let mid = (TTREE_ORDER + 1) / 2;
                
                // 更新原子节点（左子节点）
                let child_mut = child_node.as_mut();
                child_mut.key_count = mid as u8;
                for i in 0..mid {
                    child_mut.keys[i] = keys[i];
                }
                
                // 更新新右子节点
                new_right_mut.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_right_mut.key_count as usize {
                    new_right_mut.keys[i] = keys[mid + i];
                }
                
                // 将中间键提升到当前节点
                let promoted_key = keys[mid - 1];
                
                // 插入提升的键到当前节点
                self.insert_into_node(node, promoted_key);
                
                // 更新子节点指针
                // 简化实现：根据提升键的位置更新子节点
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
                // 节点已满，需要分裂
                // 简化实现：创建新节点
                let mut new_node = self.allocate_node().expect("Out of memory for T-Tree new node");
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
                        // 移动后续键
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
                
                // 更新新节点
                new_node_mut.key_count = ((TTREE_ORDER + 1) - mid) as u8;
                for i in 0..new_node_mut.key_count as usize {
                    new_node_mut.keys[i] = keys[mid + i];
                }
                
                // 更新子节点指针
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
        
        // 创建临时索引项用于比较
        let mut search_item = SecondaryIndexItem {
            key_size: key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(search_item.key_data.as_mut_ptr(), key, key_size);
        
        let mut current = self.root;
        while let Some(node) = current {
            let node_ref = node.as_ref();
            
            // 查找键位置
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
    
    /// 范围查询（返回第一个匹配项）
    pub unsafe fn find_range(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize
    ) -> Result<u16> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(end_item.key_data.as_mut_ptr(), end_key, end_key_size);
        
        // 简化实现：遍历树，查找第一个匹配项
        let mut stack = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        let mut current = self.root;
        
        // 遍历到最左节点
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
            
            // 检查当前节点的键
            for i in 0..node_ref.key_count as usize {
                let key = &node_ref.keys[i];
                
                // 检查是否在范围内
                if self.compare_items(key, &start_item) != core::cmp::Ordering::Less && 
                   self.compare_items(key, &end_item) != core::cmp::Ordering::Greater {
                    // 更新命中统计
                    self.stats.hit_count += 1;
                    return Ok(key.record_id);
                }
                
                // 如果已经超出范围，结束搜索
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    break;
                }
            }
            
            // 遍历右子树
            let mut child = node_ref.right;
            while let Some(child_node) = child {
                stack[stack_size] = Some(child_node);
                stack_size += 1;
                child = child_node.as_ref().left;
            }
        }
        
        Err(RemDbError::RecordNotFound)
    }
    
    /// 范围查询（返回所有匹配项）
    pub unsafe fn find_range_all(
        &mut self,
        start_key: *const u8,
        start_key_size: usize,
        end_key: *const u8,
        end_key_size: usize,
        out_record_ids: *mut u16,
        max_records: usize
    ) -> Result<usize> {
        // 更新统计信息
        self.stats.access_count += 1;
        
        // 检查输出缓冲区
        if out_record_ids.is_null() {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 创建临时索引项用于比较
        let mut start_item = SecondaryIndexItem {
            key_size: start_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(start_item.key_data.as_mut_ptr(), start_key, start_key_size);
        
        let mut end_item = SecondaryIndexItem {
            key_size: end_key_size as u8,
            record_id: 0,
            key_data: [0u8; 64],
        };
        memcpy(end_item.key_data.as_mut_ptr(), end_key, end_key_size);
        
        let mut match_count = 0;
        let mut stack = [None; 64]; // 简化实现：固定大小的栈
        let mut stack_size = 0;
        let mut current = self.root;
        
        // 遍历到最左节点
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
            
            // 检查当前节点的键
            for i in 0..node_ref.key_count as usize {
                if match_count >= max_records {
                    break;
                }
                
                let key = &node_ref.keys[i];
                
                // 检查是否在范围内
                if self.compare_items(key, &start_item) != core::cmp::Ordering::Less && 
                   self.compare_items(key, &end_item) != core::cmp::Ordering::Greater {
                    // 添加到结果
                    *out_record_ids.add(match_count) = key.record_id;
                    match_count += 1;
                }
                
                // 如果已经超出范围，结束搜索
                if self.compare_items(key, &end_item) == core::cmp::Ordering::Greater {
                    break;
                }
            }
            
            // 遍历右子树
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
    
    /// 删除索引项
    pub unsafe fn delete(&mut self, key: *const u8, key_size: usize) -> Result<()> {
        // 增加索引删除计数
        crate::get_global_db().map(|db| db.metrics.inc_index_deletes());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 简化实现：暂不支持删除操作
        // 完整的T-Tree删除实现比较复杂，需要处理多种情况
        
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
