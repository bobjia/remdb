/// 测试用内存分配器实现
use remdb::config::MemoryAllocator;
use core::ptr::NonNull;

/// 简单的内存分配器实现，用于测试
pub struct TestAllocator;

impl TestAllocator {
    /// 创建新的测试分配器
    pub const fn new() -> Self {
        Self
    }
}

impl MemoryAllocator for TestAllocator {
    fn allocate(&self, _size: usize) -> Option<NonNull<u8>> {
        // 测试用实现，总是返回None，表示分配失败
        None
    }
    
    fn deallocate(&self, _ptr: NonNull<u8>, _size: usize) {
        // 测试用实现，不做任何操作
    }
}

/// 全局测试分配器实例
pub static TEST_ALLOCATOR: TestAllocator = TestAllocator::new();
