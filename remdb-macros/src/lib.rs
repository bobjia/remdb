mod codegen;
mod ddl_parser;

#[proc_macro]
pub fn define_schema(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::LitStr);
    let schema = input.value();
    
    match ddl_parser::parse_ddl(&schema) {
        Ok(table_defs) => {
            let output = codegen::generate_code(table_defs);
            output.into()
        },
        Err(e) => {
            panic!("Failed to parse DDL: {}", e);
        }
    }
}

use syn::{parse_macro_input, LitInt, Ident, Token};
use syn::parse::{Parse, ParseStream};
use quote::quote;

// 字段定义
struct Field {
    name: Ident,
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
        let primary_key_keyword: Ident = input.parse()?;
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
            }
        }
        
        Ok(Self {
            name,
            tables,
            low_power,
            low_power_max_records,
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
        let (data_type, size_val) = if type_name == "i32" {
            (quote!(remdb::types::DataType::Int32), 4)
        } else if type_name == "i8" {
            (quote!(remdb::types::DataType::Int8), 1)
        } else if type_name == "u64" {
            (quote!(remdb::types::DataType::UInt64), 8)
        } else if type_name == "f64" {
            (quote!(remdb::types::DataType::Float64), 8)
        } else if type_name == "bool" {
            (quote!(remdb::types::DataType::Bool), 1)
        } else if type_name == "str" {
            // 处理str(32)这样的类型
            let str_size = if let Some(params) = type_params {
                params.base10_parse().unwrap_or(32)
            } else {
                32
            };
            (quote!(remdb::types::DataType::String), str_size)
        } else {
            (quote!(remdb::types::DataType::Int32), 4)
        };
        
        // 生成字段定义
        let field_def = quote! {
            remdb::types::FieldDef {
                name: stringify!(#field_name),
                data_type: #data_type,
                size: #size_val as usize, // 确保是usize类型
                offset: #offset as usize,  // 确保是usize类型
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
        record_size += size_val;
    }
    
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
        Some(index) => quote! { Some(#index) },
        None => quote! { None },
    };
    
    // 生成代码：返回一个TableDef静态变量
    let output = quote! {
        #[allow(non_upper_case_globals)]
        pub static #name: remdb::types::TableDef = remdb::types::TableDef {
            id: 0,
            name: stringify!(#name),
            fields: &[#(#field_defs,)*],
            primary_key: #primary_key_index as usize,
            secondary_index: #secondary_index_code,
            secondary_index_type: #index_type,
            record_size: #record_size as usize,
            max_records: #max_records_usize,
        };
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
    
    // 处理low_power_max_records，转换为Option<usize>
    let low_power_max_records = match args.low_power_max_records {
        Some(val) => quote! { Some(#val) },
        None => quote! { None }
    };
    
    // 生成代码：返回一个DbConfig静态变量
    let output = quote! {
        #[allow(non_upper_case_globals)]
        pub static #name: remdb::config::DbConfig = remdb::config::DbConfig {
            tables: &[#(#tables),*],
            total_memory: 65536,
            low_power_mode_supported: #low_power,
            low_power_max_records: #low_power_max_records,
        };
    };
    
    output.into()
}