//! Product Quantization (PQ) 实现
//!
//! 将高维向量压缩为短码（通常每子向量 8 位）。
//! 对于 1024 维向量，使用 M=32 个子量化器可将 4096 字节压缩为 32 字节。
//! 距离计算使用非对称距离计算（ADC）：预计算查询向量与每子码本的距离表，
//! 然后查表累加，避免浮点运算。

use crate::types::{DistanceType, Result};
use crate::RemDbError;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// PQ 码本（每个子空间一个码本）
#[repr(C)]
pub struct PQCodebook {
    /// 子空间数量（M）
    pub m: usize,
    /// 每个子空间的码本大小（通常为 256，即 8 位）
    pub k: usize,
    /// 每个子向量的维度
    pub sub_dim: usize,
    /// 码本数据：形状为 [M][K][sub_dim] 的平坦数组
    pub centroids: Vec<f32>,
    /// 距离度量类型
    pub distance_type: DistanceType,
}

impl PQCodebook {
    /// 创建新的 PQ 码本
    pub fn new(m: usize, k: usize, sub_dim: usize, distance_type: DistanceType) -> Self {
        let centroids = vec![0.0; m * k * sub_dim];
        PQCodebook {
            m,
            k,
            sub_dim,
            centroids,
            distance_type,
        }
    }

    /// 获取指定子空间、指定码字的质心
    pub fn get_centroid(&self, sub_idx: usize, code: usize) -> &[f32] {
        let base = (sub_idx * self.k + code) * self.sub_dim;
        &self.centroids[base..base + self.sub_dim]
    }

    /// 获取指定子空间、指定码字的质心（可变）
    pub fn get_centroid_mut(&mut self, sub_idx: usize, code: usize) -> &mut [f32] {
        let base = (sub_idx * self.k + code) * self.sub_dim;
        &mut self.centroids[base..base + self.sub_dim]
    }
}

/// 乘积量化器
pub struct ProductQuantizer {
    /// 码本
    pub codebook: PQCodebook,
    /// 总维度
    pub dimension: usize,
    /// 子空间数量
    pub m: usize,
    /// 每个子空间的比特数
    pub nbits: usize,
}

impl ProductQuantizer {
    /// 创建新的乘积量化器
    ///
    /// # 参数
    /// - `dimension`: 向量总维度
    /// - `m`: 子空间数量（必须能整除 dimension）
    /// - `nbits`: 每个子空间的比特数（通常为 8，即 256 个码字）
    /// - `distance_type`: 距离度量类型
    pub fn new(
        dimension: usize,
        m: usize,
        nbits: usize,
        distance_type: DistanceType,
    ) -> Result<Self> {
        if m == 0 || dimension % m != 0 {
            return Err(RemDbError::InvalidData(
                "dimension must be divisible by m",
            ));
        }
        if nbits > 8 || nbits < 1 {
            return Err(RemDbError::InvalidData("nbits must be 1..=8"));
        }
        let k = 1 << nbits; // 码本大小
        let sub_dim = dimension / m;
        let codebook = PQCodebook::new(m, k, sub_dim, distance_type);
        Ok(ProductQuantizer {
            codebook,
            dimension,
            m,
            nbits,
        })
    }

