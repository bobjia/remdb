use core::ptr::NonNull;
use crate::memory::{MemoryBlock, MemoryStats};
use crate::types::Result;
use std::sync::{OnceLock, Mutex};

/// 静态内存分配器
pub struct StaticAllocator {
    /// 内存池起始地址
    start_ptr: NonNull<u8>,
    /// 内存池大小
    size: usize,
    /// 已使用内存
    used: usize,
    /// 空闲列表
    free_list: Option<NonNull<MemoryBlock>>,
    /// 分配次数
    alloc_count: usize,
    /// 释放次数
    free_count: usize,
}

// 为StaticAllocator实现Send和Sync trait
// 注意：这是安全的，因为StaticAllocator的所有操作都在锁保护下进行
unsafe impl Send for StaticAllocator {}
unsafe impl Sync for StaticAllocator {}

impl StaticAllocator {
    /// 创建新的静态内存分配器
    pub fn new(start_ptr: *mut u8, size: usize) -> Option<Self> {
        let start_ptr = NonNull::new(start_ptr)?;
        
        // 初始化内存池
        let _end_ptr = (start_ptr.as_ptr() as usize + size) as *mut u8;
        
        // 创建一个大的空闲块
        unsafe {
            let block_ptr = start_ptr.as_ptr() as *mut MemoryBlock;
            (*block_ptr).next = None;
            (*block_ptr).size = size - MemoryBlock::SIZE;
            (*block_ptr).is_allocated = false;
        }
        
        Some(StaticAllocator {
            start_ptr,
            size,
            used: 0,
            free_list: Some(NonNull::new(start_ptr.as_ptr() as *mut MemoryBlock).unwrap()),
            alloc_count: 0,
            free_count: 0,
        })
    }
    
    /// 分配内存
    pub fn allocate(&mut self, size: usize) -> Result<NonNull<u8>> {
        // 对齐到8字节
        let aligned_size = (size + 7) & !7;
        let total_size = aligned_size + MemoryBlock::SIZE;
        
        // 查找合适的空闲块
        let mut current = &mut self.free_list;
        while let Some(mut block) = *current {
            let block_mut = unsafe { block.as_mut() };
            
            // 检查块大小是否足够
            if block_mut.size >= aligned_size {
                // 如果块太大，分割成两个块
                if block_mut.size >= aligned_size + MemoryBlock::SIZE + 8 {
                    unsafe {
                        let new_block_size = block_mut.size - total_size;
                        let new_block_ptr = (block.as_ptr() as usize + total_size) as *mut MemoryBlock;
                        
                        (*new_block_ptr).next = block_mut.next;
                        (*new_block_ptr).size = new_block_size;
                        (*new_block_ptr).is_allocated = false;
                        
                        block_mut.next = Some(NonNull::new_unchecked(new_block_ptr));
                        block_mut.size = aligned_size;
                    }
                }
                
                // 从空闲列表中移除该块
                let _allocated_block = *current;
                *current = unsafe { block.as_mut() }.next;
                
                // 标记为已分配
                unsafe {
                    block.as_mut().is_allocated = true;
                }
                
                // 更新统计信息
                self.used += unsafe { block.as_mut() }.size + MemoryBlock::SIZE;
                self.alloc_count += 1;
                
                // 返回块数据指针
                let data_ptr = (block.as_ptr() as usize + MemoryBlock::SIZE) as *mut u8;
                return Ok(NonNull::new(data_ptr).unwrap())
            }
            
            current = &mut unsafe { block.as_mut() }.next;
        }
        
        // 没有找到合适的块
        Err(crate::types::RemDbError::OutOfMemory)
    }
    
    /// 释放内存
    pub fn free(&mut self, ptr: NonNull<u8>) {
        // 获取块头指针
        let block_ptr = (ptr.as_ptr() as usize - MemoryBlock::SIZE) as *mut MemoryBlock;
        let mut block = NonNull::new(block_ptr).unwrap();
        
        // 标记为未分配
        unsafe {
            block.as_mut().is_allocated = false;
        }
        
        // 更新统计信息
        self.used -= unsafe { block.as_mut() }.size + MemoryBlock::SIZE;
        self.free_count += 1;
        
        // 插入到空闲列表，保持地址有序
        let mut current = &mut self.free_list;
        while let Some(mut current_block) = *current {
            if current_block.as_ptr() > block.as_ptr() {
                // 插入到当前位置之前
                unsafe {
                    block.as_mut().next = Some(current_block);
                }
                *current = Some(block);
                
                // 尝试合并前后块
                self.merge_adjacent_blocks();
                return;
            }
            current = &mut unsafe { current_block.as_mut() }.next;
        }
        
        // 插入到列表末尾
        unsafe {
            block.as_mut().next = None;
        }
        *current = Some(block);
        
        // 尝试合并前后块
        self.merge_adjacent_blocks();
    }
    
