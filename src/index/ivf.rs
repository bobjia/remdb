use crate::types::{DistanceType, VectorMetadata};
use crate::{RemDbError, Result};
use alloc::vec::Vec;
use core::cmp::Ordering;

/// IVF_FLAT簇结构
#[repr(C)]
pub struct IVFCluster {
    /// 簇中心向量
    pub centroid: Vec<f32>,
    /// 属于该簇的向量数量
    pub vector_count: u32,
    /// 向量偏移量列表
    pub vector_offsets: Vec<usize>,
    /// 记录ID列表
    pub record_ids: Vec<u16>,
}

impl IVFCluster {
    /// 创建新的IVF簇
    pub fn new(dimension: u16) -> Self {
        let centroid = vec![0.0; dimension as usize];        
        IVFCluster {
            centroid,
            vector_count: 0,
            vector_offsets: Vec::new(),
            record_ids: Vec::new(),
        }
    }
    
    /// 添加向量到簇
    pub fn add_vector(
        &mut self,
        vector_offset: usize,
        record_id: u16,
    ) -> Result<()> {
        self.vector_offsets.push(vector_offset);        
        self.record_ids.push(record_id);        
        self.vector_count += 1;
        
        Ok(())
    }
}

/// IVF_FLAT索引结构
pub struct IVFIndex {
    /// 向量元数据
    pub meta: VectorMetadata,
    /// 向量数据存储
    pub vectors: *mut f32,
    /// 簇列表
    pub clusters: Vec<IVFCluster>,
    /// 簇数量
    pub nlist: u32,
    /// 搜索时检查的簇数量
    pub nprobe: u32,
    /// 向量维度
    pub dimension: u16,
    /// 总向量数量
    pub vector_count: u32,
    /// 自旋锁
    pub lock: u32,
}

