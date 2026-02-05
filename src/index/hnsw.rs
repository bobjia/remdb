use crate::platform::memset;
use crate::types::{DistanceType, VectorMetadata};
use crate::{RemDbError, Result};
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ptr::NonNull;

/// 简单的XorShift随机数生成器（用于baremetal环境）
#[cfg(not(feature = "std"))]
struct XorShiftRng {
    state: u64,
}

#[cfg(not(feature = "std"))]
impl XorShiftRng {
    fn new(seed: u64) -> Self {
        XorShiftRng {
            state: if seed == 0 {
                0x123456789abcdef
            } else {
                seed
            },
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

/// HNSW节点结构
#[repr(C)]
pub struct HNSWNode {
    /// 向量在存储中的偏移量
    pub vector_offset: usize,
    /// 记录ID
    pub record_id: u16,
    /// 每层的邻居节点数量
    pub neighbor_counts: Vec<u8>,
    /// 邻居节点列表（按层组织）
    pub neighbors: Vec<NonNull<HNSWNode>>,
}

impl HNSWNode {
    /// 创建新的HNSW节点
    pub unsafe fn new(
        vector_offset: usize,
        record_id: u16,
        max_level: usize,
    ) -> Self {
        let mut neighbor_counts = Vec::with_capacity(max_level + 1);
        let neighbors = Vec::with_capacity((max_level + 1) * 32); // 每层最多32个邻居        
        for _ in 0..=max_level {
            neighbor_counts.push(0);
        }
        
        HNSWNode {
            vector_offset,
            record_id,
            neighbor_counts,
            neighbors,
        }
    }
    
    /// 获取节点在指定层的邻居列表
    pub unsafe fn get_neighbors_at_level(&self, level: usize) -> &[NonNull<HNSWNode>] {
        let start_offset = level * 32; // 每层最多32个邻居        
        let count = self.neighbor_counts[level] as usize;
        &self.neighbors[start_offset..start_offset + count]
    }
    
    /// 添加邻居节点到指定层
    pub unsafe fn add_neighbor_at_level(
        &mut self,
        level: usize,
        neighbor: NonNull<HNSWNode>,
    ) -> Result<()> {
        let start_offset = level * 32;
        let count = self.neighbor_counts[level] as usize;
        
        if count >= 32 {
            return Err(RemDbError::OutOfMemory);
        }
        
        self.neighbors[start_offset + count] = neighbor;
        self.neighbor_counts[level] += 1;
        
        Ok(())
    }
}

/// HNSW索引结构
pub struct HNSWIndex {
    /// 向量元数据
    pub meta: VectorMetadata,
    /// 向量数据存储
    pub vectors: *mut f32,
    /// 最大层数
    pub max_level: usize,
    /// 当前插入点
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
}

impl HNSWIndex {
    /// 创建新的HNSW索引
    pub unsafe fn new(
        meta: VectorMetadata,
        vectors: *mut f32,
        memory_start: *mut u8,
        max_nodes: usize,
    ) -> Result<Self> {
        // 计算最大层数
        let max_level = (max_nodes as f64).ln() as usize;
        
        // 初始化节点池
        let _node_size = core::mem::size_of::<HNSWNode>();
        let nodes = NonNull::new_unchecked(memory_start as *mut HNSWNode);
        
        // 初始化空闲节点链表
        let mut free_nodes = None;
        for i in (0..max_nodes).rev() {
            let node_ptr = nodes.as_ptr().add(i);
            // 初始化节点
            let _node = HNSWNode::new(0, 0, max_level);            
            // 链接到空闲列表
            let _next = free_nodes;
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
        })
    }
    
    /// 计算两个向量之间的距离
    unsafe fn calculate_distance(
        &self,
        vec1: *const f32,
        vec2: *const f32,
    ) -> f32 {
        match self.meta.distance_type {
            DistanceType::L2 => {
                // L2距离（欧几里得距离）
                let mut sum = 0.0;
                for i in 0..self.meta.dimension {
                    let diff = *vec1.add(i as usize) - *vec2.add(i as usize);
                    sum += diff * diff;
                }
                sum.sqrt()
            }
            DistanceType::InnerProduct => {
                // 内积
                let mut sum = 0.0;
                for i in 0..self.meta.dimension {
                    sum += *vec1.add(i as usize) * *vec2.add(i as usize);
                }
                -sum // 返回负数，因为内积越大相似度越高
            }
            DistanceType::Cosine => {
                // 余弦相似度
                let mut dot = 0.0;
                let mut norm1 = 0.0;
                let mut norm2 = 0.0;
                
                for i in 0..self.meta.dimension {
                    let v1 = *vec1.add(i as usize);
                    let v2 = *vec2.add(i as usize);
                    dot += v1 * v2;
                    norm1 += v1 * v1;
                    norm2 += v2 * v2;
                }
                
                let norm1 = norm1.sqrt();
                let norm2 = norm2.sqrt();
                
                if norm1 == 0.0 || norm2 == 0.0 {
                    -1.0 // 相似度最低
                } else {
                    -(dot / (norm1 * norm2)) // 返回负数，因为余弦相似度越大相似度越高
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
        // 简单的伪随机数生成器（用于baremetal环境）
        {
            // 使用当前时间作为种子
            let seed = core::time::Instant::now().elapsed().as_nanos() as u64;
            let mut rng = XorShiftRng::new(seed);
            while level < self.max_level && rng.next_f64() < p {
                level += 1;
            }
        }
        
        level
    }
    
    /// 保存HNSW索引到文件
    #[cfg(feature = "std")]
    pub fn save<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        use std::io::Write;
        
        // 保存向量元数据参数
        // 写入hnsw_m
        writer.write_all(&self.meta.hnsw_m.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入hnsw_ef_construction
        writer.write_all(&self.meta.hnsw_ef_construction.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入hnsw_ef_search
        writer.write_all(&self.meta.hnsw_ef_search.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存HNSW索引元数据
        // 写入最大层数
        writer.write_all(&self.max_level.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入当前节点数量
        writer.write_all(&self.node_count.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存入口点
        let enter_point_offset = match self.enter_point {
            Some(point) => {
                let offset = unsafe {
                    point.as_ptr().offset_from(self.nodes.as_ptr())
                } as usize;
                offset
            },
            None => usize::MAX,
        };
        writer.write_all(&enter_point_offset.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存每层入口节点
        writer.write_all(&self.layer_enter_points.len().to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        for &point in &self.layer_enter_points {
            let offset = match point {
                Some(p) => {
                    let offset = unsafe {
                        p.as_ptr().offset_from(self.nodes.as_ptr())
                    } as usize;
                    offset
                },
                None => usize::MAX,
            };
            writer.write_all(&offset.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
        }
        
        // 保存节点数据
        for i in 0..self.node_count {
            let node_ptr = unsafe { self.nodes.as_ptr().add(i) };
            let node = unsafe { &*node_ptr };
            
            // 写入向量偏移量
            writer.write_all(&node.vector_offset.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            // 写入记录ID
            writer.write_all(&node.record_id.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            
            // 写入邻居数量
            writer.write_all(&node.neighbor_counts.len().to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            for &count in &node.neighbor_counts {
                writer.write_all(&count.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
            }
            
            // 写入邻居节点
            for &neighbor in &node.neighbors {
                let offset = unsafe {
                    neighbor.as_ptr().offset_from(self.nodes.as_ptr())
                } as usize;
                writer.write_all(&offset.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
            }
        }
        
        Ok(())
    }
    
    /// 从文件加载HNSW索引
    #[cfg(feature = "std")]
    pub unsafe fn load<
        R: std::io::Read,
    >(
        mut meta: VectorMetadata,
        vectors: *mut f32,
        memory_start: *mut u8,
        max_nodes: usize,
        reader: &mut R,
    ) -> Result<Self> {
        use std::io::Read;
        
        // 读取向量元数据参数
        // 读取hnsw_m
        let mut m_bytes = [0u8; 1];
        reader.read_exact(&mut m_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.hnsw_m = m_bytes[0];
        
        // 读取hnsw_ef_construction
        let mut efc_bytes = [0u8; 4];
        reader.read_exact(&mut efc_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.hnsw_ef_construction = u32::from_le_bytes(efc_bytes);
        
        // 读取hnsw_ef_search
        let mut efs_bytes = [0u8; 4];
        reader.read_exact(&mut efs_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.hnsw_ef_search = u32::from_le_bytes(efs_bytes);
        
        // 读取HNSW索引元数据
        // 读取最大层数
        let mut max_level_bytes = [0u8; 8];
        reader.read_exact(&mut max_level_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let max_level = usize::from_le_bytes(max_level_bytes);
        
        // 读取当前节点数量
        let mut node_count_bytes = [0u8; 8];
        reader.read_exact(&mut node_count_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let node_count = usize::from_le_bytes(node_count_bytes);
        
        // 初始化节点池
        let _node_size = core::mem::size_of::<HNSWNode>();
        let nodes = NonNull::new_unchecked(memory_start as *mut HNSWNode);
        
        // 读取入口点
        let mut enter_point_offset_bytes = [0u8; 8];
        reader.read_exact(&mut enter_point_offset_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let enter_point_offset = usize::from_le_bytes(enter_point_offset_bytes);
        let enter_point = if enter_point_offset == usize::MAX {
            None
        } else {
            Some(NonNull::new_unchecked(nodes.as_ptr().add(enter_point_offset)))
        };
        
        // 读取每层入口节点
        let mut layer_enter_points_len_bytes = [0u8; 8];
        reader.read_exact(&mut layer_enter_points_len_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let layer_enter_points_len = usize::from_le_bytes(layer_enter_points_len_bytes);
        
        let mut layer_enter_points = Vec::with_capacity(layer_enter_points_len);
        for _ in 0..layer_enter_points_len {
            let mut offset_bytes = [0u8; 8];
            reader.read_exact(&mut offset_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let offset = usize::from_le_bytes(offset_bytes);
            let point = if offset == usize::MAX {
                None
            } else {
                Some(NonNull::new_unchecked(nodes.as_ptr().add(offset)))
            };
            layer_enter_points.push(point);
        }
        
        // 读取节点数据
        for i in 0..node_count {
            let node_ptr = nodes.as_ptr().add(i);
            
            // 读取向量偏移量
            let mut vector_offset_bytes = [0u8; 8];
            reader.read_exact(&mut vector_offset_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let vector_offset = usize::from_le_bytes(vector_offset_bytes);
            
            // 读取记录ID
            let mut record_id_bytes = [0u8; 2];
            reader.read_exact(&mut record_id_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let record_id = u16::from_le_bytes(record_id_bytes);
            
            // 读取邻居数量
            let mut neighbor_counts_len_bytes = [0u8; 8];
            reader.read_exact(&mut neighbor_counts_len_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let neighbor_counts_len = usize::from_le_bytes(neighbor_counts_len_bytes);
            
            let mut neighbor_counts = Vec::with_capacity(neighbor_counts_len);
            for _ in 0..neighbor_counts_len {
                let mut count_bytes = [0u8; 1];
                reader.read_exact(&mut count_bytes)
                    .map_err(|_| RemDbError::FileIoError)?;
                neighbor_counts.push(count_bytes[0]);
            }
            
            // 读取邻居节点
            let total_neighbors = neighbor_counts.iter().sum::<u8>() as usize;
            let mut neighbors = Vec::with_capacity(total_neighbors);
            for _ in 0..total_neighbors {
                let mut offset_bytes = [0u8; 8];
                reader.read_exact(&mut offset_bytes)
                    .map_err(|_| RemDbError::FileIoError)?;
                let offset = usize::from_le_bytes(offset_bytes);
                let neighbor = NonNull::new_unchecked(nodes.as_ptr().add(offset));
                neighbors.push(neighbor);
            }
            
            // 构建节点
            let node = HNSWNode {
                vector_offset,
                record_id,
                neighbor_counts,
                neighbors,
            };
            
            // 写入节点
            *node_ptr = node;
        }
        
        Ok(HNSWIndex {
            meta,
            vectors,
            max_level,
            enter_point,
            layer_enter_points,
            nodes,
            free_nodes: None, // 加载时不需要空闲节点列表
            max_nodes,
            node_count,
            lock: 0,
        })
    }
    
    /// 搜索最近邻（单个层）
    unsafe fn search_layer(
        &self,
        query_vec: *const f32,
        entry_point: NonNull<HNSWNode>,
        ef: usize,
        level: usize,
    ) -> Vec<(f32, NonNull<HNSWNode>)> {
        let mut visited = Vec::new();
        let mut candidates = Vec::new();
        let mut results = Vec::new();
        
        // 初始化
        let entry_node = entry_point.as_ref();
        let entry_vec = self.vectors.add(entry_node.vector_offset);
        let distance = self.calculate_distance(query_vec, entry_vec);        
        candidates.push((distance, entry_point));
        results.push((distance, entry_point));        
        while let Some((current_dist, current_node)) = candidates.pop() {
            // 更新结果列表
            if results.len() < ef || current_dist < results.last().unwrap().0 {
                // 获取当前节点的邻居
                let neighbors = current_node.as_ref().get_neighbors_at_level(level);                
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.push(neighbor);                        
                        let neighbor_node = neighbor.as_ref();
                        let neighbor_vec = self.vectors.add(neighbor_node.vector_offset);
                        let neighbor_dist = self.calculate_distance(query_vec, neighbor_vec);                        
                        if results.len() < ef || neighbor_dist < results.last().unwrap().0 {
                            candidates.push((neighbor_dist, neighbor));
                            results.push((neighbor_dist, neighbor));                            
                            // 按距离排序并限制结果数量
                            results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }
        
        results
    }
    
    /// 搜索最近邻（所有层）
    pub unsafe fn search(
        &self,
        query_vec: *const f32,
        _k: usize,
    ) -> Result<Vec<(f32, u16)>> {
        if self.enter_point.is_none() {
            return Err(RemDbError::RecordNotFound);
        }
        
        let mut current_point = self.enter_point.unwrap();
        let mut current_level = self.max_level;
        
        // 从上到下遍历各层        
        while current_level > 0 {
            let results = self.search_layer(query_vec, current_point, 1, current_level);
            if !results.is_empty() {
                current_point = results[0].1;
            }
            current_level -= 1;
        }
        
        // 在最底层进行精确搜索        
        let results = self.search_layer(query_vec, current_point, self.meta.hnsw_ef_search as usize, 0);        
        // 转换为记录ID和距离        
        let mut final_results = Vec::new();
        for (distance, node) in results {
            final_results.push((distance, node.as_ref().record_id));
        }
        
        Ok(final_results)
    }
    
    /// 插入新节点
    pub unsafe fn insert(
        &mut self,
        vector_offset: usize,
        record_id: u16,
    ) -> Result<()> {
        // 获取向量数据        
        let vec_ptr = self.vectors.add(vector_offset);        
        // 生成随机层号        
        let new_level = self.generate_random_level();        
        // 创建新节点        
        let mut new_node = HNSWNode::new(vector_offset, record_id, self.max_level);        
        // 搜索路径        
        let mut entry_point = match self.enter_point {
            Some(point) => point,            None => {
                // 第一个节点                
                let node_ptr = self.nodes.as_ptr().add(self.node_count);                
                *node_ptr = new_node;
                let node = NonNull::new_unchecked(node_ptr);                
                self.enter_point = Some(node);                
                for i in 0..=new_level {
                    self.layer_enter_points[i] = Some(node);
                }
                
                self.node_count += 1;
                return Ok(());
            }
        };
        
        let mut current_level = self.max_level;
        let _visited: Vec<NonNull<HNSWNode>> = Vec::new();        
        // 从上到下搜索插入位置        
        while current_level > new_level {
            let results = self.search_layer(vec_ptr, entry_point, 1, current_level);            
            if !results.is_empty() {
                entry_point = results[0].1;
            }
            current_level -= 1;
        }
        
        // 在各层插入节点        
        while current_level <= new_level {
            // 搜索当前层的最近邻            
            let ef_construction = self.meta.hnsw_ef_construction as usize;
            let neighbors = self.search_layer(vec_ptr, entry_point, ef_construction, current_level);            
            // 选择M个最近邻            
            let m = self.meta.hnsw_m as usize;
            let selected_neighbors = neighbors.iter().take(m).map(|&(_d, n)| n).collect::<Vec<_>>();            
            // 将新节点连接到选中的邻居            
            for &neighbor in &selected_neighbors {
                new_node.add_neighbor_at_level(current_level, neighbor)?;                
                // 双向连接                
                let neighbor_ptr = neighbor.as_ptr();
                let neighbor_mut = unsafe { &mut *neighbor_ptr };
                neighbor_mut.add_neighbor_at_level(current_level, NonNull::new_unchecked(&mut new_node))?;            }
            
            // 更新层入口点            
            if self.layer_enter_points[current_level].is_none() {
                self.layer_enter_points[current_level] = Some(NonNull::new_unchecked(&mut new_node));
            }
            
            current_level += 1;
        }
        
        // 将新节点添加到节点池        
        let node_ptr = self.nodes.as_ptr().add(self.node_count);        
        *node_ptr = new_node;
        self.node_count += 1;
        
        // 更新全局入口点        
        if new_level >= self.max_level {
            self.enter_point = Some(NonNull::new_unchecked(node_ptr));
        }
        
        Ok(())
    }
    
    /// 删除节点
    pub unsafe fn delete(
        &mut self,
        vector_offset: usize,
    ) -> Result<()> {
        // 查找要删除的节点        
        let mut target_node = None;
        let mut target_node_idx = None;
        for i in 0..self.node_count {
            let node_ptr = self.nodes.as_ptr().add(i);            
            let node = node_ptr.as_ref().unwrap();
            if node.vector_offset == vector_offset {
                target_node = Some(NonNull::new_unchecked(node_ptr));                
                target_node_idx = Some(i);
                break;
            }
        }
        
        if let Some(target_node) = target_node {
            // 1. 更新图结构，移除指向该节点的连接
            // 遍历所有节点，移除对目标节点的引用
            for i in 0..self.node_count {
                let node_ptr = self.nodes.as_ptr().add(i);
                let node = node_ptr.as_mut().unwrap();
                
                // 遍历每层
                for level in 0..=self.max_level {
                    // 获取当前层的邻居列表
                    let start_offset = level * 32;
                    let count = node.neighbor_counts[level] as usize;
                    
                    // 查找并移除目标节点
                    let mut new_neighbors = Vec::new();
                    let mut new_count = 0;
                    
                    for j in 0..count {
                        let neighbor = node.neighbors[start_offset + j];
                        if neighbor != target_node {
                            new_neighbors.push(neighbor);
                            new_count += 1;
                        }
                    }
                    
                    // 更新邻居列表
                    if new_count < count {
                        // 复制新邻居列表
                        for (j, &neighbor) in new_neighbors.iter().enumerate() {
                            node.neighbors[start_offset + j] = neighbor;
                        }
                        // 更新邻居数量
                        node.neighbor_counts[level] = new_count as u8;
                    }
                }
            }
            
            // 2. 清空节点数据            
            memset(target_node.as_ptr() as *mut u8, 0, core::mem::size_of::<HNSWNode>());            
            
            // 3. 更新空闲节点列表            
            let mut node = target_node;
            node.as_mut().neighbors.clear();            
            node.as_mut().neighbor_counts.clear();            
            let next_free = self.free_nodes;            
            self.free_nodes = Some(node);            
            
            Ok(())
        } else {
            Err(RemDbError::RecordNotFound)
        }
    }
}
