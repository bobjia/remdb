use alloc::{collections::BTreeMap, string::String, string::ToString, sync::Arc, vec::Vec};

#[cfg(feature = "std")]
use std::{collections::HashMap, sync::RwLock};

#[cfg(not(feature = "std"))]
use crate::memory::allocator::Mutex;

// For no-std mode, we don't have RwLock, so we'll use Mutex instead
#[cfg(not(feature = "std"))]
use crate::memory::allocator::Mutex as RwLock;

/// 时序索引
pub struct TimeSeriesIndex {
    /// 时间到记录ID的映射
    time_index: RwLock<BTreeMap<u64, Vec<usize>>>,
    /// 标签索引
    #[cfg(feature = "std")]
    tag_index: RwLock<HashMap<String, HashMap<String, Vec<usize>>>>,
    #[cfg(not(feature = "std"))]
    tag_index: RwLock<BTreeMap<String, BTreeMap<String, Vec<usize>>>>,
}

impl TimeSeriesIndex {
    /// 创建新的时序索引
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            time_index: RwLock::new(BTreeMap::new()),
            #[cfg(feature = "std")]
            tag_index: RwLock::new(HashMap::new()),
            #[cfg(not(feature = "std"))]
            tag_index: RwLock::new(BTreeMap::new()),
        })
    }

    /// 插入索引项
    pub fn insert(&self, timestamp: u64, record_id: usize) {
        #[cfg(feature = "std")]
        let mut time_index = self.time_index.write().unwrap();
        #[cfg(not(feature = "std"))]
        let mut time_index = self.time_index.lock().unwrap();
        time_index.entry(timestamp).or_default().push(record_id);
    }

    /// 插入标签索引
    pub fn insert_tag(&self, tag_name: &str, tag_value: &str, record_id: usize) {
        #[cfg(feature = "std")]
        let mut tag_index = self.tag_index.write().unwrap();
        #[cfg(not(feature = "std"))]
        let mut tag_index = self.tag_index.lock().unwrap();
        tag_index
            .entry(tag_name.to_string())
            .or_default()
            .entry(tag_value.to_string())
            .or_default()
            .push(record_id);
    }

    /// 时间范围查询
    pub fn query_time_range(&self, start_time: u64, end_time: u64) -> Vec<usize> {
        #[cfg(feature = "std")]
        let time_index = self.time_index.read().unwrap();
        #[cfg(not(feature = "std"))]
        let time_index = self.time_index.lock().unwrap();
        let mut result = Vec::new();

        for (_, ids) in time_index.range(start_time..=end_time) {
            result.extend_from_slice(ids);
        }

        result
    }

    /// 标签过滤
    #[cfg(feature = "std")]
    pub fn filter_by_tags(
        &self,
        record_ids: &[usize],
        tags: &HashMap<String, String>,
    ) -> Vec<usize> {
        if tags.is_empty() {
            return record_ids.to_vec();
        }

        let mut filtered_ids = record_ids.to_vec();

        let tag_index = self.tag_index.read().unwrap();

        for (tag_name, tag_value) in tags {
            if let Some(tag_values) = tag_index.get(tag_name) {
                if let Some(matching_ids) = tag_values.get(tag_value) {
                    // 交集操作
                    let mut new_filtered: Vec<usize> = Vec::new();
                    let matching_ids: &Vec<usize> = matching_ids;
                    for &id in &filtered_ids {
                        if matching_ids.contains(&id) {
                            new_filtered.push(id);
                        }
                    }
                    filtered_ids = new_filtered;

                    if filtered_ids.is_empty() {
                        break;
                    }
                } else {
                    // 没有匹配的标签值，返回空结果
                    return Vec::new();
                }
            } else {
                // 标签名不存在，返回空结果
                return Vec::new();
            }
        }

        filtered_ids
    }

    /// 标签过滤 - no-std版本
    #[cfg(not(feature = "std"))]
    pub fn filter_by_tags(
        &self,
        record_ids: &[usize],
        tags: &BTreeMap<String, String>,
    ) -> Vec<usize> {
        if tags.is_empty() {
            return record_ids.to_vec();
        }

        let tag_index = self.tag_index.lock().unwrap();
        let mut filtered_ids = record_ids.to_vec();

        for (tag_name, tag_value) in tags {
            if let Some(tag_values) = tag_index.get(tag_name) {
                if let Some(matching_ids) = tag_values.get(tag_value) {
                    // 交集操作
                    let mut new_filtered: Vec<usize> = Vec::new();
                    let matching_ids: &Vec<usize> = matching_ids;
                    for &id in &filtered_ids {
                        if matching_ids.contains(&id) {
                            new_filtered.push(id);
                        }
                    }
                    filtered_ids = new_filtered;

                    if filtered_ids.is_empty() {
                        break;
                    }
                } else {
                    // 没有匹配的标签值，返回空结果
                    return Vec::new();
                }
            } else {
                // 标签名不存在，返回空结果
                return Vec::new();
            }
        }

        filtered_ids
    }

    /// 清除指定时间之前的索引
    pub fn clear_before(&self, timestamp: u64) {
        #[cfg(feature = "std")]
        let mut time_index = self.time_index.write().unwrap();
        #[cfg(not(feature = "std"))]
        let mut time_index = self.time_index.lock().unwrap();
        let keys_to_remove: Vec<u64> = time_index.range(..timestamp).map(|(k, _)| *k).collect();

        for key in keys_to_remove {
            time_index.remove(&key);
        }
    }

    /// 删除指定时间戳的索引项
    pub fn remove(&self, timestamp: u64) {
        #[cfg(feature = "std")]
        let mut time_index = self.time_index.write().unwrap();
        #[cfg(not(feature = "std"))]
        let mut time_index = self.time_index.lock().unwrap();
        time_index.remove(&timestamp);
    }
}
