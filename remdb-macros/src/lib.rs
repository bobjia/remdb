mod codegen;
mod ddl_parser;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

#[proc_macro]
pub fn define_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::LitStr);
    let schema = input.value();

    match ddl_parser::parse_ddl(&schema) {
        Ok(table_defs) => codegen::generate_code(table_defs),
        Err(e) => {
            panic!("Failed to parse DDL: {}", e);
        }
    }
}

#[proc_macro_derive(MemdbTable, attributes(memdb_schema))]
pub fn derive_memdb_table(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as syn::DeriveInput);

    // 查找memdb_schema属性
    let mut ddl = String::new();

    for attr in &derive_input.attrs {
        if attr.path().is_ident("memdb_schema") {
            // 使用正确的syn 2.0 API解析属性
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("ddl") {
                    let lit = meta.value()?;
                    let lit_str = lit.parse::<syn::LitStr>()?;
                    ddl = lit_str.value();
                }
                Ok(())
            })
            .unwrap();
        }
    }

    if ddl.is_empty() {
        panic!("memdb_schema attribute with ddl parameter is required");
    }

    // 解析DDL并生成代码
    match ddl_parser::parse_ddl(&ddl) {
        Ok(table_defs) => codegen::generate_code(table_defs),
        Err(e) => {
            panic!("Failed to parse DDL: {}", e);
        }
    }
}

use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitInt, Token};

// 字段定义
struct Field {
    name: Ident,
    #[allow(dead_code)]
    colon: Token![:],
    // 自定义类型解析，支持 str(32) 这种语法
    type_name: Ident,
    type_params: Option<LitInt>,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let colon = input.parse()?;

        // 解析类型名称
        let type_name = input.parse()?;

        // 检查是否有括号参数，如 str(32)
        let type_params = if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let params = content.parse()?;
            Some(params)
        } else {
            None
        };

        Ok(Self {
            name,
            colon,
            type_name,
            type_params,
        })
    }
}

// 表定义结构
struct TableArgs {
    name: Ident,
    max_records: LitInt,
    primary_key: Ident,
    secondary_index: Option<Ident>,
    secondary_index_type: Option<Ident>,
    fields: Vec<Field>,
}

impl Parse for TableArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 解析表名
        let name = input.parse()?;

        // 解析逗号
        let _comma1: Token![,] = input.parse()?;

        // 解析最大记录数
        let max_records = input.parse()?;

        // 解析逗号
        let _comma2: Token![,] = input.parse()?;

        // 解析primary_key
        let _primary_key_keyword: Ident = input.parse()?;
        let _colon1: Token![:] = input.parse()?;
        let primary_key = input.parse()?;

        // 解析secondary_index（可选）
        let mut secondary_index = None;
        let mut secondary_index_type = None;

        // 检查primary_key之后是否有逗号
        if input.peek(Token![,]) {
            let _comma3: Token![,] = input.parse()?;
        }

        // 解析secondary_index、secondary_index_type和fields关键字
        loop {
            // 检查下一个标记
            let next = input.lookahead1();
            if next.peek(Ident) {
                let param_name = input.parse::<Ident>()?;
                if param_name == "secondary_index" {
                    let _colon: Token![:] = input.parse()?;
                    secondary_index = Some(input.parse()?);

                    // 解析逗号
                    if input.peek(Token![,]) {
                        let _comma4: Token![,] = input.parse()?;
                    }
                } else if param_name == "secondary_index_type" {
                    let _colon: Token![:] = input.parse()?;
                    secondary_index_type = Some(input.parse()?);

                    // 解析逗号
                    if input.peek(Token![,]) {
                        let _comma5: Token![,] = input.parse()?;
                    }
                } else if param_name == "fields" {
                    let _colon_fields: Token![:] = input.parse()?;
                    break;
                } else {
                    return Err(syn::Error::new(param_name.span(), format!("expected 'secondary_index', 'secondary_index_type' or 'fields' keyword, got '{}'", param_name)));
                }
            } else {
                return Err(next.error());
            }
        }

        // 解析fields块
        let content;
        syn::braced!(content in input);

        // 解析fields块内的内容
        let mut fields = Vec::new();
        while !content.is_empty() {
            // 解析字段
            let field = content.parse::<Field>()?;
            fields.push(field);

            // 如果还有逗号，解析它
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            name,
            max_records,
            primary_key,
            secondary_index,
            secondary_index_type,
            fields,
        })
    }
}

