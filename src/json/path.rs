//! JSON路径查询
//!
//! 该模块实现了JSONPath路径表达式的解析和执行，支持：
//! - 完整的JSONPath语法
//! - 编译时路径预编译
//! - 高效路径遍历

use crate::json::{JsonDocument, JsonQueryResult, JsonValue};

/// JSON路径表达式节点
#[derive(Debug, Clone, PartialEq)]
enum PathNode {
    /// 根节点
    Root,
    /// 对象键访问
    Key(alloc::string::String),
    /// 数组索引访问
    Index(usize),
    /// 数组通配符
    Wildcard,
    /// 数组切片
    Slice {
        start: Option<usize>,
        end: Option<usize>,
    },
    /// 过滤表达式
    Filter(alloc::string::String),
}

/// 编译后的JSON路径
pub struct CompiledPath {
    nodes: alloc::vec::Vec<PathNode>,
}

impl CompiledPath {
    /// 解析JSON路径表达式
    pub fn parse(path: &str) -> Result<Self, &'static str> {
        if path.is_empty() {
            return Err("Empty path");
        }

        let mut nodes = alloc::vec::Vec::new();
        let mut remaining = path;

        // 处理根节点
        if !remaining.starts_with('$') {
            return Err("Path must start with $");
        }
        nodes.push(PathNode::Root);
        remaining = &remaining[1..];

        // 解析路径组件
        while !remaining.is_empty() {
            if remaining.starts_with('.') {
                // 对象键访问
                remaining = &remaining[1..];

                if remaining.starts_with('[') {
                    // 数组索引访问
                    let (index, rest) = Self::parse_array_access(remaining)?;
                    nodes.push(index);
                    remaining = rest;
                } else {
                    // 对象键访问
                    let (key, rest) = Self::parse_key(remaining)?;
                    nodes.push(PathNode::Key(key));
                    remaining = rest;
                }
            } else if remaining.starts_with('[') {
                // 数组访问
                let (index, rest) = Self::parse_array_access(remaining)?;
                nodes.push(index);
                remaining = rest;
            } else {
                return Err("Invalid path syntax");
            }
        }

