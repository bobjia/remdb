//! JSON专用内存池
//! 
//! 该模块实现了JSON数据的专用内存池，使用slab分配器管理内存，
//! 减少内存碎片，提高内存使用效率。

use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

/// JSON内存池配置
pub struct JsonPoolConfig {
    /// 块大小（字节）
    pub block_size: usize,
    /// 初始块数量
    pub initial_blocks: usize,
    /// 最大块数量
    pub max_blocks: usize,
}

impl Default for JsonPoolConfig {
    fn default() -> Self {
        Self {
            block_size: 4096, // 4KB/块
            initial_blocks: 16,
            max_blocks: 1024,
        }
    }
}

/// 内存块头
#[repr(C)]
pub struct BlockHeader {
    /// 块状态：0=空闲，1=使用中，2=已分配
    pub status: u8,
    /// 引用计数
    pub ref_count: AtomicUsize,
    /// 下一个空闲块的索引
    pub next_free: usize,
    /// 块大小（字节）
    pub size: usize,
}

/// JSON专用内存池
pub struct JsonMemoryPool {
    /// 内存池ID
    pool_id: u8,
    /// 配置
    config: JsonPoolConfig,
    /// 总内存使用
    total_used: AtomicUsize,
    /// 空闲块数量
    free_blocks: AtomicUsize,
    /// 首个空闲块索引
    first_free: AtomicUsize,
    /// 块数组
    blocks: alloc::vec::Vec<Option<NonNull<u8>>>,
}

impl JsonMemoryPool {
    /// 创建新的JSON内存池
    pub fn new(pool_id: u8, config: JsonPoolConfig) -> Self {
        let initial_blocks = config.initial_blocks;
        let mut pool = Self {
            pool_id,
            config,
            total_used: AtomicUsize::new(0),
            free_blocks: AtomicUsize::new(initial_blocks),
            first_free: AtomicUsize::new(0),
            blocks: alloc::vec::Vec::with_capacity(initial_blocks),
        };
        
        // 初始化内存块
        for i in 0..pool.config.initial_blocks {
            let block = pool.allocate_block();
            if let Some(block) = block {
                pool.blocks.push(Some(block));
                
                // 初始化块头
                let header = unsafe { &mut *(block.as_ptr() as *mut BlockHeader) };
                header.status = 0;
                header.ref_count = AtomicUsize::new(0);
                header.next_free = i + 1;
                header.size = pool.config.block_size - core::mem::size_of::<BlockHeader>();
            } else {
                pool.blocks.push(None);
            }
        }
        
        // 最后一个块的next_free设为-1
        if !pool.blocks.is_empty() {
            let last_idx = pool.blocks.len() - 1;
            if let Some(block) = pool.blocks[last_idx] {
                let header = unsafe { &mut *(block.as_ptr() as *mut BlockHeader) };
                header.next_free = usize::MAX;
            }
        }
        
        pool
    }
    
