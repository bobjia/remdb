use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};

use crate::ddl_parser::{TableDef, SqlType};

/// 生成Rust代码的核心逻辑

/// 为表生成Rust结构体和相关代码
pub fn generate_table_code(table: &TableDef, max_records: usize) -> TokenStream {
    let table_name_ident = Ident::new(&table.name, Span::call_site());
    let struct_name_ident = Ident::new(&table.name, Span::call_site());
    
    // 生成结构体名称：首字母大写
    let struct_name = table.name.split('_')
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>();
    let struct_name_ident = Ident::new(&struct_name, Span::call_site());
    
    // 生成静态变量名称：全大写
    let static_name = table.name.to_uppercase();
    let static_name_ident = Ident::new(&static_name, Span::call_site());
    
    // 生成模块名称：全小写
    let module_name = table.name.to_lowercase();
    let module_name_ident = Ident::new(&module_name, Span::call_site());
    
    // 生成结构体字段
    let struct_fields = table.columns.iter().map(|col| {
        let field_name = Ident::new(&col.name, Span::call_site());
        let rust_type_str = col.sql_type.to_rust_type(col.not_null);
        
        // 使用syn::parse_str将类型字符串解析为TokenStream
        let rust_type = syn::parse_str::<syn::Type>(&rust_type_str).unwrap();
        
        quote_spanned! {Span::call_site() =>
            pub #field_name: #rust_type,
        }
    }).collect::<Vec<_>>();
    
    // 生成字段定义
    let mut current_offset = 0usize;
    let mut primary_key_index = None;
    let mut field_defs = Vec::new();
    
    for (index, col) in table.columns.iter().enumerate() {
        let name = Ident::new(&col.name, Span::call_site());
        let data_type = col.sql_type.to_data_type();
        let data_type_ident = Ident::new(&data_type, Span::call_site());
        
        let field_size = match col.sql_type {
            SqlType::Integer => 8,
            SqlType::Text => 32, // 固定长度字符串，暂时硬编码为32字节
            SqlType::Real => 8,
            SqlType::Boolean => 1,
            SqlType::Timestamp => 8,
        };
        
        if col.is_primary_key {
            primary_key_index = Some(index);
        }
        
        field_defs.push(quote! {
            remdb::types::FieldDef {
                name: stringify!(#name),
                data_type: remdb::types::DataType::#data_type_ident,
                size: #field_size,
                offset: #current_offset,
            }
        });
        
        current_offset += field_size;
    }
    
    let primary_key = primary_key_index.unwrap_or(0);
    let total_record_size = current_offset;
    
    // 生成API函数
    let api_functions = generate_api_functions(table, &struct_name_ident, &module_name_ident);
    
    quote! {
        /// 生成的表结构体
        #[repr(C)]
        #[derive(Debug, Clone, PartialEq)]
        pub struct #struct_name_ident {
            #(#struct_fields)*
        }
        
        /// 表元数据定义
        static #static_name_ident: remdb::types::TableDef = remdb::types::TableDef {
            id: 0,
            name: stringify!(#static_name_ident),
            fields: &[
                #(#field_defs),*
            ],
            primary_key: #primary_key,
            secondary_index: None,
            record_size: #total_record_size,
            max_records: #max_records,
        };
        
        #api_functions
    }
}

/// 生成类型安全的API函数
fn generate_api_functions(table: &TableDef, struct_name: &Ident, module_name: &Ident) -> TokenStream {
    // 查找主键列
    let primary_key_col = table.columns.iter()
        .find(|col| col.is_primary_key)
        .unwrap_or(&table.columns[0]);
    
    let _pk_name = Ident::new(&primary_key_col.name, Span::call_site());
    let pk_type_str = primary_key_col.sql_type.to_rust_type(true);
    let pk_type = syn::parse_str::<syn::Type>(&pk_type_str).unwrap();
    
    quote! {
        /// 类型安全的表操作API
        pub mod #module_name {
            use super::*;
            
            /// 插入记录
            pub fn insert(db: &mut remdb::RemDb, record: #struct_name) -> remdb::types::Result<()> {
                // 这里将生成实际的插入逻辑
                // 目前是占位符
                Ok(())
            }
            
            /// 通过主键查询记录
            pub fn get_by_id(db: &remdb::RemDb, id: #pk_type) -> remdb::types::Result<Option<#struct_name>> {
                // 这里将生成实际的查询逻辑
                // 目前是占位符
                Ok(None)
            }
            
            /// 更新记录
            pub fn update(db: &mut remdb::RemDb, record: #struct_name) -> remdb::types::Result<()> {
                // 这里将生成实际的更新逻辑
                // 目前是占位符
                Ok(())
            }
            
            /// 删除记录
            pub fn delete(db: &mut remdb::RemDb, id: #pk_type) -> remdb::types::Result<()> {
                // 这里将生成实际的删除逻辑
                // 目前是占位符
                Ok(())
            }
        }
    }
}

/// 生成数据库配置代码
pub fn generate_database_code(table_names: &[String], table_defs: &[TokenStream]) -> TokenStream {
    // 使用全大写的静态变量名
    let table_idents = table_names.iter()
        .map(|name| Ident::new(&name.to_uppercase(), Span::call_site()))
        .collect::<Vec<_>>();
    
    quote! {
        #(#table_defs)*
        
        /// 数据库配置
        static DATABASE: remdb::config::DbConfig = remdb::config::DbConfig {
            tables: &[
                #(#table_idents),*
            ],
            total_memory: 0,
            low_power_mode_supported: false,
            low_power_max_records: None,
        };
    }
}

/// 生成TableId类型
pub fn generate_table_id(table_name: &str) -> TokenStream {
    let table_name_ident = Ident::new(table_name, Span::call_site());
    let table_id_ident = Ident::new(&format!("{}Id", table_name), Span::call_site());
    
    quote! {
        /// 唯一的表ID类型
        pub struct #table_id_ident;
        
        impl remdb::types::TableId for #table_id_ident {
            const TABLE_ID: u8 = 0;
        }
    }
}