impl IVFIndex {
    /// 创建新的IVF_FLAT索引
    pub unsafe fn new(
        meta: VectorMetadata,
        vectors: *mut f32,
        nlist: u32,
        nprobe: u32,
    ) -> Result<Self> {
        let dimension = meta.dimension;
        
        // 初始化簇列表        
        let mut clusters = Vec::with_capacity(nlist as usize);        
        for _ in 0..nlist {
            clusters.push(IVFCluster::new(dimension));
        }
        
        Ok(IVFIndex {
            meta,
            vectors,
            clusters,
            nlist,
            nprobe,
            dimension,
            vector_count: 0,
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
                for i in 0..self.dimension {
                    let diff = *vec1.add(i as usize) - *vec2.add(i as usize);
                    sum += diff * diff;
                }
                sum.sqrt()
            }
            DistanceType::InnerProduct => {
                // 内积
                let mut sum = 0.0;
                for i in 0..self.dimension {
                    sum += *vec1.add(i as usize) * *vec2.add(i as usize);
                }
                -sum // 返回负数，因为内积越大相似度越高
            }
            DistanceType::Cosine => {
                // 余弦相似度
                let mut dot = 0.0;
                let mut norm1 = 0.0;
                let mut norm2 = 0.0;
                
                for i in 0..self.dimension {
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
    
    /// 保存IVF索引到文件
    #[cfg(feature = "std")]
    pub fn save<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        use std::io::Write;
        
        // 保存向量元数据参数
        // 写入ivf_nlist
        writer.write_all(&self.meta.ivf_nlist.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入ivf_nprobe
        writer.write_all(&self.meta.ivf_nprobe.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存IVF索引元数据
        // 写入向量维度
        writer.write_all(&self.dimension.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入簇数量
        writer.write_all(&self.nlist.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入搜索簇数量
        writer.write_all(&self.nprobe.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        // 写入总向量数量
        writer.write_all(&self.vector_count.to_le_bytes())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 保存簇数据
        for cluster in &self.clusters {
            // 保存簇中心
            writer.write_all(&cluster.centroid.len().to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            for &value in &cluster.centroid {
                writer.write_all(&value.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
            }
            
            // 保存向量数量
            writer.write_all(&cluster.vector_count.to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            
            // 保存向量偏移量列表
            writer.write_all(&cluster.vector_offsets.len().to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            for &offset in &cluster.vector_offsets {
                writer.write_all(&offset.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
            }
            
            // 保存记录ID列表
            writer.write_all(&cluster.record_ids.len().to_le_bytes())
                .map_err(|_| RemDbError::FileIoError)?;
            for &record_id in &cluster.record_ids {
                writer.write_all(&record_id.to_le_bytes())
                    .map_err(|_| RemDbError::FileIoError)?;
            }
        }
        
        Ok(())
    }
    
    /// 从文件加载IVF索引
    #[cfg(feature = "std")]
    pub unsafe fn load<
        R: std::io::Read,
    >(
        mut meta: VectorMetadata,
        vectors: *mut f32,
        reader: &mut R,
    ) -> Result<Self> {
        use std::io::Read;
        
        // 读取向量元数据参数
        // 读取ivf_nlist
        let mut nlist_bytes = [0u8; 4];
        reader.read_exact(&mut nlist_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.ivf_nlist = u32::from_le_bytes(nlist_bytes);
        
        // 读取ivf_nprobe
        let mut nprobe_bytes = [0u8; 4];
        reader.read_exact(&mut nprobe_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        meta.ivf_nprobe = u32::from_le_bytes(nprobe_bytes);
        
        // 读取IVF索引元数据
        // 读取向量维度
        let mut dimension_bytes = [0u8; 2];
        reader.read_exact(&mut dimension_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let dimension = u16::from_le_bytes(dimension_bytes);
        
        // 读取簇数量
        let mut nlist_bytes = [0u8; 4];
        reader.read_exact(&mut nlist_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let nlist = u32::from_le_bytes(nlist_bytes);
        
        // 读取搜索簇数量
        let mut nprobe_bytes = [0u8; 4];
        reader.read_exact(&mut nprobe_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let nprobe = u32::from_le_bytes(nprobe_bytes);
        
        // 读取总向量数量
        let mut vector_count_bytes = [0u8; 8];
        reader.read_exact(&mut vector_count_bytes)
            .map_err(|_| RemDbError::FileIoError)?;
        let vector_count = u64::from_le_bytes(vector_count_bytes);
        
        // 加载簇数据
        let mut clusters = Vec::with_capacity(nlist as usize);
        for _ in 0..nlist {
            // 读取簇中心
            let mut centroid_len_bytes = [0u8; 4];
            reader.read_exact(&mut centroid_len_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let centroid_len = u32::from_le_bytes(centroid_len_bytes) as usize;
            
            let mut centroid = Vec::with_capacity(centroid_len);
            for _ in 0..centroid_len {
                let mut value_bytes = [0u8; 4];
                reader.read_exact(&mut value_bytes)
                    .map_err(|_| RemDbError::FileIoError)?;
                let value = f32::from_le_bytes(value_bytes);
                centroid.push(value);
            }
            
            // 读取向量数量
            let mut vec_count_bytes = [0u8; 4];
            reader.read_exact(&mut vec_count_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let vec_count = u32::from_le_bytes(vec_count_bytes);
            
            // 读取向量偏移量列表
            let mut offsets_len_bytes = [0u8; 8];
            reader.read_exact(&mut offsets_len_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let offsets_len = usize::from_le_bytes(offsets_len_bytes);
            
            let mut vector_offsets = Vec::with_capacity(offsets_len);
            for _ in 0..offsets_len {
                let mut offset_bytes = [0u8; 8];
                reader.read_exact(&mut offset_bytes)
                    .map_err(|_| RemDbError::FileIoError)?;
                let offset = usize::from_le_bytes(offset_bytes);
                vector_offsets.push(offset);
            }
            
            // 读取记录ID列表
            let mut record_ids_len_bytes = [0u8; 8];
            reader.read_exact(&mut record_ids_len_bytes)
                .map_err(|_| RemDbError::FileIoError)?;
            let record_ids_len = usize::from_le_bytes(record_ids_len_bytes);
            
            let mut record_ids = Vec::with_capacity(record_ids_len);
            for _ in 0..record_ids_len {
                let mut record_id_bytes = [0u8; 2];
                reader.read_exact(&mut record_id_bytes)
                    .map_err(|_| RemDbError::FileIoError)?;
                let record_id = u16::from_le_bytes(record_id_bytes);
                record_ids.push(record_id);
            }
            
            // 构建簇
            let cluster = IVFCluster {
                centroid,
                vector_count: vec_count,
                vector_offsets,
                record_ids,
            };
            
            clusters.push(cluster);
        }
        
        Ok(IVFIndex {
            meta,
            vectors,
            clusters,
            nlist,
            nprobe,
            dimension,
            vector_count: vector_count as u32,
            lock: 0,
        })
    }
    
    /// 查找向量所属的簇
    unsafe fn find_closest_cluster(&self, vec_ptr: *const f32) -> usize {
        let mut closest_cluster = 0;
        let mut min_distance = f32::MAX;
        
        for (i, cluster) in self.clusters.iter().enumerate() {
            let cluster_ptr = cluster.centroid.as_ptr();            
            let distance = self.calculate_distance(vec_ptr, cluster_ptr);            
            if distance < min_distance {
                min_distance = distance;
                closest_cluster = i;
            }
        }
        
        closest_cluster
    }
    
    /// K-means聚类算法
    pub unsafe fn train(
        &mut self,
        vectors: &[*const f32],
        vector_offsets: &[usize],
        record_ids: &[u16],
        max_iter: u32,
    ) -> Result<()> {
        let dimension = self.dimension as usize;
        let nlist = self.nlist as usize;
        
        // 验证输入长度是否一致
        if vectors.len() != vector_offsets.len() || vectors.len() != record_ids.len() {
            return Err(RemDbError::InternalError);
        }
        
        // 初始化簇中心（随机选择向量）        
        for i in 0..nlist {
            if i < vectors.len() {
                let vec_ptr = vectors[i];                
                for j in 0..dimension {
                    self.clusters[i].centroid[j] = *vec_ptr.add(j);
                }
            }
        }
        
        for _ in 0..max_iter {
            // 重置簇计数和偏移量列表            
            for cluster in &mut self.clusters {
                cluster.vector_count = 0;
                cluster.vector_offsets.clear();                
                cluster.record_ids.clear();
            }
            
            // 分配向量到簇            
            for (i, &vec_ptr) in vectors.iter().enumerate() {
                let cluster_idx = self.find_closest_cluster(vec_ptr);                
                // 存储向量偏移量和记录ID
                let cluster = &mut self.clusters[cluster_idx];
                cluster.vector_offsets.push(vector_offsets[i]);
                cluster.record_ids.push(record_ids[i]);
                cluster.vector_count += 1;
            }
            
            // 更新簇中心            
            for (_i, cluster) in self.clusters.iter_mut().enumerate() {
                if cluster.vector_count > 0 {
                    // 计算新的簇中心                    
                    let mut new_centroid = vec![0.0; dimension];                    
                    for &offset in &cluster.vector_offsets {
                        let vec_ptr = self.vectors.add(offset);                        
                        for j in 0..dimension {
                            new_centroid[j] += *vec_ptr.add(j);
                        }
                    }
                    
                    // 归一化簇中心                    
                    let count = cluster.vector_count as f32;
                    for j in 0..dimension {
                        new_centroid[j] /= count;
                    }
                    
                    // 检查簇中心是否收敛                    
                    let mut converged = true;
                    for j in 0..dimension {
                        if (new_centroid[j] - cluster.centroid[j]).abs() > 1e-6 {
                            converged = false;
                            break;
                        }
                    }
                    
                    if converged {
                        // 簇中心已收敛，提前退出                        
                        break;
                    }
                    
                    // 更新簇中心                    
                    cluster.centroid = new_centroid;
                }
            }
        }
        
        Ok(())
    }
    
    /// 搜索最近邻
    pub unsafe fn search(
        &self,
        query_vec: *const f32,
        k: usize,
    ) -> Result<Vec<(f32, u16)>> {
        // 1. 计算查询向量与所有簇中心的距离        
        let mut cluster_distances = Vec::new();        
        for (i, cluster) in self.clusters.iter().enumerate() {
            let centroid_ptr = cluster.centroid.as_ptr();            
            let distance = self.calculate_distance(query_vec, centroid_ptr);            
            cluster_distances.push((distance, i));
        }
        
        // 2. 按距离排序簇中心        
        cluster_distances.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));        
        // 3. 选择前nprobe个簇        
        let nprobe = self.nprobe as usize;        
        let selected_clusters = cluster_distances.iter().take(nprobe).map(|&(_d, i)| i);        
        // 4. 在选中的簇中搜索最近邻        
        let mut results = Vec::new();        
        for cluster_idx in selected_clusters {
            let cluster = &self.clusters[cluster_idx];            
            for (i, &vector_offset) in cluster.vector_offsets.iter().enumerate() {
                let vec_ptr = self.vectors.add(vector_offset);                
                let distance = self.calculate_distance(query_vec, vec_ptr);                
                let record_id = cluster.record_ids[i];                
                results.push((distance, record_id));
            }
        }
        
        // 5. 按距离排序并返回前k个结果        
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));        
        let final_results = results.iter().take(k).cloned().collect();        
        Ok(final_results)
    }
    
    /// 插入新向量
    pub unsafe fn insert(
        &mut self,
        vector_offset: usize,
        record_id: u16,
    ) -> Result<()> {
        // 获取向量数据        
        let vec_ptr = self.vectors.add(vector_offset);        
        // 查找最接近的簇        
        let cluster_idx = self.find_closest_cluster(vec_ptr);        
        // 添加向量到簇        
        self.clusters[cluster_idx].add_vector(vector_offset, record_id)?;        
        // 更新总向量数量        
        self.vector_count += 1;
        
        Ok(())
    }
    
    /// 删除向量
    pub unsafe fn delete(
        &mut self,
        vector_offset: usize,
    ) -> Result<()> {
        // 查找向量所属的簇        
        let vec_ptr = self.vectors.add(vector_offset);        
        let cluster_idx = self.find_closest_cluster(vec_ptr);        
        let cluster = &mut self.clusters[cluster_idx];        
        // 查找向量在簇中的位置        
        if let Some(pos) = cluster.vector_offsets.iter().position(|&offset| offset == vector_offset) {
            // 从列表中移除            
            cluster.vector_offsets.remove(pos);            
            cluster.record_ids.remove(pos);            
            cluster.vector_count -= 1;
            
            // 更新总向量数量            
            self.vector_count -= 1;
            
            Ok(())
        } else {
            Err(RemDbError::RecordNotFound)
        }
    }
}
