//! HNSW (Hierarchical Navigable Small World) 索引
//!
//! 改进：
//! - 可配置的邻居数 M（支持 1024 维向量）
//! - 启发式邻居选择（提高搜索质量）
//! - 使用最小堆顺序搜索
//! - 修复空图/自举问题

use crate::platform::memset;
use crate::types::{DistanceType, VectorMetadata};
use crate::{RemDbError, Result};
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ptr::NonNull;

/// 简单的 XorShift 随机数生成器（用于 baremetal 环境）
#[cfg(not(feature = "std"))]
struct XorShiftRng {
    state: u64,
}

#[cfg(not(feature = "std"))]
impl XorShiftRng {
    fn new(seed: u64) -> Self {
        XorShiftRng {
            state: if seed == 0 { 0x123456789abcdef } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }
}

/// HNSW 节点结构
#[repr(C)]
pub struct HNSWNode {
    /// 向量在存储中的偏移量
    pub vector_offset: usize,
    /// 记录 ID
    pub record_id: u16,
    /// 每层的邻居节点列表
    pub neighbors: Vec<Vec<NonNull<HNSWNode>>>,
}

impl HNSWNode {
    /// 创建新的 HNSW 节点
    pub unsafe fn new(vector_offset: usize, record_id: u16, max_level: usize) -> Self {
        let neighbors = alloc::vec![Vec::new(); max_level + 1];
        HNSWNode {
            vector_offset,
            record_id,
            neighbors,
        }
    }

    /// 获取节点在指定层的邻居列表
    pub fn get_neighbors_at_level(&self, level: usize) -> &[NonNull<HNSWNode>] {
        match self.neighbors.get(level) {
            Some(n) => n.as_slice(),
            None => &[],
        }
    }

    /// 获取指定层邻居列表的可变引用
    pub fn get_neighbors_mut_at_level(&mut self, level: usize) -> Option<&mut Vec<NonNull<HNSWNode>>> {
        self.neighbors.get_mut(level)
    }

    /// 添加邻居节点到指定层
    pub fn add_neighbor_at_level(
        &mut self,
        level: usize,
        neighbor: NonNull<HNSWNode>,
    ) -> Result<()> {
        match self.neighbors.get_mut(level) {
            Some(n) => {
                n.push(neighbor);
                Ok(())
            }
            None => Err(RemDbError::OutOfMemory),
        }
    }

    /// 从指定层移除邻居节点
    pub fn remove_neighbor_at_level(&mut self, level: usize, neighbor: NonNull<HNSWNode>) {
        if let Some(n) = self.neighbors.get_mut(level) {
            n.retain(|&n| n != neighbor);
        }
    }
}

/// 距离-节点对
#[derive(Clone)]
struct DistNode {
    distance: f32,
    node: NonNull<HNSWNode>,
}

/// HNSW 索引结构
pub struct HNSWIndex {
    /// 向量元数据
    pub meta: VectorMetadata,
    /// 向量数据存储
    pub vectors: *mut f32,
    /// 最大层数
    pub max_level: usize,
    /// 当前入口点
    pub enter_point: Option<NonNull<HNSWNode>>,
    /// 每层的入口节点
    pub layer_enter_points: Vec<Option<NonNull<HNSWNode>>>,
    /// 节点池
    pub nodes: NonNull<HNSWNode>,
    /// 空闲节点列表
    pub free_nodes: Option<NonNull<HNSWNode>>,
    /// 最大节点数量
    pub max_nodes: usize,
    /// 当前节点数量
    pub node_count: usize,
    /// 自旋锁
    pub lock: u32,
    /// 每层最大邻居数（M）
    pub m: usize,
    /// 构建时的候选列表大小
    pub ef_construction: usize,
    /// 搜索时的候选列表大小
    pub ef_search: usize,
}

impl HNSWIndex {
    /// 创建新的 HNSW 索引
    pub unsafe fn new(
        meta: VectorMetadata,
        vectors: *mut f32,
        memory_start: *mut u8,
        max_nodes: usize,
    ) -> Result<Self> {
        // 计算最大层数
        let max_level = if max_nodes > 0 {
            (max_nodes as f64).ln().floor() as usize
        } else {
            0
        };

        let m = meta.hnsw_m as usize;
        let ef_construction = meta.hnsw_ef_construction as usize;
        let ef_search = meta.hnsw_ef_search as usize;

        // 初始化节点池
        let nodes = NonNull::new_unchecked(memory_start as *mut HNSWNode);

        // 初始化空闲节点链表
        let mut free_nodes = None;
        for i in (0..max_nodes).rev() {
            let node_ptr = nodes.as_ptr().add(i);
            let node = HNSWNode::new(0, 0, max_level);
            core::ptr::write(node_ptr, node);
            free_nodes = Some(NonNull::new_unchecked(node_ptr));
        }

        // 初始化层入口节点
        let mut layer_enter_points = Vec::with_capacity(max_level + 1);
        for _ in 0..=max_level {
            layer_enter_points.push(None);
        }

        Ok(HNSWIndex {
            meta,
            vectors,
            max_level,
            enter_point: None,
            layer_enter_points,
            nodes,
            free_nodes,
            max_nodes,
            node_count: 0,
            lock: 0,
            m,
            ef_construction,
            ef_search,
        })
    }