// 数据库定义结构，解析数据库名和表列表
struct DatabaseArgs {
    name: Ident,
    tables: Vec<Ident>,
    low_power: bool,
    low_power_max_records: Option<usize>,
    default_max_records: usize,
    total_memory: usize,
}

impl Parse for DatabaseArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 解析数据库名
        let name = input.parse()?;

        // 解析逗号
        let _comma: Token![,] = input.parse()?;

        // 解析tables关键字
        let _tables: Ident = input.parse()?;

        // 解析冒号
        let _colon: Token![:] = input.parse()?;

        // 解析表列表
        let content;
        syn::bracketed!(content in input);

        let mut tables = Vec::new();
        while !content.is_empty() {
            // 解析表名
            let table = content.parse::<Ident>()?;
            tables.push(table);

            // 如果还有逗号，解析它
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        // 解析可选的low_power参数
        let mut low_power = false;
        let mut low_power_max_records = None;
        let mut default_max_records = 100000; // 默认值
        let mut total_memory = 65536; // 默认64KB

        // 检查是否还有更多参数
        while !input.is_empty() {
            // 解析逗号
            let _comma: Token![,] = input.parse()?;

            // 解析参数名
            let param_name = input.parse::<Ident>()?;

            // 解析冒号
            let _colon: Token![:] = input.parse()?;

            if param_name == "low_power" {
                // 解析布尔值
                let lit_bool = input.parse::<syn::LitBool>()?;
                low_power = lit_bool.value;
            } else if param_name == "low_power_max_records" {
                // 解析数字
                let lit_int = input.parse::<syn::LitInt>()?;
                low_power_max_records = Some(lit_int.base10_parse().unwrap_or(0));
            } else if param_name == "default_max_records" {
                // 解析数字
                let lit_int = input.parse::<syn::LitInt>()?;
                default_max_records = lit_int.base10_parse().unwrap_or(100000);
            } else if param_name == "total_memory" {
                // 解析数字
                let lit_int = input.parse::<syn::LitInt>()?;
                total_memory = lit_int.base10_parse().unwrap_or(65536);
            }
        }

        Ok(Self {
            name,
            tables,
            low_power,
            low_power_max_records,
            default_max_records,
            total_memory,
        })
    }
}

