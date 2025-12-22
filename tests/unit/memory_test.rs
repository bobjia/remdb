use remdb::memory::allocator::*;
use remdb::memory::pool::*;
use remdb::types::Result;
use remdb::memory::MemoryBlock;

#[test]
fn test_static_allocator() {
    // 分配一个内存缓冲区
    let mut buffer = [0u8; 4096];
    
    // 创建静态分配器
    unsafe {
        let mut allocator = StaticAllocator::new(buffer.as_mut_ptr(), buffer.len());
        
        // 测试分配内存
        let ptr1 = allocator.allocate(64).unwrap();
        assert!(!ptr1.is_null());
        
        let ptr2 = allocator.allocate(128).unwrap();
        assert!(!ptr2.is_null());
        assert!(ptr1.as_ptr() as usize + 64 + core::mem::size_of::<MemoryBlock>() <= ptr2.as_ptr() as usize);
        
        // 测试释放内存
        allocator.free(ptr1);
        
        // 测试重新分配
        let ptr3 = allocator.allocate(64).unwrap();
        assert!(!ptr3.is_null());
        
        // 测试内存统计
        let stats = allocator.stats();
        assert_eq!(stats.used, (128 + core::mem::size_of::<MemoryBlock>()) * 2);
        assert_eq!(stats.total, buffer.len());
        assert_eq!(stats.alloc_count, 3);
        assert_eq!(stats.free_count, 1);
    }
}

#[test]
fn test_memory_pool() {
    // 分配一个内存缓冲区
    let mut buffer = [0u8; 4096];
    
    // 创建内存池
    unsafe {
        let mut pool = MemoryPool::new(
            buffer.as_mut_ptr(),
            64,  // 块大小
            64   // 块数量
        );
        
        // 测试分配内存块
        let ptr1 = pool.allocate().unwrap();
        assert!(!ptr1.is_null());
        
        let ptr2 = pool.allocate().unwrap();
        assert!(!ptr2.is_null());
        assert_eq!(
            ptr2.as_ptr() as usize - ptr1.as_ptr() as usize,
            64  // 块大小
        );
        
        // 测试释放内存块
        pool.free(ptr1);
        assert_eq!(pool.used_blocks(), 1);
        
        // 测试重新分配
        let ptr3 = pool.allocate().unwrap();
        assert!(!ptr3.is_null());
        assert_eq!(ptr3.as_ptr(), ptr1.as_ptr());
        
        // 测试内存池使用率
        assert_eq!(pool.usage(), 2.0 / 64.0);
    }
}

#[test]
fn test_multi_pool_manager() {
    // 分配内存缓冲区
    let mut buffer = [0u8; 8192];
    
    // 定义内存池配置
    let pool_sizes = [16, 32, 64, 128];
    let pool_counts = [16, 8, 4, 2];
    
    // 创建内存池
    unsafe {
        let mut pools = [
            MemoryPool::new(
                buffer.as_mut_ptr(),
                16, 16
            ),
            MemoryPool::new(
                (buffer.as_mut_ptr() as usize + 16 * 16) as *mut u8,
                32, 8
            ),
            MemoryPool::new(
                (buffer.as_mut_ptr() as usize + 16 * 16 + 32 * 8) as *mut u8,
                64, 4
            ),
            MemoryPool::new(
                (buffer.as_mut_ptr() as usize + 16 * 16 + 32 * 8 + 64 * 4) as *mut u8,
                128, 2
            ),
        ];
        
        // 创建多内存池管理器
        let mut manager = MultiPoolManager::new(&mut pools);
        
        // 测试分配不同大小的内存
        let ptr1 = manager.allocate(10).unwrap();
        assert!(!ptr1.is_null());
        
        let ptr2 = manager.allocate(20).unwrap();
        assert!(!ptr2.is_null());
        
        let ptr3 = manager.allocate(50).unwrap();
        assert!(!ptr3.is_null());
        
        let ptr4 = manager.allocate(100).unwrap();
        assert!(!ptr4.is_null());
        
        // 测试释放内存
        manager.free(ptr1);
        manager.free(ptr2);
        manager.free(ptr3);
        manager.free(ptr4);
        
        // 测试内存池总使用率
        assert_eq!(manager.total_usage(), 0.0);
    }
}

#[test]
fn test_allocator_edge_cases() {
    // 分配一个小内存缓冲区
    let mut buffer = [0u8; 128];
    
    unsafe {
        let mut allocator = StaticAllocator::new(buffer.as_mut_ptr(), buffer.len());
        
        // 测试分配接近最大容量的内存
        let ptr1 = allocator.allocate(100).unwrap();
        assert!(!ptr1.is_null());
        
        // 测试分配超出容量的内存
        let result = allocator.allocate(100);
        assert!(result.is_err());
        
        // 释放内存后再测试
        allocator.free(ptr1);
        let ptr2 = allocator.allocate(100).unwrap();
        assert!(!ptr2.is_null());
    }
}