    /// 计算两个向量之间的距离
    unsafe fn calculate_distance(&self, vec1: *const f32, vec2: *const f32) -> f32 {
        let dim = self.meta.dimension as usize;
        match self.meta.distance_type {
            DistanceType::L2 => {
                let mut sum = 0.0f32;
                for i in 0..dim {
                    let diff = *vec1.add(i) - *vec2.add(i);
                    sum += diff * diff;
                }
                sum
            }
            DistanceType::InnerProduct => {
                let mut sum = 0.0f32;
                for i in 0..dim {
                    sum += *vec1.add(i) * *vec2.add(i);
                }
                -sum
            }
            DistanceType::Cosine => {
                let mut dot = 0.0f32;
                let mut norm1 = 0.0f32;
                let mut norm2 = 0.0f32;
                for i in 0..dim {
                    let v1 = *vec1.add(i);
                    let v2 = *vec2.add(i);
                    dot += v1 * v2;
                    norm1 += v1 * v1;
                    norm2 += v2 * v2;
                }
                let n1 = norm1.sqrt();
                let n2 = norm2.sqrt();
                if n1 == 0.0 || n2 == 0.0 {
                    -1.0
                } else {
                    -(dot / (n1 * n2))
                }
            }
        }
    }

    /// 生成随机层号
    fn generate_random_level(&self) -> usize {
        let mut level = 0;
        let p = 0.5; // 层概率衰减因子
        #[cfg(feature = "std")]
        while level < self.max_level && rand::random::<f64>() < p {
            level += 1;
        }

        #[cfg(not(feature = "std"))]
        {
            let seed = core::time::Instant::now().elapsed().as_nanos() as u64;
            let mut rng = XorShiftRng::new(seed);
            while level < self.max_level && rng.next_f64() < p {
                level += 1;
            }
        }

        level
    }

