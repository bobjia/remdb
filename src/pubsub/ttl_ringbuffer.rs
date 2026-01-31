use std::alloc::{alloc, dealloc, Layout};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::SystemTime;

// 定义槽位状态
const SLOT_FREE: u8 = 0;
const SLOT_WRITING: u8 = 1;
const SLOT_ACTIVE: u8 = 2;
const SLOT_DELETED: u8 = 3;

// 槽位结构体
struct Slot {
    state: AtomicU8,      // 槽位状态
    timestamp: AtomicU64, // 过期时间戳
    data: NonNull<u8>,    // 数据指针
    data_len: usize,      // 数据长度
}

// 小顶堆条目，实现必要的trait以便在BinaryHeap中使用
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone)]
struct TTLHeapEntry {
    expire_time: u64,   // 过期时间
    slot_idx: usize,    // 槽位索引
    logical_idx: usize, // 逻辑索引，用于处理覆盖情况
}

// TTL RingBuffer结构体
pub struct TTLCircularBuffer {
    buffer: *mut Slot,                           // 槽位数组指针
    mask: usize,                                 // 用于快速取模的掩码（size-1）
    current_read: usize,                         // 当前读位置
    current_write: usize,                        // 当前写位置
    capacity: usize,                             // 容量
    ttl_heap: BinaryHeap<Reverse<TTLHeapEntry>>, // 小顶堆：堆顶是最近要过期的数据
}

impl TTLCircularBuffer {
    /// 创建新的TTL环形缓冲区
    pub fn new(capacity: usize) -> Self {
        // 确保容量是2的幂
        let actual_capacity = capacity.next_power_of_two();
        let mask = actual_capacity - 1;

        // 分配槽位数组
        let buffer = unsafe {
            let layout = Layout::array::<Slot>(actual_capacity).unwrap();
            let ptr = alloc(layout) as *mut Slot;
            // 初始化所有槽位
            for i in 0..actual_capacity {
                let slot_ptr = ptr.add(i);
                std::ptr::write(
                    slot_ptr,
                    Slot {
                        state: AtomicU8::new(SLOT_FREE),
                        timestamp: AtomicU64::new(0),
                        data: NonNull::dangling(),
                        data_len: 0,
                    },
                );
            }
            ptr
        };

        Self {
            buffer,
            mask,
            current_read: 0,
            current_write: 0,
            capacity: actual_capacity,
            ttl_heap: BinaryHeap::new(),
        }
    }

    /// 写入数据到缓冲区
    pub fn write(&mut self, data: &[u8], ttl_ms: u64) -> bool {
        // 使用SystemTime获取绝对时间戳（毫秒）
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let expire_time = now + ttl_ms;

        // 获取当前写位置
        let write_idx = self.current_write;
        let slot_idx = write_idx & self.mask;

        // 检查槽位是否可用
        let slot = unsafe { &mut *self.buffer.add(slot_idx) };
        let state = slot.state.load(Ordering::Acquire);
        if state != SLOT_FREE && state != SLOT_DELETED {
            // 槽位忙，尝试淘汰过期数据
            if !self.evict_expired(now) {
                return false; // 淘汰失败，缓冲区已满
            }
            // 重新检查当前槽位
            let state = slot.state.load(Ordering::Acquire);
            if state != SLOT_FREE && state != SLOT_DELETED {
                return false; // 仍然不可用
            }
        }

        // 开始写入
        if !slot
            .state
            .compare_exchange(SLOT_FREE, SLOT_WRITING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return false;
        }

        // 为数据分配内存
        let data_len = data.len();
        let layout = Layout::from_size_align(data_len, 1).unwrap();
        let data_ptr = unsafe {
            let ptr = alloc(layout) as *mut u8;
            // 复制数据到分配的内存中
            core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data_len);
            NonNull::new_unchecked(ptr)
        };

        // 更新槽位信息
        slot.data = data_ptr;
        slot.data_len = data_len;
        slot.timestamp.store(expire_time, Ordering::Release);

        // 写入完成，标记为活跃
        slot.state.store(SLOT_ACTIVE, Ordering::Release);

        // 将条目添加到小顶堆
        self.ttl_heap.push(Reverse(TTLHeapEntry {
            expire_time,
            slot_idx,
            logical_idx: write_idx,
        }));