    /// 合并相邻的空闲块
    fn merge_adjacent_blocks(&mut self) {
        let mut current = &mut self.free_list;
        while let Some(mut block) = *current {
            let block_mut = unsafe { block.as_mut() };
            
            // 检查下一个块是否相邻
            if let Some(mut next_block) = block_mut.next {
                let next_block_mut = unsafe { next_block.as_mut() };
                let block_end = block.as_ptr() as usize + MemoryBlock::SIZE + block_mut.size;
                let next_block_start = next_block.as_ptr() as usize;
                
                if block_end == next_block_start {
                    // 合并两个块
                    block_mut.size += MemoryBlock::SIZE + next_block_mut.size;
                    block_mut.next = next_block_mut.next;
                    continue;
                }
            }
            
            current = &mut block_mut.next;
        }
    }
    
    /// 获取内存统计信息
    pub fn stats(&self) -> MemoryStats {
        // 计算空闲块数量和最大空闲块大小
        let mut free_blocks = 0;
        let mut max_free_block = 0;
        let mut total_free = 0;
        
        let mut current = self.free_list;
        while let Some(block) = current {
            free_blocks += 1;
            unsafe {
                total_free += block.as_ref().size + MemoryBlock::SIZE;
                if block.as_ref().size > max_free_block {
                    max_free_block = block.as_ref().size;
                }
                current = block.as_ref().next;
            }
        }
        
        // 计算内存碎片率
        let fragmentation = if free_blocks == 0 {
            0.0
        } else {
            1.0 - (max_free_block as f32 / total_free as f32)
        };
        
        MemoryStats {
            used: self.used,
            total: self.size,
            fragmentation,
            alloc_count: self.alloc_count,
            free_count: self.free_count,
        }
    }
    
    /// 重置内存分配器
    pub fn reset(&mut self) {
        // 创建一个大的空闲块
        unsafe {
            let block_ptr = self.start_ptr.as_ptr() as *mut MemoryBlock;
            (*block_ptr).next = None;
            (*block_ptr).size = self.size - MemoryBlock::SIZE;
            (*block_ptr).is_allocated = false;
        }
        
        self.used = 0;
        self.free_list = Some(NonNull::new(self.start_ptr.as_ptr() as *mut MemoryBlock).unwrap());
        self.alloc_count = 0;
        self.free_count = 0;
    }
}

/// 全局内存分配器 - 使用OnceLock和Mutex确保线程安全
static GLOBAL_ALLOCATOR: OnceLock<Mutex<StaticAllocator>> = OnceLock::new();

/// 初始化全局内存分配器
pub fn init_global_allocator(start_ptr: *mut u8, size: usize) -> Result<()> {
    let allocator = StaticAllocator::new(start_ptr, size)
        .ok_or(crate::types::RemDbError::OutOfMemory)?;
    
    GLOBAL_ALLOCATOR.set(Mutex::new(allocator))
        .map_err(|_| crate::types::RemDbError::ConfigError)?;
    
    Ok(())
}

/// 从全局分配器分配内存
pub fn alloc(size: usize) -> Result<NonNull<u8>> {
    let allocator = GLOBAL_ALLOCATOR.get()
        .ok_or(crate::types::RemDbError::OutOfMemory)?;
    
    let mut allocator_guard = allocator.lock().map_err(|_| crate::types::RemDbError::OutOfMemory)?;
    allocator_guard.allocate(size)
}

/// 释放内存到全局分配器
pub fn free(ptr: NonNull<u8>) {
    if let Some(allocator) = GLOBAL_ALLOCATOR.get() {
        if let Ok(mut allocator_guard) = allocator.lock() {
            allocator_guard.free(ptr);
        }
    }
}

/// 获取全局内存统计信息
pub fn get_memory_stats() -> MemoryStats {
    if let Some(allocator) = GLOBAL_ALLOCATOR.get() {
        if let Ok(allocator_guard) = allocator.lock() {
            return allocator_guard.stats();
        }
    }
    
    MemoryStats {
        used: 0,
        total: 0,
        fragmentation: 0.0,
        alloc_count: 0,
        free_count: 0,
    }
}
