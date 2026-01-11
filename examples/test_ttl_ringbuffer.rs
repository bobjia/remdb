// 示例文件，用于测试ttl_ringbuffer模块
#![cfg(feature = "pubsub")]

use std::time::{Instant, Duration};

// 导入ttl_ringbuffer模块，使用库名称
use remdb::pubsub::ttl_ringbuffer;


fn main() {
    println!("Running TTL RingBuffer tests...");
    
    // 测试1：缓冲区创建
    println!("\nTest 1: Buffer Creation");
    let buffer = ttl_ringbuffer::TTLCircularBuffer::new(8);
    // 不直接访问capacity字段，而是通过可用空间来验证
    assert_eq!(buffer.available_space(), 8, "Test 1 failed: Initial available space should be 8");
    assert_eq!(buffer.used_space(), 0, "Test 1 failed: Initial used space should be 0");
    println!("✓ Test 1 passed: Buffer creation works correctly");
    
    // 测试2：写入和读取数据
    println!("\nTest 2: Write and Read");
    let mut buffer = ttl_ringbuffer::TTLCircularBuffer::new(8);
    let data = b"test data";
    
    let success = buffer.write(data, 1000);
    assert!(success, "Test 2 failed: Write should succeed");
    
    let mut read_buf = vec![0; 100];
    let read_len = buffer.read(&mut read_buf);
    assert_eq!(read_len, Some(data.len()), "Test 2 failed: Read should return correct length");
    assert_eq!(&read_buf[..data.len()], data, "Test 2 failed: Read data should match written data");
    println!("✓ Test 2 passed: Write and read work correctly");
    
    // 测试3：过期数据处理
    println!("\nTest 3: Expired Data Handling");
    let mut buffer = ttl_ringbuffer::TTLCircularBuffer::new(8);
    let data = b"expiring data";
    
    let success = buffer.write(data, 1);
    assert!(success, "Test 3 failed: Write should succeed");
    
    std::thread::sleep(Duration::from_millis(2));
    
    let mut read_buf = vec![0; 100];
    let read_len = buffer.read(&mut read_buf);
    assert_eq!(read_len, None, "Test 3 failed: Read should return None for expired data");
    println!("✓ Test 3 passed: Expired data is handled correctly");
    
    // 测试4：可用空间和已使用空间
    println!("\nTest 4: Available and Used Space");
    let mut buffer = ttl_ringbuffer::TTLCircularBuffer::new(8);
    let data = b"test";
    
    assert_eq!(buffer.available_space(), 8, "Test 4 failed: Initial available space should be 8");
    assert_eq!(buffer.used_space(), 0, "Test 4 failed: Initial used space should be 0");
    
    let success = buffer.write(data, 1000);
    assert!(success, "Test 4 failed: Write should succeed");
    
    assert_eq!(buffer.available_space(), 7, "Test 4 failed: Available space should decrease after write");
    assert_eq!(buffer.used_space(), 1, "Test 4 failed: Used space should increase after write");
    println!("✓ Test 4 passed: Space tracking works correctly");
    
    // 测试5：evict_shortest_ttl功能
    println!("\nTest 5: Evict Shortest TTL");
    let mut buffer = ttl_ringbuffer::TTLCircularBuffer::new(8);
    let data1 = b"short ttl";
    let data2 = b"long ttl";
    
    let now = Instant::now().elapsed().as_millis() as u64;
    
    let success1 = buffer.write(data1, 100);
    let success2 = buffer.write(data2, 1000);
    assert!(success1, "Test 5 failed: Write 1 should succeed");
    assert!(success2, "Test 5 failed: Write 2 should succeed");
    
    let result = buffer.evict_shortest_ttl(now);
    assert!(result, "Test 5 failed: evict_shortest_ttl should succeed");
    
    // 读取数据，应该只能读到第二个数据
    let mut read_buf = vec![0; 100];
    let read_len = buffer.read(&mut read_buf);
    assert_eq!(read_len, Some(data2.len()), "Test 5 failed: Read should return data2");
    assert_eq!(&read_buf[..data2.len()], data2, "Test 5 failed: Read data should be data2");
    
    // 再次读取，应该没有数据了
    let read_len = buffer.read(&mut read_buf);
    assert_eq!(read_len, None, "Test 5 failed: Read should return None");
    println!("✓ Test 5 passed: evict_shortest_ttl works correctly");
    
    println!("\nAll tests passed! ✓");
}