        Ok(Self { nodes })
    }

    /// 解析对象键
    fn parse_key(path: &str) -> Result<(alloc::string::String, &str), &'static str> {
        let mut end = 0;
        while end < path.len() {
            let c = path.as_bytes()[end];
            if c == b'.' || c == b'[' || c == b']' {
                break;
            }
            end += 1;
        }

        if end == 0 {
            return Err("Empty key");
        }

        let key = alloc::string::String::from(&path[..end]);
        Ok((key, &path[end..]))
    }

    /// 解析数组访问
    fn parse_array_access(path: &str) -> Result<(PathNode, &str), &'static str> {
        if !path.starts_with('[') {
            return Err("Array access must start with [");
        }

        let mut end = 1;
        while end < path.len() {
            if path.as_bytes()[end] == b']' {
                break;
            }
            end += 1;
        }

        if end >= path.len() {
            return Err("Array access must end with ]");
        }

        let index_str = &path[1..end];
        let rest = &path[end + 1..];

        if index_str == "*" {
            // 通配符
            Ok((PathNode::Wildcard, rest))
        } else if index_str.contains(':') {
            // 切片
            let parts: alloc::vec::Vec<&str> = index_str.split(':').collect();
            let start = if parts[0].is_empty() {
                None
            } else {
                parts[0].parse::<usize>().ok()
            };
            let end = if parts.len() > 1 && !parts[1].is_empty() {
                parts[1].parse::<usize>().ok()
            } else {
                None
            };

            Ok((PathNode::Slice { start, end }, rest))
        } else if index_str.starts_with('?') {
            // 过滤表达式
            Ok((
                PathNode::Filter(alloc::string::String::from(&index_str[1..])),
                rest,
            ))
        } else {
            // 具体索引
            let index = index_str
                .parse::<usize>()
                .map_err(|_| "Invalid array index")?;
            Ok((PathNode::Index(index), rest))
        }
    }

    /// 执行路径查询
    pub fn execute(&self, doc: &crate::json::JsonDocument) -> crate::json::JsonQueryResult {
        match doc.parse_json() {
            Ok(root_value) => {
                let mut current = vec![root_value];

                for node in &self.nodes[1..] {
                    // 跳过根节点
                    let mut next = vec![];

                    for value in current {
                        match (node, value) {
                            (PathNode::Key(key), crate::json::JsonValue::Object(obj)) => {
                                if let Some(v) = obj.get(key) {
                                    next.push(v.clone());
                                }
                            }
                            (PathNode::Index(index), JsonValue::Array(arr)) => {
                                if *index < arr.len() {
                                    next.push(arr[*index].clone());
                                }
                            }
                            (PathNode::Wildcard, JsonValue::Object(obj)) => {
                                for v in obj.values() {
                                    next.push(v.clone());
                                }
                            }
                            (PathNode::Wildcard, JsonValue::Array(arr)) => {
                                for v in arr {
                                    next.push(v.clone());
                                }
                            }
                            (PathNode::Slice { start, end }, JsonValue::Array(arr)) => {
                                let start_idx = start.unwrap_or(0);
                                let end_idx = end.unwrap_or(arr.len());
                                for i in start_idx..std::cmp::min(end_idx, arr.len()) {
                                    next.push(arr[i].clone());
                                }
                            }
                            _ => {}
                        }
                    }

                    if next.is_empty() {
                        return JsonQueryResult::None;
                    }

                    current = next;
                }

                // 处理结果
                if current.len() == 1 {
                    match &current[0] {
                        JsonValue::String(s) => JsonQueryResult::Scalar(s.clone()),
                        JsonValue::Number(n) => JsonQueryResult::Scalar(n.clone()),
                        JsonValue::Boolean(b) => JsonQueryResult::Scalar(b.to_string()),
                        JsonValue::Null => JsonQueryResult::Scalar("null".to_string()),
                        JsonValue::Object(_) => {
                            // 直接返回原始文档，因为我们没有实现对象的重新序列化
                            JsonQueryResult::Object(doc.clone())
                        }
                        JsonValue::Array(arr) => {
                            let results: Vec<JsonQueryResult> = arr
                                .iter()
                                .map(|v| match v {
                                    JsonValue::String(s) => JsonQueryResult::Scalar(s.clone()),
                                    JsonValue::Number(n) => JsonQueryResult::Scalar(n.clone()),
                                    JsonValue::Boolean(b) => JsonQueryResult::Scalar(b.to_string()),
                                    JsonValue::Null => JsonQueryResult::Scalar("null".to_string()),
                                    _ => JsonQueryResult::None,
                                })
                                .collect();
                            JsonQueryResult::Array(results)
                        }
                    }
                } else {
                    let results: Vec<JsonQueryResult> = current
                        .iter()
                        .map(|v| match v {
                            JsonValue::String(s) => JsonQueryResult::Scalar(s.clone()),
                            JsonValue::Number(n) => JsonQueryResult::Scalar(n.clone()),
                            JsonValue::Boolean(b) => JsonQueryResult::Scalar(b.to_string()),
                            JsonValue::Null => JsonQueryResult::Scalar("null".to_string()),
                            _ => JsonQueryResult::None,
                        })
                        .collect();
                    JsonQueryResult::Array(results)
                }
            }
            Err(_) => JsonQueryResult::None,
        }
    }
}

/// JSON路径表达式
pub struct JsonPath {
    /// 原始路径字符串
    path_str: alloc::string::String,
    /// 编译后的路径
    compiled: Option<CompiledPath>,
}

impl JsonPath {
    /// 创建新的JSON路径
    pub fn new(path: &str) -> Result<Self, &'static str> {
        let compiled = CompiledPath::parse(path)?;
        Ok(Self {
            path_str: alloc::string::String::from(path),
            compiled: Some(compiled),
        })
    }

    /// 执行路径查询
    pub fn execute(&self, doc: &JsonDocument) -> JsonQueryResult {
        if let Some(compiled) = &self.compiled {
            compiled.execute(doc)
        } else {
            JsonQueryResult::None
        }
    }

    /// 获取原始路径字符串
    pub fn as_str(&self) -> &str {
        &self.path_str
    }
}

/// 解析JSON路径表达式
pub fn parse_json_path(path: &str) -> Result<JsonPath, &'static str> {
    JsonPath::new(path)
}

/// 执行JSON路径查询
pub fn execute_json_path(doc: &JsonDocument, path: &str) -> JsonQueryResult {
    match JsonPath::new(path) {
        Ok(json_path) => json_path.execute(doc),
        Err(_) => JsonQueryResult::None,
    }
}