    /// 获取块数据指针
    pub fn get_block_data(&self, block_idx: usize, offset: usize) -> Option<*const u8> {
        if let Some(block) = self.blocks.get(block_idx) {
            if let Some(block_ptr) = block {
                let data_ptr = unsafe { block_ptr.as_ptr().add(core::mem::size_of::<BlockHeader>() + offset) };
                Some(data_ptr)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// 获取块大小
    pub fn block_size(&self) -> usize {
        self.config.block_size - core::mem::size_of::<BlockHeader>()
    }
    
    /// 分配内存块
    fn allocate_block(&self) -> Option<NonNull<u8>> {
        let block_size = self.config.block_size;
        
        match crate::memory::allocator::alloc(block_size) {
            Ok(ptr) => Some(ptr),
            Err(_) => None,
        }
    }
    
    /// 分配JSON内存
    pub fn allocate(&mut self, size: usize) -> Option<(usize, usize)> {
        // 计算需要的块大小（包括块头）
        let required_size = size + core::mem::size_of::<BlockHeader>();
        
        // 检查是否超过块大小
        if required_size > self.config.block_size {
            return None;
        }
        
        // 原子获取首个空闲块
        let mut first_free = self.first_free.load(Ordering::Acquire);
        
        loop {
            if first_free >= self.blocks.len() || first_free == usize::MAX {
                // 没有空闲块，尝试扩展
                if self.blocks.len() < self.config.max_blocks {
                    let block = self.allocate_block();
                    if let Some(block) = block {
                        let new_idx = self.blocks.len();
                        self.blocks.push(Some(block));
                        
                        // 初始化块头
                        let header = unsafe { &mut *(block.as_ptr() as *mut BlockHeader) };
                        header.status = 1;
                        header.ref_count = AtomicUsize::new(1);
                        header.next_free = usize::MAX;
                        header.size = self.config.block_size - core::mem::size_of::<BlockHeader>();
                        
                        self.total_used.fetch_add(size, Ordering::Release);
                        self.free_blocks.fetch_sub(1, Ordering::Release);
                        
                        return Some((new_idx, core::mem::size_of::<BlockHeader>()));
                    }
                }
                return None;
            }
            
            // 检查块是否空闲
            if let Some(block) = self.blocks[first_free] {
                let header = unsafe { &mut *(block.as_ptr() as *mut BlockHeader) };
                
                if header.status == 0 {
                    // 尝试标记为使用中
                    header.status = 1;
                    header.ref_count.store(1, Ordering::Release);
                    
                    // 更新首个空闲块
                    let next_free = header.next_free;
                    if self.first_free.compare_exchange(first_free, next_free, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                        self.total_used.fetch_add(size, Ordering::Release);
                        self.free_blocks.fetch_sub(1, Ordering::Release);
                        return Some((first_free, core::mem::size_of::<BlockHeader>()));
                    }
                }
            }
            
            // 尝试下一个块
            first_free = self.first_free.load(Ordering::Acquire);
        }
    }
    
    /// 增加引用计数
    pub fn add_ref(&self, block_idx: usize, offset: usize) {
        if let Some(block) = self.blocks.get(block_idx) {
            if let Some(block_ptr) = block {
                let header = unsafe { &mut *(block_ptr.as_ptr() as *mut BlockHeader) };
                header.ref_count.fetch_add(1, Ordering::Release);
            }
        }
    }
    
    /// 减少引用计数，当计数为0时释放
    pub fn release(&self, block_idx: usize, offset: usize) {
        if let Some(block) = self.blocks.get(block_idx) {
            if let Some(block_ptr) = block {
                let header = unsafe { &mut *(block_ptr.as_ptr() as *mut BlockHeader) };
                let ref_count = header.ref_count.fetch_sub(1, Ordering::AcqRel);
                
                if ref_count == 1 {
                    // 释放块
                    header.status = 0;
                    header.next_free = self.first_free.load(Ordering::Acquire);
                    
                    // 更新首个空闲块
                    self.first_free.store(block_idx, Ordering::Release);
                    self.free_blocks.fetch_add(1, Ordering::Release);
                    
                    // 计算实际使用的大小
                    let used_size = header.size;
                    self.total_used.fetch_sub(used_size, Ordering::Release);
                }
            }
        }
    }
    
    /// 获取内存池状态
    pub fn status(&self) -> (usize, usize, usize) {
        let total_used = self.total_used.load(Ordering::Acquire);
        let free_blocks = self.free_blocks.load(Ordering::Acquire);
        let total_blocks = self.blocks.len();
        
        (total_used, free_blocks, total_blocks)
    }
    
    /// 清理内存池
    pub fn cleanup(&mut self) {
        for block in &mut self.blocks {
            if let Some(ptr) = block {
                unsafe {
                    crate::memory::allocator::free(*ptr);
                }
            }
        }
        self.blocks.clear();
        self.total_used.store(0, Ordering::Release);
        self.free_blocks.store(0, Ordering::Release);
        self.first_free.store(usize::MAX, Ordering::Release);
    }
}

/// 全局JSON内存池管理器
pub struct JsonPoolManager {
    pools: alloc::vec::Vec<JsonMemoryPool>,
    max_pools: usize,
}

impl JsonPoolManager {
    /// 创建新的内存池管理器
    pub fn new(max_pools: usize) -> Self {
        Self {
            pools: alloc::vec::Vec::with_capacity(max_pools),
            max_pools,
        }
    }
    
    /// 创建新的JSON内存池
    pub fn create_pool(&mut self, config: JsonPoolConfig) -> Option<u8> {
        if self.pools.len() >= self.max_pools {
            return None;
        }
        
        let pool_id = self.pools.len() as u8;
        let pool = JsonMemoryPool::new(pool_id, config);
        self.pools.push(pool);
        
        Some(pool_id)
    }
    
    /// 获取内存池
    pub fn get_pool(&self, pool_id: u8) -> Option<&JsonMemoryPool> {
        self.pools.get(pool_id as usize)
    }
    
    /// 获取可变内存池
    pub fn get_pool_mut(&mut self, pool_id: u8) -> Option<&mut JsonMemoryPool> {
        self.pools.get_mut(pool_id as usize)
    }
    
    /// 清理所有内存池
    pub fn cleanup_all(&mut self) {
        for pool in &mut self.pools {
            pool.cleanup();
        }
        self.pools.clear();
    }
}

impl Default for JsonPoolManager {
    fn default() -> Self {
        Self::new(8)
    }
}

/// 全局JSON内存池管理器实例
static mut GLOBAL_JSON_POOL_MANAGER: Option<JsonPoolManager> = None;

/// 初始化全局JSON内存池管理器
pub fn init_json_pool_manager() {
    unsafe {
        if GLOBAL_JSON_POOL_MANAGER.is_none() {
            GLOBAL_JSON_POOL_MANAGER = Some(JsonPoolManager::default());
        }
    }
}

/// 获取全局JSON内存池管理器
pub fn get_global_json_pool_manager() -> Option<&'static mut JsonPoolManager> {
    unsafe {
        GLOBAL_JSON_POOL_MANAGER.as_mut()
    }
}