#[proc_macro]
pub fn table(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // 解析输入参数
    let args = parse_macro_input!(input as TableArgs);
    let name = &args.name;
    let max_records = &args.max_records;
    let primary_key = &args.primary_key;
    let secondary_index = &args.secondary_index;
    let secondary_index_type = &args.secondary_index_type;
    let fields = &args.fields;

    // 生成字段定义
    let mut offset = 0;
    let mut field_defs = Vec::new();
    let mut record_size = 0;
    let mut primary_key_index = 0usize;
    let mut secondary_key_index: Option<usize> = None;

    for (i, field) in fields.iter().enumerate() {
        let field_name = &field.name;
        let type_name = &field.type_name;
        let type_params = &field.type_params;

        // 确定数据类型和大小
        let (data_type, size_val, string_length) = if type_name == "i32" {
            (quote!(remdb::types::DataType::Int32), 4, quote!(None))
        } else if type_name == "i8" {
            (quote!(remdb::types::DataType::Int8), 1, quote!(None))
        } else if type_name == "u64" {
            (quote!(remdb::types::DataType::UInt64), 8, quote!(None))
        } else if type_name == "f64" {
            (quote!(remdb::types::DataType::Float64), 8, quote!(None))
        } else if type_name == "bool" {
            (quote!(remdb::types::DataType::Bool), 1, quote!(None))
        } else if type_name == "str" {
            // 处理str(32)这样的类型
            let str_size = if let Some(params) = type_params {
                params.base10_parse().unwrap_or(32)
            } else {
                32
            };
            (
                quote!(remdb::types::DataType::VarChar),
                str_size,
                quote!(Some(#str_size as usize)),
            )
        } else if type_name == "text" {
            // 处理text(1024)或text这样的类型，默认512字节（DEFAULT_TEXT_SIZE）
            let text_size = if let Some(params) = type_params {
                params.base10_parse().unwrap_or(512)
            } else {
                512
            };
            (
                quote!(remdb::types::DataType::Text),
                text_size,
                quote!(None),
            )
        } else if type_name == "vector" {
            // 处理vector(2)这样的向量类型
            let dim = if let Some(params) = type_params {
                params.base10_parse().unwrap_or(128)
            } else {
                128
            };
            (
                quote!(remdb::types::DataType::Vector),
                dim * 4,
                quote!(None),
            ) // 向量每个维度4字节
        } else {
            (quote!(remdb::types::DataType::Int32), 4, quote!(None))
        };

        // 计算对齐要求
        let alignment = if type_name == "u64" || type_name == "f64" || type_name == "i64" {
            8
        } else if type_name == "i32" || type_name == "u32" || type_name == "f32" {
            4
        } else if type_name == "i16" || type_name == "u16" {
            2
        } else {
            1
        };

        // 调整偏移量以满足对齐要求
        offset = ((offset + alignment - 1) / alignment) * alignment;

        // 确定约束字段值
        let is_primary_key = field_name == primary_key;
        let primary_key_val = is_primary_key;
        let not_null_val = is_primary_key; // 主键字段默认为非空
        let unique_val = is_primary_key;

        // 检查是否为自增主键：
        // 1. 整数主键默认自增
        // 2. 可以显式指定AUTOINCREMENT
        let is_integer_type =
            type_name == "i32" || type_name == "i64" || type_name == "u32" || type_name == "u64";
        let auto_increment_val = is_primary_key && is_integer_type;

        // 生成向量元数据（仅向量类型字段需要）
        let vector_metadata_code = if type_name == "vector" {
            let dim = if let Some(params) = type_params {
                params.base10_parse::<u16>().unwrap_or(128)
            } else {
                128u16
            };
            quote! {
                Some(remdb::types::VectorMetadata {
                    dimension: #dim,
                    distance_type: remdb::types::DistanceType::L2,
                    index_type: remdb::types::VectorIndexType::HNSW,
                    compression_enabled: false,
                    compression_scheme: 0,
                    compression_level: 3,
                    // HNSW默认参数
                    hnsw_m: 16,
                    hnsw_ef_construction: 200,
                    hnsw_ef_search: 128,
                    // IVF默认参数
                    ivf_nlist: 1024,
                    ivf_nprobe: 16,
                })
            }
        } else {
            quote! { None }
        };

        // 生成字段定义
        let field_def = quote! {
            remdb::types::FieldDef {
                name: stringify!(#field_name).to_string(),
                data_type: #data_type,
                size: #size_val as usize, // 确保是usize类型
                string_length: #string_length,
                offset: #offset as usize,  // 确保是usize类型
                primary_key: #primary_key_val,
                not_null: #not_null_val,
                unique: #unique_val,
                auto_increment: #auto_increment_val,
                default_value: None,
                vector_metadata: #vector_metadata_code,
                json_metadata: None,
            }
        };

        field_defs.push(field_def);

        // 确定主键和二级索引的字段索引
        if field_name == primary_key {
            primary_key_index = i;
        }

        if let Some(secondary_field) = secondary_index {
            if field_name == secondary_field {
                secondary_key_index = Some(i);
            }
        }

        // 更新偏移量和记录大小
        offset += size_val;
        record_size = offset;
    }

    // 确保整个记录满足最大对齐要求（8字节对齐）
    let max_alignment = 8;
    record_size = ((record_size + max_alignment - 1) / max_alignment) * max_alignment;

    // 将max_records转换为usize
    let max_records_usize = max_records.base10_parse::<usize>().unwrap_or(100);

    // 确定索引类型
    let index_type = match secondary_index_type.as_ref() {
        Some(ty) if ty == "btree" => quote!(remdb::types::IndexType::BTree),
        Some(ty) if ty == "hash" => quote!(remdb::types::IndexType::Hash),
        Some(ty) if ty == "ttree" => quote!(remdb::types::IndexType::TTree),
        Some(ty) if ty == "sortedarray" => quote!(remdb::types::IndexType::SortedArray),
        _ => quote!(remdb::types::IndexType::BTree),
    };

    // 生成secondary_index代码
    let secondary_index_code = match secondary_key_index {
        Some(index) => quote! { Some(vec![#index as usize]) },
        None => quote! { None },
    };

    // 生成代码：返回一个静态TableDef变量，使用LazyLock延迟初始化
    let output = quote! {
        #[allow(non_upper_case_globals)]
        pub static #name: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| {
            remdb::types::TableDef {
                id: 0,
                name: stringify!(#name).to_string(),
                fields: vec![#(#field_defs,)*],
                primary_key: vec![#primary_key_index as usize],
                secondary_index: #secondary_index_code,
                secondary_index_type: #index_type,
                record_size: #record_size as usize,
                max_records: #max_records_usize,
                version: 1,
                created_at: 0,
                updated_at: 0,
            }
        });
    };

    output.into()
}

#[proc_macro]
pub fn database(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // 解析输入参数
    let args = parse_macro_input!(input as DatabaseArgs);
    let name = &args.name;
    let tables = &args.tables;
    let low_power = args.low_power;
    let default_max_records = args.default_max_records;
    let total_memory = args.total_memory;

    // 处理low_power_max_records，转换为Option<usize>
    let low_power_max_records = match args.low_power_max_records {
        Some(val) => quote! { Some(#val) },
        None => quote! { None },
    };

    // 生成代码：返回一个静态DbConfig变量，使用LazyLock延迟初始化
    let output = quote! {
        #[allow(non_upper_case_globals)]
        pub static #name: std::sync::LazyLock<remdb::config::DbConfig> = std::sync::LazyLock::new(|| {
            remdb::config::DbConfig {
                tables: vec![#( std::sync::LazyLock::force(&#tables).clone(), )*],
                total_memory: #total_memory,
                low_power_mode_supported: #low_power,
                low_power_max_records: #low_power_max_records,
                default_max_records: #default_max_records,
                memory_allocator: unsafe {
                    // 使用默认的内存分配器实现，这里返回一个空指针的静态引用
                    static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;
                    &mut DEFAULT_ALLOCATOR
                },
                // 日志相关配置
                wal_config: remdb::config::WALConfig {
                    log_path: "wal",
                    log_mode: remdb::config::LogMode::Sync,
                    checkpoint_interval_ms: 60000, // 默认60秒
                    log_file_size_limit: 16 * 1024 * 1024, // 默认16MB
                    log_prealloc_size: 1 * 1024 * 1024, // 默认1MB预分配
                    log_segment_size: 16 * 1024 * 1024, // 默认16MB分段
                    retained_checkpoints: 3, // 保留3个检查点
                    max_consecutive_invalid: 100,
                    skip_threshold: 1000,
                    skip_block_size: 1024 * 1024,
                    max_skip_attempts: 3,
                    compression_type: remdb::config::WALCompressionType::None,
                    compression_level: 3
                },
                // 时序数据默认配置
                time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
                // PubSub配置（可选）
                #[cfg(feature = "pubsub")]
                pubsub_config: None,
                // HA相关配置（可选）
                #[cfg(feature = "ha")]
                ha_config: Some(remdb::ha::HAConfig {
                    node_id: 1, // 默认节点ID为1
                    ha_role: remdb::ha::HARole::Auto,
                    replication_mode: remdb::ha::ReplicationMode::Async,
                    heartbeat_interval_ms: 1000, // 默认1秒
                    failure_detection_ms: 3000, // 默认3秒
                    sync_timeout_ms: 2000, // 默认2秒
                    master_address: None,
                    master_port: None,
                    replication_port: 5556,
                }),
                // Model Worker配置
                model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
            }
        });
    };

    output.into()
}