        // 更新写位置
        self.current_write += 1;

        true
    }

    /// 读取数据
    pub fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
        // 使用SystemTime获取绝对时间戳（毫秒）
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 先清理过期数据
        self.evict_expired(now);

        // 检查是否有数据可读
        if self.current_read == self.current_write {
            return None;
        }

        let read_idx = self.current_read;
        let slot_idx = read_idx & self.mask;
        let slot = unsafe { &mut *self.buffer.add(slot_idx) };

        let state = slot.state.load(Ordering::Acquire);
        if state != SLOT_ACTIVE {
            // 跳过无效槽位
            self.current_read += 1;
            return self.read(buffer);
        }

        // 检查是否过期
        let expire_time = slot.timestamp.load(Ordering::Acquire);
        if expire_time <= now {
            // 过期，释放内存并标记为删除
            unsafe {
                let data_len = slot.data_len;
                let layout = Layout::from_size_align(data_len, 1).unwrap();
                dealloc(slot.data.as_ptr(), layout);
                slot.state.store(SLOT_DELETED, Ordering::Release);
            }
            self.current_read += 1;
            return self.read(buffer);
        }

        // 复制数据
        let data_len = core::cmp::min(slot.data_len, buffer.len());
        unsafe {
            core::ptr::copy_nonoverlapping(slot.data.as_ptr(), buffer.as_mut_ptr(), data_len);

            // 释放内存
            let layout = Layout::from_size_align(slot.data_len, 1).unwrap();
            dealloc(slot.data.as_ptr(), layout);
        }

        // 标记为删除
        slot.state.store(SLOT_DELETED, Ordering::Release);

        // 更新读位置
        self.current_read += 1;

        Some(data_len)
    }

    /// 淘汰过期数据
    pub fn evict_expired(&mut self, now: u64) -> bool {
        // 从堆顶开始检查过期数据
        while let Some(Reverse(entry)) = self.ttl_heap.peek() {
            // 检查堆顶元素是否过期
            if entry.expire_time > now {
                // 堆顶元素未过期，说明没有更多过期数据
                break;
            }

            // 检查该槽位的实际状态和逻辑索引
            let slot = unsafe { &mut *self.buffer.add(entry.slot_idx) };
            let state = slot.state.load(Ordering::Acquire);
            let current_expire_time = slot.timestamp.load(Ordering::Acquire);

            // 计算当前槽位的逻辑索引
            // 逻辑索引 = current_write - (容量 - slot_idx) 当 slot_idx > current_write % capacity
            // 或者逻辑索引 = current_write - slot_idx 当 slot_idx <= current_write % capacity
            let current_slot_idx = self.current_write & self.mask;
            let slot_logical_idx = if entry.slot_idx > current_slot_idx {
                self.current_write - (self.capacity - entry.slot_idx + current_slot_idx)
            } else {
                self.current_write - (current_slot_idx - entry.slot_idx)
            };

            if state == SLOT_ACTIVE {
                // 槽位仍然活跃，检查逻辑索引和过期时间是否匹配（防止覆盖情况）
                if slot_logical_idx == entry.logical_idx && current_expire_time == entry.expire_time
                {
                    // 匹配，释放内存并标记为删除
                    unsafe {
                        let data_len = slot.data_len;
                        let layout = Layout::from_size_align(data_len, 1).unwrap();
                        dealloc(slot.data.as_ptr(), layout);
                        slot.state.store(SLOT_DELETED, Ordering::Release);
                    }
                }
            }

            // 移除堆顶元素
            self.ttl_heap.pop();
        }

        true
    }

    /// 淘汰最短TTL的数据（优化后的实现）
    pub fn evict_shortest_ttl(&mut self, now: u64) -> bool {
        // 清理已过期数据
        self.evict_expired(now);

        // 查找最短TTL的数据
        let mut target_slot = None;

        // 遍历小顶堆，找到实际可用的最短TTL条目
        // 注意：这里需要复制堆内容，因为我们不能直接修改堆
        let mut heap_copy = self.ttl_heap.clone();

        while let Some(Reverse(entry)) = heap_copy.pop() {
            let slot = unsafe { &*self.buffer.add(entry.slot_idx) };
            let state = slot.state.load(Ordering::Acquire);

            if state == SLOT_ACTIVE {
                // 检查逻辑索引是否匹配（防止覆盖情况）
                let current_slot_idx = self.current_write & self.mask;
                let slot_logical_idx = if entry.slot_idx > current_slot_idx {
                    self.current_write - (self.capacity - entry.slot_idx + current_slot_idx)
                } else {
                    self.current_write - (current_slot_idx - entry.slot_idx)
                };
                let current_expire_time = slot.timestamp.load(Ordering::Acquire);

                if slot_logical_idx == entry.logical_idx && current_expire_time == entry.expire_time
                {
                    // 找到目标槽位
                    target_slot = Some(entry.slot_idx);
                    break;
                }
            }
        }

        // 执行淘汰
        if let Some(slot_idx) = target_slot {
            unsafe {
                let slot = &mut *self.buffer.add(slot_idx);
                // 释放内存
                let data_len = slot.data_len;
                let layout = Layout::from_size_align(data_len, 1).unwrap();
                dealloc(slot.data.as_ptr(), layout);
                // 标记为删除
                slot.state.store(SLOT_DELETED, Ordering::Release);
            }
            return true;
        }

        // 如果小顶堆中没有找到可用条目，回退到线性扫描
        // 这种情况很少发生，通常是因为堆中的条目已经被覆盖或删除
        self.evict_shortest_ttl_fallback(now)
    }

    /// 回退方案：线性扫描找到最短TTL的数据
    fn evict_shortest_ttl_fallback(&self, now: u64) -> bool {
        let mut shortest_ttl = u64::MAX;
        let mut target_slot_idx = None;

        // 扫描队列中所有数据
        for logical_idx in self.current_read..self.current_write {
            let slot_idx = logical_idx & self.mask;
            let slot = unsafe { &*self.buffer.add(slot_idx) };

            let state = slot.state.load(Ordering::Acquire);
            if state == SLOT_ACTIVE {
                let expire_time = slot.timestamp.load(Ordering::Acquire);

                // 计算剩余TTL
                let remaining_ttl = expire_time.saturating_sub(now);
                if remaining_ttl < shortest_ttl {
                    shortest_ttl = remaining_ttl;
                    target_slot_idx = Some(slot_idx);
                }
            }
        }

        // 淘汰找到的剩余TTL最短的数据
        if let Some(slot_idx) = target_slot_idx {
            unsafe {
                let slot = &mut *self.buffer.add(slot_idx);
                // 释放内存
                let data_len = slot.data_len;
                let layout = Layout::from_size_align(data_len, 1).unwrap();
                dealloc(slot.data.as_ptr(), layout);
                // 标记为删除
                slot.state.store(SLOT_DELETED, Ordering::Release);
            }
            return true;
        }

        false
    }

    /// 获取当前可用空间
    pub fn available_space(&self) -> usize {
        self.capacity - (self.current_write - self.current_read)
    }

    /// 获取当前已使用空间
    pub fn used_space(&self) -> usize {
        self.current_write - self.current_read
    }
}

