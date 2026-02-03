use crate::memory::{MemoryBlock, MemoryStats};
use crate::types::Result;
use core::ptr::NonNull;

// 使用条件编译，在std环境下使用std::sync::OnceLock，在no_std环境下使用platform::OnceLock
#[cfg(feature = "std")]
use std::sync::OnceLock;

#[cfg(not(feature = "std"))]
use crate::platform::OnceLock;

// 根据是否启用std特性选择不同的同步机制
#[cfg(feature = "std")]
use std::sync::Mutex;

// no_std环境下的简单自旋锁实现
#[cfg(not(feature = "std"))]
pub struct Mutex<T> {
    data: core::cell::UnsafeCell<T>,
    lock: u32,
}

#[cfg(not(feature = "std"))]
impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Mutex {
            data: core::cell::UnsafeCell::new(data),
            lock: 0,
        }
    }

    pub fn lock(&self) -> core::result::Result<MutexGuard<'_, T>, ()> {
        // 简单的自旋锁实现
        while unsafe {
            core::sync::atomic::AtomicU32::from_ptr(&self.lock as *const u32 as *mut u32)
                .compare_exchange(
                    0,
                    1,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
        } {
            core::hint::spin_loop();
        }

        Ok(MutexGuard { mutex: self })
    }
}

#[cfg(not(feature = "std"))]
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

#[cfg(not(feature = "std"))]
impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

#[cfg(not(feature = "std"))]
impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            core::sync::atomic::AtomicU32::from_ptr(&self.mutex.lock as *const u32 as *mut u32)
                .store(0, core::sync::atomic::Ordering::Release);
        }
    }
}

// 为Mutex添加Sync trait实现
#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Sync for Mutex<T> {}

// 为Mutex添加Send trait实现
#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Send for Mutex<T> {}

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

// Note: Clone trait implementation removed because it's unsafe
// Cloning would create a new allocator with the same memory pool,
// which can lead to double-free or use-after-free errors when the memory pool is updated.

impl StaticAllocator {
    /// 创建新的静态内存分配器
    pub fn new(start_ptr: *mut u8, size: usize) -> Option<Self> {
        // 计算MemoryBlock所需的对齐值
        const ALIGNMENT: usize = core::mem::align_of::<MemoryBlock>();

        // 对齐start_ptr到MemoryBlock的对齐要求
        let start_addr = start_ptr as usize;
        let aligned_addr = (start_addr + ALIGNMENT - 1) & !(ALIGNMENT - 1);
        let aligned_ptr = aligned_addr as *mut u8;

        // 计算对齐后的可用大小
        let aligned_size = size - (aligned_addr - start_addr);

        // 确保对齐后的大小足够容纳至少一个MemoryBlock
        if aligned_size < MemoryBlock::SIZE {
            return None;
        }

        let mut allocator = StaticAllocator {
            start_ptr: NonNull::new(aligned_ptr)?,
            size: aligned_size,
            used: 0,
            free_list: None,
            alloc_count: 0,
            free_count: 0,
        };

        allocator.reset();
        Some(allocator)
    }

    /// 重置分配器，重新初始化内存池
    pub fn reset(&mut self) {
        // 创建一个大的空闲块
        unsafe {
            let block_ptr = self.start_ptr.as_ptr() as *mut MemoryBlock;
            (*block_ptr).next = None;
            (*block_ptr).size = self.size - MemoryBlock::SIZE;
            (*block_ptr).is_allocated = false;

            self.free_list = Some(NonNull::new_unchecked(block_ptr));
            self.used = 0;
            self.alloc_count = 0;
            self.free_count = 0;
        }
    }

