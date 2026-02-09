use super::ddl_parser::{ColumnDef, TableDef};
use proc_macro2::Span;
use quote::quote;

pub fn generate_code(table_defs: Vec<TableDef>) -> proc_macro::TokenStream {
    let mut struct_defs = vec![];
    let mut table_defs_code = vec![];
    let mut time_series_table_defs_code = vec![];
    let mut table_names = vec![];
    let mut time_series_table_names = vec![];

    for table in table_defs {
        let table_name = table.name;
        // 将表名转换为驼峰式命名（CamelCase）
        let struct_name_str = table_name
            .split('_')
            .map(|part| {
                if part.is_empty() {
                    String::new()
                } else {
                    part.chars()
                        .next()
                        .map(|c| c.to_uppercase().collect::<String>() + &part[1..])
                        .unwrap_or(part.to_string())
                }
            })
            .collect::<String>();
        let struct_name = syn::Ident::new(&struct_name_str, Span::call_site());
        let table_ident =
            syn::Ident::new(&table_name.to_uppercase().to_string(), Span::call_site());

        // 生成结构体定义
        let struct_fields = table.columns.iter().map(|col| {
            let field_name = syn::Ident::new(&col.name, Span::call_site());
            let rust_type = convert_to_rust_type(&col.typ, col.nullable, col.primary_key);

            quote! {
                #field_name: #rust_type
            }
        });

        struct_defs.push(quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #struct_name {
                #(#struct_fields,)*
            }
        });

        if table.is_time_series {
            // 生成时序表的基础表定义
            let (field_defs, record_size, primary_key_index, secondary_index, secondary_index_type) =
                generate_field_defs(&table.columns, &table.indices);

            let max_records = 1000usize; // 默认值，可以通过DDL扩展支持

            // 查找时间字段和值字段的索引
            let time_field_index = table
                .columns
                .iter()
                .position(|col| col.name == "time" || col.name == "timestamp")
                .unwrap_or(0);
            let value_field_index = table
                .columns
                .iter()
                .position(|col| col.name == "value" || col.name == "val")
                .unwrap_or(1);

            // 提取标签字段索引（所有非时间和非值字段）
            let tag_field_indices: Vec<usize> = table
                .columns
                .iter()
                .enumerate()
                .filter(|(i, _col)| *i != time_field_index && *i != value_field_index)
                .map(|(i, _)| i)
                .collect();

            // 转换标签字段索引为TokenStream
            let tag_fields_code = tag_field_indices.iter().map(|index| {
                quote! { #index }
            });

            time_series_table_defs_code.push(quote! {
                #[allow(non_upper_case_globals)]
                pub static #table_ident: std::sync::LazyLock<remdb::time_series::TimeSeriesTableDef> = std::sync::LazyLock::new(|| {
                    remdb::time_series::TimeSeriesTableDef {
                        base: remdb::types::TableDef {
                            id: 0u8,
                            name: #table_name.to_string(),
                            fields: vec![#(#field_defs,)*],
                            primary_key: #primary_key_index,
                            secondary_index: #secondary_index,
                            secondary_index_type: #secondary_index_type,
                            record_size: #record_size,
                            max_records: #max_records,
                            version: 1,
                            created_at: 0,
                            updated_at: 0,
                        },
                        time_field: #time_field_index,
                            value_field: #value_field_index,
                            tag_fields: Box::new([#(#tag_fields_code,)*]),
                            config: remdb::time_series::TimeSeriesConfig::DEFAULT,
                    }
                });
            });

            time_series_table_names.push(table_ident);
        } else {
            // 生成普通表定义
            let (field_defs, record_size, primary_key_index, secondary_index, secondary_index_type) =
                generate_field_defs(&table.columns, &table.indices);

            let max_records = 1000usize; // 默认值，可以通过DDL扩展支持

            table_defs_code.push(quote! {
                #[allow(non_upper_case_globals)]
                pub static #table_ident: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| {
                    remdb::types::TableDef {
                        id: 0u8,
                        name: #table_name.to_string(),
                        fields: vec![#(#field_defs,)*],
                        primary_key: #primary_key_index,
                        secondary_index: #secondary_index,
                        secondary_index_type: #secondary_index_type,
                        record_size: #record_size,
                        max_records: #max_records,
                        version: 1,
                        created_at: 0,
                        updated_at: 0,
                    }
                });
            });

            table_names.push(table_ident);
        }
    }

    // 生成数据库配置
    let database_ident = syn::Ident::new("DATABASE", Span::call_site());

    let database_code = quote! {
        #[allow(non_upper_case_globals)]
        pub static #database_ident: std::sync::LazyLock<remdb::config::DbConfig> = std::sync::LazyLock::new(|| {
            remdb::config::DbConfig {
                tables: vec![#( #table_names.clone(), )*],
                total_memory: 1024 * 1024, // 1MB
                low_power_mode_supported: false,
                low_power_max_records: None,
                default_max_records: 100, // 减少默认记录数以避免内存不足
                memory_allocator: unsafe {
                    static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;
                    &mut DEFAULT_ALLOCATOR
                },
                // WAL配置
                wal_config: remdb::config::WALConfig {
                    log_path: "wal",
                    log_mode: remdb::config::LogMode::Sync,
                    checkpoint_interval_ms: 60000,
                    log_file_size_limit: 16 * 1024 * 1024,
                    log_prealloc_size: 1 * 1024 * 1024,
                    log_segment_size: 16 * 1024 * 1024,
                    retained_checkpoints: 3,
                    max_consecutive_invalid: 100,
                    skip_threshold: 1000,
                    skip_block_size: 1024 * 1024,
                    max_skip_attempts: 3,
                },
                // PubSub配置（可选）
                #[cfg(feature = "pubsub")]
                pubsub_config: None,
                // HA相关配置（可选）
                #[cfg(feature = "ha")]
                ha_config: Some(remdb::ha::HAConfig {
                    ha_role: remdb::ha::HARole::Auto,
                    replication_mode: remdb::ha::ReplicationMode::Async,
                    node_id: 1,
                    heartbeat_interval_ms: 1000,
                    failure_detection_ms: 3000,
                    sync_timeout_ms: 2000,
                    master_address: None,
                    master_port: None,
                    replication_port: 5556,
                }),
                time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
            }
        });
    };

    let output = quote! {
        #(#struct_defs)*
        #(#table_defs_code)*
        #(#time_series_table_defs_code)*
        #database_code
    };

    output.into()
}