// 实现Drop trait，释放资源
impl Drop for TTLCircularBuffer {
    fn drop(&mut self) {
        // 释放所有槽位的数据内存
        unsafe {
            for i in 0..self.capacity {
                let slot = &mut *self.buffer.add(i);
                let state = slot.state.load(Ordering::Acquire);
                if state == SLOT_ACTIVE {
                    // 释放数据内存
                    let data_len = slot.data_len;
                    let layout = Layout::from_size_align(data_len, 1).unwrap();
                    dealloc(slot.data.as_ptr(), layout);
                }
                // 释放槽位本身
                std::ptr::drop_in_place(self.buffer.add(i));
            }
            // 释放缓冲区内存
            let layout = Layout::array::<Slot>(self.capacity).unwrap();
            dealloc(self.buffer as *mut u8, layout);
        }
    }
}

// 测试用例，仅在测试模式下编译
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_buffer_creation() {
        // 测试创建不同容量的缓冲区
        let buffer = TTLCircularBuffer::new(8);
        assert_eq!(buffer.capacity, 8);

        let buffer = TTLCircularBuffer::new(10);
        assert_eq!(buffer.capacity, 16); // 容量应自动调整为2的幂
    }

    #[test]
    fn test_write_read() {
        let mut buffer = TTLCircularBuffer::new(8);
        let data = b"test data";

        // 写入数据
        let success = buffer.write(data, 1000);
        assert!(success);

        // 读取数据
        let mut read_buf = vec![0; 100];
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, Some(data.len()));
        assert_eq!(&read_buf[..data.len()], data);

        // 读取完后应该没有数据了
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, None);
    }

    #[test]
    fn test_expired_data() {
        let mut buffer = TTLCircularBuffer::new(8);
        let data = b"test data";

        // 写入数据，TTL设置为1毫秒
        let success = buffer.write(data, 1);
        assert!(success);

        // 等待2毫秒，确保数据过期
        std::thread::sleep(Duration::from_millis(2));

        // 读取数据，应该返回None，因为数据已经过期
        let mut read_buf = vec![0; 100];
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, None);
    }

    #[test]
    fn test_evict_expired() {
        let mut buffer = TTLCircularBuffer::new(8);
        let data1 = b"data1";
        let data2 = b"data2";

        // 写入第一个数据，TTL设置为1毫秒
        let success = buffer.write(data1, 1);
        assert!(success);

        // 等待2毫秒，确保第一个数据过期
        std::thread::sleep(Duration::from_millis(2));

        // 清理过期数据
        let now = Instant::now().elapsed().as_millis() as u64;
        buffer.evict_expired(now);

        // 写入第二个数据，应该成功，因为第一个数据已经被清理
        let success = buffer.write(data2, 1000);
        assert!(success);

        // 读取数据，应该只能读到第二个数据
        let mut read_buf = vec![0; 100];
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, Some(data2.len()));
        assert_eq!(&read_buf[..data2.len()], data2);
    }

    #[test]
    fn test_evict_shortest_ttl() {
        let mut buffer = TTLCircularBuffer::new(8);
        let data1 = b"short ttl";
        let data2 = b"long ttl";

        let now = Instant::now().elapsed().as_millis() as u64;

        // 写入第一个数据，TTL设置为100毫秒
        let success = buffer.write(data1, 100);
        assert!(success);

        // 写入第二个数据，TTL设置为1000毫秒
        let success = buffer.write(data2, 1000);
        assert!(success);

        // 手动调用evict_shortest_ttl，应该淘汰第一个数据（TTL较短）
        let result = buffer.evict_shortest_ttl(now);
        assert!(result);

        // 读取数据，应该只能读到第二个数据
        let mut read_buf = vec![0; 100];
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, Some(data2.len()));
        assert_eq!(&read_buf[..data2.len()], data2);

        // 再次读取，应该没有数据了
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, None);
    }

    #[test]
    fn test_buffer_full() {
        let mut buffer = TTLCircularBuffer::new(2); // 实际容量为2
        let data1 = b"data1";
        let data2 = b"data2";
        let data3 = b"data3";

        // 写入两个数据，应该成功
        let success1 = buffer.write(data1, 1000);
        let success2 = buffer.write(data2, 1000);
        assert!(success1);
        assert!(success2);

        // 写入第三个数据，应该失败，因为缓冲区已满
        let success3 = buffer.write(data3, 1000);
        assert!(!success3);
    }

    #[test]
    fn test_available_used_space() {
        let mut buffer = TTLCircularBuffer::new(8);
        let data = b"test";

        // 初始状态：可用空间为8，已使用空间为0
        assert_eq!(buffer.available_space(), 8);
        assert_eq!(buffer.used_space(), 0);

        // 写入一个数据
        let success = buffer.write(data, 1000);
        assert!(success);

        // 写入后：可用空间为7，已使用空间为1
        assert_eq!(buffer.available_space(), 7);
        assert_eq!(buffer.used_space(), 1);

        // 读取数据
        let mut read_buf = vec![0; 100];
        let read_len = buffer.read(&mut read_buf);
        assert_eq!(read_len, Some(data.len()));

        // 读取后：可用空间应该恢复为8，已使用空间为0
        // 注意：这里由于RingBuffer的设计，读取后current_read会增加，所以used_space会减少
        // 但available_space的计算是基于capacity - (current_write - current_read)，所以会增加
        assert_eq!(buffer.available_space(), 8);
        assert_eq!(buffer.used_space(), 0);
    }
}