    /// 使用 k-means 训练码本
    ///
    /// # 参数
    /// - `data`: 训练数据，形状为 [n][dimension] 的平坦数组
    /// - `n`: 数据点数量
    /// - `max_iter`: 最大迭代次数
    pub fn train(&mut self, data: &[f32], n: usize, max_iter: usize) -> Result<()> {
        let m = self.m;
        let k = self.k();
        let sub_dim = self.codebook.sub_dim;
        let dimension = self.dimension;
        let ksize = self.k();

        if n < ksize {
            return Err(RemDbError::InvalidData(
                "not enough training data for k-means",
            ));
        }

        // 对每个子空间独立训练 k-means
        for sub_idx in 0..m {
            // 提取该子空间的所有子向量
            let mut sub_vectors = Vec::with_capacity(n);
            for i in 0..n {
                let base = i * dimension + sub_idx * sub_dim;
                sub_vectors.extend_from_slice(&data[base..base + sub_dim]);
            }

            // 初始化码本：随机选择 k 个数据点作为初始质心
            let mut rng_seed = (sub_idx as u64).wrapping_mul(0x9e3779b97f4a7c15);
            let centroid_base = sub_idx * ksize * sub_dim;
            for c in 0..ksize {
                let idx = (self.xorshift(&mut rng_seed) as usize) % n;
                let src = idx * sub_dim;
                let dst = centroid_base + c * sub_dim;
                for j in 0..sub_dim {
                    self.codebook.centroids[dst + j] = sub_vectors[src + j];
                }
            }

            // k-means 迭代
            let mut assignments = vec![0usize; n];
            for _iter in 0..max_iter {
                // 分配阶段：每个点到最近质心
                let mut changed = false;
                for i in 0..n {
                    let base = i * sub_dim;
                    let mut best_c = 0;
                    let mut best_dist = f32::MAX;
                    for c in 0..ksize {
                        let cb = centroid_base + c * sub_dim;
                        let dist = self.subvector_distance(
                            &sub_vectors[base..base + sub_dim],
                            &self.codebook.centroids[cb..cb + sub_dim],
                        );
                        if dist < best_dist {
                            best_dist = dist;
                            best_c = c;
                        }
                    }
                    if assignments[i] != best_c {
                        assignments[i] = best_c;
                        changed = true;
                    }
                }

                if !changed {
                    break;
                }

                // 更新阶段：重新计算质心
                let mut counts = vec![0usize; ksize];
                let mut new_centroids = vec![0.0f32; ksize * sub_dim];

                for i in 0..n {
                    let c = assignments[i];
                    let base = i * sub_dim;
                    let cb = c * sub_dim;
                    counts[c] += 1;
                    for j in 0..sub_dim {
                        new_centroids[cb + j] += sub_vectors[base + j];
                    }
                }

                for c in 0..ksize {
                    if counts[c] > 0 {
                        let inv = 1.0 / counts[c] as f32;
                        let cb = c * sub_dim;
                        for j in 0..sub_dim {
                            self.codebook.centroids[centroid_base + cb + j] =
                                new_centroids[cb + j] * inv;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 对单个向量编码
    pub fn encode(&self, vector: &[f32]) -> Result<Vec<u8>> {
        if vector.len() < self.dimension {
            return Err(RemDbError::TypeMismatch);
        }
        let m = self.m;
        let k = self.k();
        let sub_dim = self.codebook.sub_dim;
        let mut codes = vec![0u8; m];

        for sub_idx in 0..m {
            let base = sub_idx * sub_dim;
            let sub_vec = &vector[base..base + sub_dim];
            let mut best_c = 0u8;
            let mut best_dist = f32::MAX;
            let cb_base = (sub_idx * k) * sub_dim;
            for c in 0..k {
                let cb = cb_base + c * sub_dim;
                let dist = self.subvector_distance(sub_vec, &self.codebook.centroids[cb..cb + sub_dim]);
                if dist < best_dist {
                    best_dist = dist;
                    best_c = c as u8;
                }
            }
            codes[sub_idx] = best_c;
        }

        Ok(codes)
    }

    /// 批量编码向量
    pub fn encode_batch(&self, data: &[f32], n: usize) -> Result<Vec<u8>> {
        let m = self.m;
        let codes_len = n * m;
        let mut all_codes = vec![0u8; codes_len];

        for i in 0..n {
            let vec_start = i * self.dimension;
            let codes = self.encode(&data[vec_start..vec_start + self.dimension])?;
            let code_start = i * m;
            for j in 0..m {
                all_codes[code_start + j] = codes[j];
            }
        }

        Ok(all_codes)
    }

    /// 预计算查询向量与所有码字的距离表
    /// 返回形状为 [M][K] 的平坦数组
    pub fn compute_distance_table(&self, query: &[f32]) -> Vec<f32> {
        let m = self.m;
        let k = self.k();
        let sub_dim = self.codebook.sub_dim;
        let mut table = vec![0.0f32; m * k];

        for sub_idx in 0..m {
            let q_base = sub_idx * sub_dim;
            let query_sub = &query[q_base..q_base + sub_dim];
            let cb_base = (sub_idx * k) * sub_dim;
            for c in 0..k {
                let cb = cb_base + c * sub_dim;
                table[sub_idx * k + c] =
                    self.subvector_distance(query_sub, &self.codebook.centroids[cb..cb + sub_dim]);
            }
        }

        table
    }

    /// 使用预计算的距离表计算距离
    pub fn compute_distance_from_table(table: &[f32], codes: &[u8], k: usize) -> f32 {
        let mut dist = 0.0f32;
        for (i, &code) in codes.iter().enumerate() {
            dist += table[i * k + code as usize];
        }
        dist
    }

    /// 非对称距离计算（ADC）：查询向量 vs PQ 编码
    pub fn compute_adc(&self, query: &[f32], codes: &[u8]) -> Result<f32> {
        if codes.len() < self.m {
            return Err(RemDbError::TypeMismatch);
        }
        let table = self.compute_distance_table(query);
        Ok(Self::compute_distance_from_table(&table, codes, self.k()))
    }

    /// 解码 PQ 码为近似向量（用于调试/可视化）
    pub fn decode(&self, codes: &[u8]) -> Vec<f32> {
        let m = self.m;
        let sub_dim = self.codebook.sub_dim;
        let mut vec = vec![0.0f32; m * sub_dim];

        for sub_idx in 0..m {
            let code = codes[sub_idx] as usize;
            let centroid = self.codebook.get_centroid(sub_idx, code);
            let base = sub_idx * sub_dim;
            for j in 0..sub_dim {
                vec[base + j] = centroid[j];
            }
        }

        vec
    }

    /// 简单 XorShift 随机数生成器
    fn xorshift(&self, state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    /// 计算两个子向量之间的距离
    fn subvector_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        let len = core::cmp::min(a.len(), b.len());
        match self.codebook.distance_type {
            DistanceType::L2 => {
                let mut sum = 0.0;
                for i in 0..len {
                    let diff = a[i] - b[i];
                    sum += diff * diff;
                }
                sum
            }
            DistanceType::InnerProduct => {
                let mut sum = 0.0;
                for i in 0..len {
                    sum += a[i] * b[i];
                }
                -sum
            }
            DistanceType::Cosine => {
                let mut dot = 0.0;
                let mut norm_a = 0.0;
                let mut norm_b = 0.0;
                for i in 0..len {
                    dot += a[i] * b[i];
                    norm_a += a[i] * a[i];
                    norm_b += b[i] * b[i];
                }
                let na = norm_a.sqrt();
                let nb = norm_b.sqrt();
                if na == 0.0 || nb == 0.0 {
                    1.0
                } else {
                    -(dot / (na * nb))
                }
            }
        }
    }

    /// 获取码本大小
    pub fn k(&self) -> usize {
        1 << self.nbits
    }

    /// PQ 编码后每个向量的字节数
    pub fn code_size(&self) -> usize {
        self.m
    }
}

/// IVF_PQ 簇结构
#[repr(C)]
pub struct IVFPQCluster {
    /// 簇中心向量
    pub centroid: Vec<f32>,
    /// 属于该簇的向量数量
    pub vector_count: u32,
    /// PQ 编码数据（平坦数组）
    pub pq_codes: Vec<u8>,
    /// 记录 ID 列表
    pub record_ids: Vec<u16>,
}

impl IVFPQCluster {
    pub fn new(dimension: usize) -> Self {
        let centroid = vec![0.0; dimension];
        IVFPQCluster {
            centroid,
            vector_count: 0,
            pq_codes: Vec::new(),
            record_ids: Vec::new(),
        }
    }

    /// 添加 PQ 编码的向量到簇
    pub fn add_vector(&mut self, pq_code: &[u8], record_id: u16, code_size: usize) {
        let offset = self.pq_codes.len();
        self.pq_codes.resize(offset + code_size, 0);
        for (i, &c) in pq_code.iter().enumerate() {
            self.pq_codes[offset + i] = c;
        }
        self.record_ids.push(record_id);
        self.vector_count += 1;
    }

    /// 获取指定向量的 PQ 编码
    pub fn get_code(&self, index: usize, code_size: usize) -> &[u8] {
        let start = index * code_size;
        &self.pq_codes[start..start + code_size]
    }
}

/// IVF_PQ 索引
pub struct IVFPQIndex {
    /// 向量维度
    pub dimension: usize,
    /// 簇数量
    pub nlist: usize,
    /// 搜索时检查的簇数量
    pub nprobe: usize,
    /// 簇列表
    pub clusters: Vec<IVFPQCluster>,
    /// 乘积量化器
    pub quantizer: ProductQuantizer,
    /// 总向量数量
    pub vector_count: u32,
}

impl IVFPQIndex {
    /// 创建新的 IVF_PQ 索引
    pub fn new(
        dimension: usize,
        nlist: usize,
        nprobe: usize,
        m: usize,
        nbits: usize,
        distance_type: DistanceType,
    ) -> Result<Self> {
        let quantizer = ProductQuantizer::new(dimension, m, nbits, distance_type)?;
        let mut clusters = Vec::with_capacity(nlist);
        for _ in 0..nlist {
            clusters.push(IVFPQCluster::new(dimension));
        }

        Ok(IVFPQIndex {
            dimension,
            nlist,
            nprobe,
            clusters,
            quantizer,
            vector_count: 0,
        })
    }

    /// 训练 IVF_PQ 索引
    pub fn train(
        &mut self,
        data: &[f32],
        n: usize,
        vector_offsets: &[usize],
        record_ids: &[u16],
        max_iter: u32,
    ) -> Result<()> {
        // 1. 训练 PQ 码本
        // 需要足够的训练数据
        let train_n = core::cmp::min(n, 10000);
        self.quantizer.train(data, train_n, max_iter as usize)?;

        // 2. 使用 k-means 初始化簇中心
        // 简化：随机选择 nlist 个向量作为初始簇中心
        let dimension = self.dimension;
        for i in 0..self.nlist {
            if i < n {
                let src = i * dimension;
                for j in 0..dimension {
                    self.clusters[i].centroid[j] = data[src + j];
                }
            }
        }

        // 3. k-means 聚类（使用原始向量）
        for _ in 0..max_iter {
            // 重置簇
            for cluster in &mut self.clusters {
                cluster.vector_count = 0;
                cluster.pq_codes.clear();
                cluster.record_ids.clear();
            }

            // 分配向量到最近簇
            for i in 0..n {
                let vec_start = i * dimension;
                let mut best_c = 0;
                let mut best_dist = f32::MAX;
                for (c, cluster) in self.clusters.iter().enumerate() {
                    let dist = euclidean_distance(&data[vec_start..vec_start + dimension], &cluster.centroid);
                    if dist < best_dist {
                        best_dist = dist;
                        best_c = c;
                    }
                }
                // 编码并添加到簇
                let pq_code = self.quantizer.encode(&data[vec_start..vec_start + dimension])?;
                let code_size = self.quantizer.code_size();
                self.clusters[best_c].add_vector(&pq_code, record_ids[i], code_size);
            }

            // 更新簇中心
            let mut converged = true;
            for cluster in &mut self.clusters {
                if cluster.vector_count > 0 {
                    let mut new_centroid = vec![0.0f32; dimension];
                    let mut centroid_count = 0u32;
                    // 解码 PQ 码并求和
                    for j in 0..cluster.vector_count as usize {
                        let code = cluster.get_code(j, self.quantizer.code_size());
                        let decoded = self.quantizer.decode(code);
                        for k in 0..dimension {
                            new_centroid[k] += decoded[k];
                        }
                        centroid_count += 1;
                    }
                    let inv = 1.0 / centroid_count as f32;
                    for k in 0..dimension {
                        new_centroid[k] *= inv;
                        if (new_centroid[k] - cluster.centroid[k]).abs() > 1e-6 {
                            converged = false;
                        }
                    }
                    cluster.centroid = new_centroid;
                }
            }

            if converged {
                break;
            }
        }

        // 更新总向量数量
        self.vector_count = n as u32;

        Ok(())
    }

    /// 搜索最近邻
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(f32, u16)>> {
        if self.vector_count == 0 {
            return Ok(Vec::new());
        }

        // 1. 计算查询向量与所有簇中心的距离
        let mut cluster_dists: Vec<(f32, usize)> = self
            .clusters
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let d = euclidean_distance(query, &c.centroid);
                (d, i)
            })
            .collect();

        // 2. 排序，选择前 nprobe 个簇
        cluster_dists.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        let nprobe = core::cmp::min(self.nprobe, self.nlist);
        let code_size = self.quantizer.code_size();
        let ksize = self.quantizer.k();

        // 3. 对每个选中的簇，使用 ADC 计算距离
        let mut results = Vec::new();
        for &(_cdist, ci) in cluster_dists.iter().take(nprobe) {
            let cluster = &self.clusters[ci];
            if cluster.vector_count == 0 {
                continue;
            }

            // 预计算该簇的距离表
            let table = self.quantizer.compute_distance_table(query);

            for j in 0..cluster.vector_count as usize {
                let code = cluster.get_code(j, code_size);
                let dist = ProductQuantizer::compute_distance_from_table(&table, code, ksize);
                let record_id = cluster.record_ids[j];
                results.push((dist, record_id));
            }
        }

        // 4. 排序并返回前 k 个
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        results.truncate(k);
        Ok(results)
    }

    /// 插入新向量
    pub fn insert(&mut self, vector: &[f32], record_id: u16) -> Result<()> {
        // 找到最近簇
        let mut best_c = 0;
        let mut best_dist = f32::MAX;
        for (i, cluster) in self.clusters.iter().enumerate() {
            let d = euclidean_distance(vector, &cluster.centroid);
            if d < best_dist {
                best_dist = d;
                best_c = i;
            }
        }

        // PQ 编码
        let pq_code = self.quantizer.encode(vector)?;
        let code_size = self.quantizer.code_size();
        self.clusters[best_c].add_vector(&pq_code, record_id, code_size);
        self.vector_count += 1;

        Ok(())
    }

    /// 删除向量
    pub fn delete(&mut self, _record_id: u16) -> Result<()> {
        // 简单实现：查找并移除
        for cluster in &mut self.clusters {
            if let Some(pos) = cluster.record_ids.iter().position(|&id| id == _record_id) {
                let code_size = self.quantizer.code_size();
                let start = pos * code_size;
                // 移除 PQ 码
                if start < cluster.pq_codes.len() {
                    let new_end = cluster.pq_codes.len() - code_size;
                    for i in start..new_end {
                        cluster.pq_codes[i] = cluster.pq_codes[i + code_size];
                    }
                    cluster.pq_codes.truncate(new_end);
                }
                cluster.record_ids.remove(pos);
                cluster.vector_count -= 1;
                self.vector_count -= 1;
                return Ok(());
            }
        }
        Err(RemDbError::RecordNotFound)
    }
}

/// 计算欧几里得距离
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    let len = core::cmp::min(a.len(), b.len());
    let mut sum = 0.0;
    for i in 0..len {
        let diff = a[i] - b[i];
        sum += diff * diff;
    }
    sum
}

// ===================== 标量量化 (SQ) =====================

/// 标量量化器：将 f32 向量量化为 u8 向量
pub struct ScalarQuantizer {
    /// 每维度的最小值
    pub min_values: Vec<f32>,
    /// 每维度的最大值
    pub max_values: Vec<f32>,
    /// 维度
    pub dimension: usize,
    /// 量化位数（4 或 8）
    pub nbits: u8,
}

impl ScalarQuantizer {
    pub fn new(dimension: usize, nbits: u8) -> Result<Self> {
        if nbits != 4 && nbits != 8 {
            return Err(RemDbError::InvalidData("SQ nbits must be 4 or 8"));
        }
        Ok(ScalarQuantizer {
            min_values: vec![0.0f32; dimension],
            max_values: vec![0.0f32; dimension],
            dimension,
            nbits,
        })
    }

    /// 训练：收集每维度的 min/max
    pub fn train(&mut self, data: &[f32], n: usize) {
        let dim = self.dimension;
        for d in 0..dim {
            self.min_values[d] = f32::MAX;
            self.max_values[d] = f32::MIN;
        }
        for i in 0..n {
            let base = i * dim;
            for d in 0..dim {
                let v = data[base + d];
                if v < self.min_values[d] {
                    self.min_values[d] = v;
                }
                if v > self.max_values[d] {
                    self.max_values[d] = v;
                }
            }
        }
    }

    /// 编码：f32 -> u8
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        let dim = self.dimension;
        let levels = if self.nbits == 8 { 256u16 } else { 16u16 };
        let mut encoded = vec![0u8; dim];
        for d in 0..dim {
            let range = self.max_values[d] - self.min_values[d];
            if range <= 0.0 {
                encoded[d] = 0;
            } else {
                let normalized = (vector[d] - self.min_values[d]) / range;
                let quantized = (normalized * (levels - 1) as f32).round() as u16;
                let quantized = core::cmp::min(quantized, levels - 1);
                encoded[d] = quantized as u8;
            }
        }
        encoded
    }

    /// 解码：u8 -> f32（近似）
    pub fn decode(&self, encoded: &[u8]) -> Vec<f32> {
        let dim = self.dimension;
        let levels = if self.nbits == 8 { 256f32 } else { 16f32 };
        let mut vec = vec![0.0f32; dim];
        for d in 0..dim {
            let ratio = encoded[d] as f32 / (levels - 1.0);
            vec[d] = self.min_values[d] + ratio * (self.max_values[d] - self.min_values[d]);
        }
        vec
    }

    /// 编码后的字节数
    pub fn code_size(&self) -> usize {
        if self.nbits == 8 {
            self.dimension
        } else {
            (self.dimension + 1) / 2 // 4-bit: 两个维度打包一个字节
        }
    }
}

// ===================== 二值量化 (BQ) =====================

/// 二值量化器：将 f32 向量量化为位向量
pub struct BinaryQuantizer {
    /// 每维度的中位数（阈值）
    pub thresholds: Vec<f32>,
    /// 维度
    pub dimension: usize,
}

impl BinaryQuantizer {
    pub fn new(dimension: usize) -> Self {
        BinaryQuantizer {
            thresholds: vec![0.0f32; dimension],
            dimension,
        }
    }

    /// 训练：计算每维度的中位数
    pub fn train(&mut self, data: &[f32], n: usize) {
        let dim = self.dimension;

        // 收集每维度的值
        let mut values: Vec<Vec<f32>> = vec![Vec::with_capacity(n); dim];
        for i in 0..n {
            let base = i * dim;
            for d in 0..dim {
                values[d].push(data[base + d]);
            }
        }

        // 排序并取中位数
        for d in 0..dim {
            values[d].sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            self.thresholds[d] = if n > 0 {
                values[d][n / 2]
            } else {
                0.0
            };
        }
    }

    /// 编码：f32 -> 位向量（每个维度 1 位）
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        let dim = self.dimension;
        let byte_len = (dim + 7) / 8;
        let mut bits = vec![0u8; byte_len];

        for d in 0..dim {
            if vector[d] >= self.thresholds[d] {
                bits[d / 8] |= 1 << (d % 8);
            }
        }

        bits
    }

    /// 计算汉明距离（异或 + popcount）
    pub fn hamming_distance(a: &[u8], b: &[u8]) -> f32 {
        let len = core::cmp::min(a.len(), b.len());
        let mut dist = 0u32;
        for i in 0..len {
            dist += (a[i] ^ b[i]).count_ones();
        }
        dist as f32
    }

    /// 编码后的字节数
    pub fn code_size(&self) -> usize {
        (self.dimension + 7) / 8
    }
}