fn generate_field_defs(
    columns: &[ColumnDef],
    indices: &[super::ddl_parser::IndexDef],
) -> (
    Vec<proc_macro2::TokenStream>,
    usize,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let mut field_defs = vec![];
    let mut offset = 0;
    let mut primary_key_indices = vec![];
    let mut secondary_index_indices = vec![];
    let mut secondary_index_type = quote!(remdb::types::IndexType::BTree);

    for (i, col) in columns.iter().enumerate() {
        let name = &col.name;
        let data_type = convert_to_data_type(&col.typ);
        let size = get_type_size(&col.typ);
        let primary_key = col.primary_key;
        let not_null = !col.nullable; // nullable为false表示not null
        let unique = col.unique;

        // 检查是否为自增主键：
        // 1. 显式指定AUTOINCREMENT
        // 2. INTEGER PRIMARY KEY（隐式自增）
        let is_integer_primary_key = col.typ.to_lowercase() == "integer" && col.primary_key;
        let auto_increment = col.auto_increment || is_integer_primary_key;

        // 计算 string_length
        let string_length = if col.typ.to_lowercase().contains("varchar") || col.typ.to_lowercase().contains("text") || col.typ.to_lowercase().contains("string") {
            quote!(Some(#size))
        } else {
            quote!(None)
        };

        field_defs.push(quote! {
            remdb::types::FieldDef {
                name: #name.to_string(),
                data_type: #data_type,
                size: #size,
                string_length: #string_length,
                offset: #offset,
                primary_key: #primary_key,
                not_null: #not_null,
                unique: #unique,
                auto_increment: #auto_increment,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            }
        });

        if col.primary_key {
            primary_key_indices.push(i);
        }

        offset += size;
    }

    // 处理索引
    for index in indices {
        if let Some(col_index) = columns.iter().position(|col| col.name == index.field) {
            secondary_index_indices.push(col_index);
            secondary_index_type = match index.index_type.to_lowercase().as_str() {
                "hash" => quote!(remdb::types::IndexType::Hash),
                "sortedarray" => quote!(remdb::types::IndexType::SortedArray),
                "ttree" => quote!(remdb::types::IndexType::TTree),
                _ => quote!(remdb::types::IndexType::BTree),
            };
        }
    }

    // 生成primary_key代码
    let primary_key_code = if !primary_key_indices.is_empty() {
        quote!(vec![#(#primary_key_indices),*])
    } else {
        quote!(vec![0]) // 默认第一个字段为主键
    };

    // 生成secondary_index代码
    let secondary_index_code = if !secondary_index_indices.is_empty() {
        quote!(Some(vec![#(#secondary_index_indices),*]))
    } else {
        quote!(None)
    };

    (
        field_defs,
        offset,
        primary_key_code,
        secondary_index_code,
        secondary_index_type,
    )
}

fn convert_to_data_type(sql_type: &str) -> proc_macro2::TokenStream {
    match sql_type.to_lowercase().as_str() {
        "integer" | "int" => quote!(remdb::types::DataType::Int32),
        "bigint" => quote!(remdb::types::DataType::Int64),
        "smallint" => quote!(remdb::types::DataType::Int16),
        "tinyint" => quote!(remdb::types::DataType::Int8),
        "unsigned integer" | "uint" => quote!(remdb::types::DataType::UInt32),
        "unsigned bigint" => quote!(remdb::types::DataType::UInt64),
        "unsigned smallint" => quote!(remdb::types::DataType::UInt16),
        "unsigned tinyint" => quote!(remdb::types::DataType::UInt8),
        "real" | "float" => quote!(remdb::types::DataType::Float32),
        "double" | "double precision" => quote!(remdb::types::DataType::Float64),
        "boolean" | "bool" => quote!(remdb::types::DataType::Bool),
        "text" | "varchar" | "string" => quote!(remdb::types::DataType::VarChar),
        "timestamp" => quote!(remdb::types::DataType::Timestamp),
        _ => quote!(remdb::types::DataType::Int32),
    }
}

fn get_type_size(sql_type: &str) -> usize {
    match sql_type.to_lowercase().as_str() {
        "integer" | "int" | "unsigned integer" | "uint" => 4,
        "bigint" | "unsigned bigint" => 8,
        "smallint" | "unsigned smallint" => 2,
        "tinyint" | "unsigned tinyint" | "boolean" | "bool" => 1,
        "real" | "float" => 4,
        "double" | "double precision" => 8,
        "text" | "varchar" | "string" => 64, // 默认字符串大小
        "timestamp" => 8,
        _ => 4, // 默认大小
    }
}

fn convert_to_rust_type(
    sql_type: &str,
    nullable: bool,
    is_primary_key: bool,
) -> proc_macro2::TokenStream {
    let base_type = match sql_type.to_lowercase().as_str() {
        "integer" | "int" => quote!(i32),
        "bigint" => quote!(i64),
        "smallint" => quote!(i16),
        "tinyint" => quote!(i8),
        "unsigned integer" | "uint" => quote!(u32),
        "unsigned bigint" => quote!(u64),
        "unsigned smallint" => quote!(u16),
        "unsigned tinyint" => quote!(u8),
        "real" | "float" => quote!(f32),
        "double" | "double precision" => quote!(f64),
        "boolean" | "bool" => quote!(bool),
        "text" | "varchar" | "string" => quote!(String),
        "timestamp" => quote!(u64),
        _ => quote!(i32),
    };

    // 主键字段不能为None
    if nullable && !is_primary_key {
        quote!(Option<#base_type>)
    } else {
        base_type
    }
}
