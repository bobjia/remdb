#![allow(unsafe_code)]

use remdb::memory::*;
use remdb::memory::pool::*;
use remdb::types::Result;

#[test]
fn test_static_allocator() {
    // 分配一个内存缓冲区
    let mut buffer = [0u8; 4096];
    
    // 创建静态分配器
    unsafe {
        let mut allocator = StaticAllocator::new(buffer.as_mut_ptr(), buffer.len()).expect("Failed to create allocator");
        
        // 测试分配内存
        let ptr1 = allocator.allocate(64).unwrap();
        
        let ptr2 = allocator.allocate(128).unwrap();
        assert!(ptr1.as_ptr() as usize + 64 + core::mem::size_of::<MemoryBlock>() <= ptr2.as_ptr() as usize);
        
        // 测试释放内存
        allocator.free(ptr1);
        
        // 测试重新分配
        let ptr3 = allocator.allocate(64).unwrap();
        
        // 测试内存统计
        let stats = allocator.stats();
        // 正确的计算：128字节分配 + 64字节重新分配 + 2个MemoryBlock大小
        let expected_used = 128 + 64 + 2 * core::mem::size_of::<MemoryBlock>();
        assert_eq!(stats.used, expected_used);
        assert_eq!(stats.total, buffer.len());
        assert_eq!(stats.alloc_count, 3);
        assert_eq!(stats.free_count, 1);
    }
}

#[test]
fn test_memory_pool() {
    let block_size = 64;
    let mut pool = MemoryPool::new(block_size, 64);

    // 测试分配内存块（返回块索引）
    let idx1 = pool.allocate().unwrap();
    let idx2 = pool.allocate().unwrap();

    // 块索引是连续分配的
    assert_eq!(idx2, idx1 + 1);
    assert!(idx1 < pool.total_blocks());

    // 测试对块的读写访问
    {
        let buf1 = pool.get_block_mut(idx1);
        buf1[0] = 42;
        buf1[block_size - 1] = 7;
    }
    assert_eq!(pool.get_block(idx1)[0], 42);
    assert_eq!(pool.get_block(idx1)[block_size - 1], 7);

    // 测试释放内存块
    pool.free(idx1);
    assert_eq!(pool.used_blocks(), 1);

    // 测试重新分配（会复用刚释放的块索引）
    let idx3 = pool.allocate().unwrap();
    assert_eq!(idx3, idx1);

    // 测试内存池统计信息
    assert_eq!(pool.used_blocks(), 2);
    assert_eq!(pool.total_blocks(), 64);
    assert_eq!(pool.free_blocks(), 62);

    // 测试内存池使用率
    assert_eq!(pool.usage(), 2.0 / 64.0);
}

#[test]
fn test_multi_pool_manager() {
    // 定义内存池配置
    let mut pools = [
        MemoryPool::new(16, 16),
        MemoryPool::new(32, 8),
        MemoryPool::new(64, 4),
        MemoryPool::new(128, 2),
    ];

    // 创建多内存池管理器
    let mut manager = MultiPoolManager::new(&mut pools);

    // 测试分配不同大小的内存（返回 (池索引, 块索引)）
    let (pool1, block1) = manager.allocate(10).unwrap();
    let (pool2, block2) = manager.allocate(20).unwrap();
    let (pool3, block3) = manager.allocate(50).unwrap();
    let (pool4, block4) = manager.allocate(100).unwrap();

    // 验证请求被分配到满足大小要求的内存池
    assert_eq!(pool1, 0);
    assert_eq!(pool2, 1);
    assert_eq!(pool3, 2);
    assert_eq!(pool4, 3);

    // 测试对块的读写访问
    {
        let buf = manager.get_block_mut(pool1, block1);
        buf[0] = 99;
    }
    assert_eq!(manager.get_block(pool1, block1)[0], 99);

    // 测试释放内存
    manager.free(pool1, block1);
    manager.free(pool2, block2);
    manager.free(pool3, block3);
    manager.free(pool4, block4);

    // 测试内存池总使用率
    assert_eq!(manager.total_usage(), 0.0);
}

#[test]
fn test_allocator_edge_cases() {
    // 分配一个小内存缓冲区
    let mut buffer = [0u8; 128];
    
    unsafe {
        let mut allocator = StaticAllocator::new(buffer.as_mut_ptr(), buffer.len()).expect("Failed to create allocator");
        
        // 测试分配接近最大容量的内存
        let ptr1 = allocator.allocate(100).unwrap();
        
        // 测试分配超出容量的内存
        let result = allocator.allocate(100);
        assert!(result.is_err());
        
        // 释放内存后再测试
        allocator.free(ptr1);
        let ptr2 = allocator.allocate(100).unwrap();
    }
}