    /// 保存 HNSW 索引到文件
    #[cfg(feature = "std")]
    pub fn save<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        writer
            .write_all(&self.meta.hnsw_m.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        writer
            .write_all(&self.meta.hnsw_ef_construction.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        writer
            .write_all(&self.meta.hnsw_ef_search.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        writer
            .write_all(&self.max_level.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        writer
            .write_all(&self.node_count.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;

        let enter_point_offset = match self.enter_point {
            Some(point) => (unsafe { point.as_ptr().offset_from(self.nodes.as_ptr()) } as usize),
            None => usize::MAX,
        };
        writer
            .write_all(&enter_point_offset.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;

        writer
            .write_all(&self.layer_enter_points.len().to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        for &point in &self.layer_enter_points {
            let offset = match point {
                Some(p) => (unsafe { p.as_ptr().offset_from(self.nodes.as_ptr()) } as usize),
                None => usize::MAX,
            };
            writer
                .write_all(&offset.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
        }

        for i in 0..self.node_count {
            let node_ptr = unsafe { self.nodes.as_ptr().add(i) };
            let node = unsafe { &*node_ptr };
            writer
                .write_all(&node.vector_offset.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            writer
                .write_all(&node.record_id.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            writer
                .write_all(&node.neighbors.len().to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            for level_neighbors in &node.neighbors {
                writer
                    .write_all(&level_neighbors.len().to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
                for &neighbor in level_neighbors {
                    let offset = unsafe { neighbor.as_ptr().offset_from(self.nodes.as_ptr()) } as usize;
                    writer
                        .write_all(&offset.to_le_bytes())
                        .map_err(|_| RemDbError::FileIoError)?;
                }
            }
        }

        Ok(())
    }

    /// 从文件加载 HNSW 索引
    #[cfg(feature = "std")]
    pub unsafe fn load<R: std::io::Read>(
        mut meta: VectorMetadata,
        vectors: *mut f32,
        memory_start: *mut u8,
        max_nodes: usize,
        reader: &mut R,
    ) -> Result<Self> {
        let mut m_bytes = [0u8; 1];
        reader
            .read_exact(&mut m_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.hnsw_m = m_bytes[0];

        let mut efc_bytes = [0u8; 4];
        reader
            .read_exact(&mut efc_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.hnsw_ef_construction = u32::from_le_bytes(efc_bytes);

        let mut efs_bytes = [0u8; 4];
        reader
            .read_exact(&mut efs_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.hnsw_ef_search = u32::from_le_bytes(efs_bytes);

        let mut max_level_bytes = [0u8; 8];
        reader
            .read_exact(&mut max_level_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let max_level = usize::from_le_bytes(max_level_bytes);

        let mut node_count_bytes = [0u8; 8];
        reader
            .read_exact(&mut node_count_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let node_count = usize::from_le_bytes(node_count_bytes);

        let nodes = NonNull::new_unchecked(memory_start as *mut HNSWNode);

        let mut enter_point_offset_bytes = [0u8; 8];
        reader
            .read_exact(&mut enter_point_offset_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let enter_point_offset = usize::from_le_bytes(enter_point_offset_bytes);
        let enter_point = if enter_point_offset == usize::MAX {
            None
        } else {
            Some(NonNull::new_unchecked(nodes.as_ptr().add(enter_point_offset)))
        };

        let mut layer_enter_points_len_bytes = [0u8; 8];
        reader
            .read_exact(&mut layer_enter_points_len_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let layer_enter_points_len = usize::from_le_bytes(layer_enter_points_len_bytes);

        let mut layer_enter_points = Vec::with_capacity(layer_enter_points_len);
        for _ in 0..layer_enter_points_len {
            let mut offset_bytes = [0u8; 8];
            reader
                .read_exact(&mut offset_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let offset = usize::from_le_bytes(offset_bytes);
            let point = if offset == usize::MAX {
                None
            } else {
                Some(NonNull::new_unchecked(nodes.as_ptr().add(offset)))
            };
            layer_enter_points.push(point);
        }

        for i in 0..node_count {
            let node_ptr = nodes.as_ptr().add(i);

            let mut vector_offset_bytes = [0u8; 8];
            reader
                .read_exact(&mut vector_offset_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let vector_offset = usize::from_le_bytes(vector_offset_bytes);

            let mut record_id_bytes = [0u8; 2];
            reader
                .read_exact(&mut record_id_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let record_id = u16::from_le_bytes(record_id_bytes);

            let mut num_levels_bytes = [0u8; 8];
            reader
                .read_exact(&mut num_levels_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let num_levels = usize::from_le_bytes(num_levels_bytes);

            let mut neighbors: Vec<Vec<NonNull<HNSWNode>>> = Vec::with_capacity(num_levels);
            for _lvl in 0..num_levels {
                let mut level_len_bytes = [0u8; 8];
                reader
                    .read_exact(&mut level_len_bytes)
                    .map_err(|_| RemDbError::FileIoError)?;
                let level_len = usize::from_le_bytes(level_len_bytes);

                let mut level_neighbors = Vec::with_capacity(level_len);
                for _n in 0..level_len {
                    let mut offset_bytes = [0u8; 8];
                    reader
                        .read_exact(&mut offset_bytes)
                        .map_err(|_| RemDbError::FileIoError)?;
                    let offset = usize::from_le_bytes(offset_bytes);
                    level_neighbors.push(NonNull::new_unchecked(nodes.as_ptr().add(offset)));
                }
                neighbors.push(level_neighbors);
            }

            let node = HNSWNode {
                vector_offset,
                record_id,
                neighbors,
            };
            *node_ptr = node;
        }

        let m = meta.hnsw_m as usize;
        let ef_construction = meta.hnsw_ef_construction as usize;
        let ef_search = meta.hnsw_ef_search as usize;

        Ok(HNSWIndex {
            meta,
            vectors,
            max_level,
            enter_point,
            layer_enter_points,
            nodes,
            free_nodes: None,
            max_nodes,
            node_count,
            lock: 0,
            m,
            ef_construction,
            ef_search,
        })
    }

    /// 搜索单层（使用最小堆顺序）
    unsafe fn search_layer(
        &self,
        query_vec: *const f32,
        entry_point: NonNull<HNSWNode>,
        ef: usize,
        level: usize,
    ) -> Vec<(f32, NonNull<HNSWNode>)> {
        // 使用简单 Vec 作为候选列表和结果集
        let mut candidates: Vec<DistNode> = Vec::new();
        let mut results: Vec<DistNode> = Vec::new();
        let mut visited: Vec<NonNull<HNSWNode>> = Vec::new();

        const MAX_ITERATIONS: usize = 100000;
        let mut iteration_count = 0;

        // 初始化
        let entry_vec = self.vectors.add(entry_point.as_ref().vector_offset);
        let entry_dist = self.calculate_distance(query_vec, entry_vec);

        candidates.push(DistNode {
            distance: entry_dist,
            node: entry_point,
        });
        results.push(DistNode {
            distance: entry_dist,
            node: entry_point,
        });
        visited.push(entry_point);

        while !candidates.is_empty() {
            iteration_count += 1;
            if iteration_count > MAX_ITERATIONS {
                #[cfg(feature = "log")]
                crate::log::warn!("HNSW search_layer reached max iterations");
                break;
            }

            // 找到最近候选（最小堆）
            let mut best_idx = 0;
            for i in 1..candidates.len() {
                if candidates[i].distance < candidates[best_idx].distance {
                    best_idx = i;
                }
            }
            let current = candidates.swap_remove(best_idx);

            // 如果当前候选距离大于结果集中最远的，停止搜索
            if !results.is_empty() && current.distance > results[results.len() - 1].distance {
                break;
            }

            // 获取当前节点的邻居
            let neighbors = current.node.as_ref().get_neighbors_at_level(level);
            for &neighbor in neighbors {
                if visited.contains(&neighbor) {
                    continue;
                }
                visited.push(neighbor);

                let neighbor_vec = self.vectors.add(neighbor.as_ref().vector_offset);
                let neighbor_dist = self.calculate_distance(query_vec, neighbor_vec);

                let neighbor_dist_node = DistNode {
                    distance: neighbor_dist,
                    node: neighbor,
                };

                // 检查是否需要加入结果集
                if results.len() < ef || neighbor_dist < results[results.len() - 1].distance {
                    candidates.push(DistNode {
                        distance: neighbor_dist,
                        node: neighbor,
                    });
                    results.push(neighbor_dist_node);
                    // 按距离排序
                    results.sort_by(|a, b| {
                        a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal)
                    });
                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }

        results.into_iter().map(|dn| (dn.distance, dn.node)).collect()
    }

    /// 搜索最近邻（所有层）
    pub unsafe fn search(&self, query_vec: *const f32, k: usize) -> Result<Vec<(f32, u16)>> {
        // 如果索引为空，返回空结果
        let current_ep = match self.enter_point {
            Some(ep) => ep,
            None => return Ok(Vec::new()),
        };

        let mut current_point = current_ep;
        let mut current_level = self.max_level;

        // 从上到下遍历各层
        while current_level > 0 {
            let results = self.search_layer(query_vec, current_point, 1, current_level);
            if let Some(&(_dist, point)) = results.first() {
                current_point = point;
            }
            current_level -= 1;
        }

        // 在最底层搜索
        let ef = core::cmp::max(self.ef_search, k);
        let results = self.search_layer(query_vec, current_point, ef, 0);

        // 转换为记录 ID 和距离
        let k = core::cmp::min(k, results.len());
        let mut final_results: Vec<(f32, u16)> = Vec::with_capacity(k);
        for i in 0..k {
            if let Some(&(distance, node)) = results.get(i) {
                final_results.push((distance, node.as_ref().record_id));
            }
        }

        Ok(final_results)
    }

    /// 启发式邻居选择（HNSW 论文算法）
    unsafe fn select_neighbors_heuristic(
        &self,
        candidates: &[(f32, NonNull<HNSWNode>)],
        m: usize,
    ) -> Vec<NonNull<HNSWNode>> {
        if candidates.len() <= m {
            return candidates.iter().map(|&(_d, n)| n).collect();
        }

        // 按距离排序
        let mut sorted: Vec<(f32, NonNull<HNSWNode>)> = candidates.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

        let mut result: Vec<NonNull<HNSWNode>> = Vec::new();
        let mut queue: Vec<(f32, NonNull<HNSWNode>)> = sorted;

        while !queue.is_empty() && result.len() < m {
            let (_, node) = queue.remove(0);

            // 检查是否与已有结果过于接近
            let mut too_close = false;
            for &existing in &result {
                let existing_vec = self.vectors.add(existing.as_ref().vector_offset);
                let node_vec = self.vectors.add(node.as_ref().vector_offset);
                let dist = self.calculate_distance(node_vec, existing_vec);
                if dist < 0.001 {
                    // 非常接近，跳过
                    too_close = true;
                    break;
                }
            }

            if !too_close {
                result.push(node);
            }
        }

        result
    }

    /// 插入新节点
    pub unsafe fn insert(&mut self, vector_offset: usize, record_id: u16) -> Result<()> {
        // 获取向量数据
        let vec_ptr = self.vectors.add(vector_offset);
        // 生成随机层号
        let new_level = self.generate_random_level();

        // 分配节点
        if self.node_count >= self.max_nodes {
            return Err(RemDbError::OutOfMemory);
        }
        let node_ptr = self.nodes.as_ptr().add(self.node_count);
        core::ptr::write(
            node_ptr,
            HNSWNode::new(vector_offset, record_id, self.max_level),
        );
        let pool_node = NonNull::new_unchecked(node_ptr);

        // 处理第一个节点
        let mut entry_point = match self.enter_point {
            Some(point) => point,
            None => {
                self.enter_point = Some(pool_node);
                for i in 0..=new_level {
                    if let Some(ep) = self.layer_enter_points.get_mut(i) {
                        *ep = Some(pool_node);
                    }
                }
                self.node_count += 1;
                return Ok(());
            }
        };

        // 从上到下搜索插入位置
        let mut current_level = self.max_level;
        while current_level > new_level {
            let results = self.search_layer(vec_ptr, entry_point, 1, current_level);
            if let Some(&(_dist, point)) = results.first() {
                entry_point = point;
            }
            current_level -= 1;
        }

        // 在各层插入节点
        while current_level <= new_level {
            let ef_construction = core::cmp::max(self.ef_construction, self.m);
            let neighbors = self.search_layer(vec_ptr, entry_point, ef_construction, current_level);

            // 启发式选择 M 个最近邻
            let selected = self.select_neighbors_heuristic(&neighbors, self.m);

            // 双向连接
            let node_mut = &mut *node_ptr;
            for &neighbor in &selected {
                node_mut.add_neighbor_at_level(current_level, neighbor)?;
                let neighbor_ptr = neighbor.as_ptr();
                // 获取邻居的向量偏移量（在借用其邻居列表之前，通过原始指针读取）
                let neighbor_vec_offset = (*neighbor_ptr).vector_offset;
                let neighbor_mut = &mut *neighbor_ptr;
                // 如果邻居的邻居数超过 M，修剪
                if let Some(n) = neighbor_mut.get_neighbors_mut_at_level(current_level) {
                    if n.len() > self.m {
                        // 按距离排序，保留最近的 M 个
                        let mut with_dist: Vec<(f32, NonNull<HNSWNode>)> = Vec::new();
                        for &nn in n.iter() {
                            let nn_vec = self.vectors.add(nn.as_ref().vector_offset);
                            let n_vec = self.vectors.add(neighbor_vec_offset);
                            let d = self.calculate_distance(n_vec, nn_vec);
                            with_dist.push((d, nn));
                        }
                        with_dist.sort_by(|a, b| {
                            a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal)
                        });
                        n.clear();
                        for &(_d, nn) in with_dist.iter().take(self.m) {
                            n.push(nn);
                        }
                    }
                }
            }

            // 更新层入口点
            if let Some(ep) = self.layer_enter_points.get_mut(current_level) {
                if ep.is_none() {
                    *ep = Some(pool_node);
                }
            }

            current_level += 1;
        }

        self.node_count += 1;

        // 更新全局入口点
        if new_level >= self.max_level {
            self.enter_point = Some(pool_node);
        }

        Ok(())
    }

    /// 删除节点
    pub unsafe fn delete(&mut self, vector_offset: usize) -> Result<()> {
        // 查找要删除的节点
        let mut target_node = None;
        let mut target_node_idx = None;
        for i in 0..self.node_count {
            let node_ptr = self.nodes.as_ptr().add(i);
            let node = &*node_ptr;
            if node.vector_offset == vector_offset {
                target_node = Some(NonNull::new_unchecked(node_ptr));
                target_node_idx = Some(i);
                break;
            }
        }

        if let Some(target_node) = target_node {
            let target_idx = target_node_idx.ok_or(RemDbError::RecordNotFound)?;

            // 从所有节点的邻居列表中移除目标节点
            for i in 0..self.node_count {
                if i == target_idx {
                    continue;
                }
                let node_ptr = self.nodes.as_ptr().add(i);
                let node = &mut *node_ptr;
                let max_level = node.neighbors.len();
                for level in 0..max_level {
                    node.remove_neighbor_at_level(level, target_node);
                }
            }

            // 清空节点数据
            memset(
                target_node.as_ptr() as *mut u8,
                0,
                core::mem::size_of::<HNSWNode>(),
            );

            // 更新空闲节点列表
            let mut node = target_node;
            node.as_mut().neighbors.clear();
            let _next_free = self.free_nodes;
            self.free_nodes = Some(node);

            // 如果删除的是入口点，更新入口点
            if self.enter_point == Some(target_node) {
                // 找第一个非空节点作为新入口点
                let mut new_ep = None;
                for i in 0..self.node_count {
                    if i != target_idx {
                        new_ep = Some(NonNull::new_unchecked(self.nodes.as_ptr().add(i)));
                        break;
                    }
                }
                self.enter_point = new_ep;
            }

            Ok(())
        } else {
            Err(RemDbError::RecordNotFound)
        }
    }
}