    /// 更新内存池
    pub fn update_memory_pool(&mut self, start_ptr: *mut u8, size: usize) {
        // 计算MemoryBlock所需的对齐值
        const ALIGNMENT: usize = core::mem::align_of::<MemoryBlock>();

        // 对齐start_ptr到MemoryBlock的对齐要求
        let start_addr = start_ptr as usize;
        let aligned_addr = (start_addr + ALIGNMENT - 1) & !(ALIGNMENT - 1);
        let aligned_ptr = aligned_addr as *mut u8;

        // 计算对齐后的可用大小
        let aligned_size = size - (aligned_addr - start_addr);

        // 更新内存池信息
        self.start_ptr = NonNull::new(aligned_ptr).expect("Failed to update memory pool");
        self.size = aligned_size;

        // 重置分配器
        self.reset();
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
                        let new_block_size = block_mut.size - aligned_size - MemoryBlock::SIZE;
                        let new_block_ptr =
                            (block.as_ptr() as usize + total_size) as *mut MemoryBlock;

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
                return Ok(NonNull::new(data_ptr).unwrap());
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
        
        // 检查块指针是否在当前分配器的内存范围内
        let block_addr = block_ptr as usize;
        let start_addr = self.start_ptr.as_ptr() as usize;
        let end_addr = start_addr + self.size;
        
        // 如果指针不在当前分配器的内存范围内，直接返回
        // 这避免了在分配器被替换时的访问冲突
        if block_addr < start_addr || block_addr >= end_addr {
            return;
        }
        
        // 检查块指针是否有效
        let Some(mut block) = NonNull::new(block_ptr) else {
            return;
        };

        // 标记为未分配
        unsafe {
            block.as_mut().is_allocated = false;
        }

        // 更新统计信息
        let block_size = unsafe { block.as_mut() }.size + MemoryBlock::SIZE;
        // 防止溢出：只有当used >= block_size时才减去，否则保持不变
        // 这是因为如果used < block_size，说明存在内存分配错误
        if self.used >= block_size {
            self.used -= block_size;
        }
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
}

/// 全局内存分配器 - 使用Mutex和Option确保可以重置
#[cfg(feature = "std")]
static GLOBAL_ALLOCATOR: std::sync::Mutex<Option<StaticAllocator>> = std::sync::Mutex::new(None);

#[cfg(not(feature = "std"))]
static GLOBAL_ALLOCATOR: Mutex<Option<StaticAllocator>> = Mutex::new(None);

/// 初始化全局内存分配器
pub fn init_global_allocator(start_ptr: *mut u8, size: usize) -> Result<()> {
    // 检查内存大小是否足够
    if size < MemoryBlock::SIZE * 2 { // 至少需要两个块头大小
        return Err(crate::types::RemDbError::OutOfMemory);
    }
    
    // 检查内存指针是否有效
    if start_ptr.is_null() {
        return Err(crate::types::RemDbError::OutOfMemory);
    }
    
    // 创建新的分配器实例
    let new_allocator = 
        StaticAllocator::new(start_ptr, size).ok_or(crate::types::RemDbError::OutOfMemory)?;

    // 锁定并替换现有的分配器
    let mut allocator_guard = GLOBAL_ALLOCATOR
        .lock()
        .map_err(|_| crate::types::RemDbError::OutOfMemory)?;

    // 直接替换为新的分配器
    *allocator_guard = Some(new_allocator);

    Ok(())
}

/// 从全局分配器分配内存
pub fn alloc(size: usize) -> Result<NonNull<u8>> {
    let mut allocator_guard = GLOBAL_ALLOCATOR
        .lock()
        .map_err(|_| crate::types::RemDbError::OutOfMemory)?;

    let allocator = allocator_guard
        .as_mut()
        .ok_or(crate::types::RemDbError::OutOfMemory)?;

    allocator.allocate(size)
}

/// 释放内存到全局分配器
pub fn free(ptr: NonNull<u8>) {
    if let Ok(mut allocator_guard) = GLOBAL_ALLOCATOR.lock() {
        if let Some(allocator) = allocator_guard.as_mut() {
            allocator.free(ptr);
        }
    }
}

/// 获取全局内存统计信息
pub fn get_memory_stats() -> MemoryStats {
    if let Ok(allocator_guard) = GLOBAL_ALLOCATOR.lock() {
        if let Some(allocator) = allocator_guard.as_ref() {
            return allocator.stats();
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

/// 重置全局内存分配器
pub fn reset_global_allocator() -> Result<()> {
    let mut allocator_guard = GLOBAL_ALLOCATOR
        .lock()
        .map_err(|_| crate::types::RemDbError::OutOfMemory)?;
    
    if let Some(allocator) = allocator_guard.as_mut() {
        allocator.reset();
    }

    Ok(())
}

// 为no_std环境实现全局内存分配器
#[cfg(not(feature = "std"))]
pub struct GlobalAllocator;

#[cfg(not(feature = "std"))]
unsafe impl core::alloc::GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        match crate::memory::allocator::alloc(layout.size()) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        if let Some(non_null_ptr) = core::ptr::NonNull::new(ptr) {
            crate::memory::allocator::free(non_null_ptr);
        }
    }
}

// 声明全局内存分配器
#[cfg(not(feature = "std"))]
#[global_allocator]
pub static GLOBAL_ALLOC: GlobalAllocator = GlobalAllocator;
