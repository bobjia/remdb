import sys

path = "/workspace/src/lib.rs"
with open(path) as f:
    lines = f.readlines()

# 1-based inclusive line range to replace
start, end = 1098, 1263

assert lines[start-1].strip().startswith("if let Some(default_value)"), repr(lines[start-1])
assert lines[end-1].strip() == "}", repr(lines[end-1])

new_block = '''                    if let Some(default_value) = field.default_value {
                        // 根据数据类型写入默认值，添加完善的边界检查
                        match field.data_type {
                            crate::types::DataType::Bool => {
                                if offset + 1 <= log_data.len() {
                                    log_data[offset] = default_value.as_bool() as u8;
                                    offset += 1;
                                }
                            },
                            crate::types::DataType::Int8 => {
                                if offset + 1 <= log_data.len() {
                                    log_data[offset] = default_value.as_i8() as u8;
                                    offset += 1;
                                }
                            },
                            crate::types::DataType::UInt8 => {
                                if offset + 1 <= log_data.len() {
                                    log_data[offset] = default_value.as_u8();
                                    offset += 1;
                                }
                            },
                            crate::types::DataType::Int16 => {
                                if offset + 2 <= log_data.len() {
                                    log_data[offset..offset+2].copy_from_slice(&default_value.as_i16().to_le_bytes());
                                    offset += 2;
                                }
                            },
                            crate::types::DataType::UInt16 => {
                                if offset + 2 <= log_data.len() {
                                    log_data[offset..offset+2].copy_from_slice(&default_value.as_u16().to_le_bytes());
                                    offset += 2;
                                }
                            },
                            crate::types::DataType::Int32 => {
                                if offset + 4 <= log_data.len() {
                                    log_data[offset..offset+4].copy_from_slice(&default_value.as_i32().to_le_bytes());
                                    offset += 4;
                                }
                            },
                            crate::types::DataType::UInt32 => {
                                if offset + 4 <= log_data.len() {
                                    log_data[offset..offset+4].copy_from_slice(&default_value.as_u32().to_le_bytes());
                                    offset += 4;
                                }
                            },
                            crate::types::DataType::Float32 => {
                                if offset + 4 <= log_data.len() {
                                    log_data[offset..offset+4].copy_from_slice(&default_value.as_float32().to_le_bytes());
                                    offset += 4;
                                }
                            },
                            crate::types::DataType::Int64 => {
                                if offset + 8 <= log_data.len() {
                                    log_data[offset..offset+8].copy_from_slice(&default_value.as_i64().to_le_bytes());
                                    offset += 8;
                                }
                            },
                            crate::types::DataType::UInt64 => {
                                if offset + 8 <= log_data.len() {
                                    log_data[offset..offset+8].copy_from_slice(&default_value.as_u64().to_le_bytes());
                                    offset += 8;
                                }
                            },
                            crate::types::DataType::Float64 => {
                                if offset + 8 <= log_data.len() {
                                    log_data[offset..offset+8].copy_from_slice(&default_value.as_float64().to_le_bytes());
                                    offset += 8;
                                }
                            },
                            crate::types::DataType::Timestamp | crate::types::DataType::TimestampTZ => {
                                if offset + 8 <= log_data.len() {
                                    log_data[offset..offset+8].copy_from_slice(&default_value.as_time().value.to_le_bytes());
                                    offset += 8;
                                }
                            },
                            crate::types::DataType::String => {
                                if offset + 65 <= log_data.len() {
                                    let s = default_value.as_string();
                                    let string_len = core::cmp::min(s.iter().position(|&c| c == 0).unwrap_or(64), 64);
                                    log_data[offset] = string_len as u8;
                                    offset += 1;
                                    log_data[offset..offset+string_len].copy_from_slice(&s[..string_len]);
                                    offset += 64;
                                }
                            },
                            crate::types::DataType::Interval => {
                                if offset + 10 <= log_data.len() {
                                    let interval = default_value.as_interval();
                                    log_data[offset..offset+8].copy_from_slice(&interval.value.to_le_bytes());
                                    offset += 8;
                                    log_data[offset] = interval.precision;
                                    offset += 1;
                                    log_data[offset] = interval.flags;
                                    offset += 1;
                                }
                            },
                        }
                    }
'''
new_lines = lines[:start-1] + [new_block] + lines[end:]
with open(path, "w") as f:
    f.writelines(new_lines)

print("Replaced lines", start, "to", end)