pub mod allocator;
pub mod pool;

use core::ptr::NonNull;

/// 内存统计信息
pub struct MemoryStats {
    /// 已使用内存大小
    pub used: usize,
    /// 总内存大小
    pub total: usize,
    /// 内存碎片率
    pub fragmentation: f32,
    /// 分配次数
    pub alloc_count: usize,
    /// 释放次数
    pub free_count: usize,
}

/// 内存块头
#[repr(C)]
pub struct MemoryBlock {
    /// 下一个块指针
    pub next: Option<NonNull<MemoryBlock>>,
    /// 块大小
    pub size: usize,
    /// 是否已分配
    pub is_allocated: bool,
}

impl MemoryBlock {
    /// 块头大小
    pub const SIZE: usize = core::mem::size_of::<Self>();
}
