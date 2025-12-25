use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::parse_macro_input;
use syn::{Ident, Lit, Meta, Token};
use syn::meta::ParseNestedMeta;

// 导入新模块
mod ddl_parser;
mod codegen;

// 导出现有宏
/// Proc-macro for table configuration
#[proc_macro]
pub fn table(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as TableInput);
    parsed.generate().into()
}

/// Proc-macro for database configuration
#[proc_macro]
pub fn database(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as DatabaseInput);
    parsed.generate().into()
}

/// 模式配置
enum SchemaConfig {
    Inline(String), // 内联DDL
    File(String),   // 外部DDL文件路径
}

/// 支持从DDL生成类型安全代码的派生宏
#[proc_macro_derive(MemdbTable, attributes(memdb_schema))]
pub fn memdb_table_derive(input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as syn::ItemStruct);
    
    // 查找memdb_schema属性
    let mut schema_attr = None;
    for attr in &item.attrs {
        let path = attr.path();
        let path_str = path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");
        if path_str == "memdb_schema" {
            schema_attr = Some(attr.clone());
            break;
        }
    }
    
    let schema_attr = schema_attr.ok_or_else(|| {
        syn::Error::new(item.span(), "memdb_table_derive requires a #[memdb_schema] attribute")
    }).unwrap();
    
    // 解析属性参数
    let mut schema_config = None;
    schema_attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("ddl") {
            let value = meta.value()?.parse::<syn::LitStr>()?;
            schema_config = Some(SchemaConfig::Inline(value.value()));
        } else if meta.path.is_ident("file") {
            let value = meta.value()?.parse::<syn::LitStr>()?;
            schema_config = Some(SchemaConfig::File(value.value()));
        }
        Ok(())
    }).unwrap();
    
    let schema_config = schema_config.ok_or_else(|| {
        syn::Error::new(item.span(), "Expected either ddl or file argument in memdb_schema attribute")
    }).unwrap();
    
    // 根据配置生成代码
    let generated_code = match schema_config {
        SchemaConfig::Inline(ddl) => {
            // 解析内联DDL
            let mut parser = ddl_parser::DdlParser::new(ddl);
            let tables = parser.parse().unwrap();
            generate_code_from_tables(&tables)
        },
        SchemaConfig::File(path) => {
            // 读取文件并解析DDL
            let content = std::fs::read_to_string(path).unwrap();
            let mut parser = ddl_parser::DdlParser::new(content);
            let tables = parser.parse().unwrap();
            generate_code_from_tables(&tables)
        },
    };
    
    generated_code.into()
}

/// 生成表代码
fn generate_code_from_tables(tables: &[ddl_parser::TableDef]) -> proc_macro2::TokenStream {
    let mut table_names = Vec::new();
    let mut table_defs = Vec::new();
    
    for table in tables {
        table_names.push(table.name.clone());
        let table_code = codegen::generate_table_code(table, 1000); // 默认最大记录数1000
        table_defs.push(table_code);
    }
    
    // 生成数据库配置
    let database_code = codegen::generate_database_code(&table_names, &table_defs);
    
    quote! {
        #database_code
    }
}

// 保留现有代码
// Table macro input structure
struct TableInput {
    table_name: Ident,
    max_records: syn::Expr,
    primary_key: Ident,
    secondary_index: Option<Ident>,
    fields: Vec<FieldDef>,
}

impl syn::parse::Parse for TableInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse table name: `TEST_TABLE`
        let table_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        
        // Parse max records: `100`
        let max_records: syn::Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        
        // Parse primary key: `primary_key: id`
        let _: Ident = input.parse()?; // "primary_key"
        input.parse::<Token![:]>()?;
        let primary_key: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        
        // Parse secondary index if present: `secondary_index: name`
        let mut secondary_index = None;
        if input.peek(Ident) {
            let fork = input.fork();
            let ident: Ident = fork.parse()?;
            
            if ident.to_string() == "secondary_index" {
                let _: Ident = input.parse()?; // "secondary_index"
                input.parse::<Token![:]>()?;
                secondary_index = Some(input.parse()?);
                input.parse::<Token![,]>()?;
            }
        }
        
        // Parse fields: `fields: { ... }`
        let _: Ident = input.parse()?; // "fields"
        input.parse::<Token![:]>()?;
        
        // Parse opening brace
        let content;
        let _braces = syn::braced!(content in input);
        
        // Parse field definitions
        let mut fields = Vec::new();
        
        // Simple field parsing loop
        while !content.is_empty() {
            // Parse a single field
            let field: FieldDef = content.parse()?;
            fields.push(field);
            
            // Skip trailing comma if present
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
            
            // Exit if we've reached the end
            if content.is_empty() {
                break;
            }
        }
        
        Ok(Self {
            table_name,
            max_records,
            primary_key,
            secondary_index,
            fields,
        })
    }
}

impl TableInput {
    fn generate(&self) -> proc_macro2::TokenStream {
        let table_name = &self.table_name;
        let max_records = &self.max_records;
        
        // Calculate field offsets and record size
        let mut current_offset = 0usize;
        let mut total_record_size = 0usize;
        let mut primary_key_index = None;
        let mut secondary_index_index = None;
        
        // Generate field definitions with correct offsets
        let mut field_defs = Vec::new();
        
        for (index, field) in self.fields.iter().enumerate() {
            let name = &field.name;
            let data_type = field.data_type.clone();
            let size = &field.size;
            
            // Check if this is the primary key
            if field.name == self.primary_key {
                primary_key_index = Some(index);
            }
            
            // Check if this is the secondary index
            if let Some(secondary_key) = &self.secondary_index {
                if field.name == *secondary_key {
                    secondary_index_index = Some(index);
                }
            }
            
            // For each field, calculate its size and offset
            if field.data_type.to_string() == "String" {
                // Handle string type: str(32)
                // size is an expression like 32
                let string_size = match size {
                    syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int_lit), .. }) => {
                        int_lit.base10_parse::<usize>().unwrap()
                    },
                    _ => panic!("String field size must be a literal integer"),
                };
                
                field_defs.push(quote_spanned! {field.name.span() =>
                    remdb::types::FieldDef {
                        name: stringify!(#name),
                        data_type: remdb::types::DataType::#data_type,
                        size: #string_size,
                        offset: #current_offset,
                    }
                });
                
                current_offset += string_size;
                total_record_size += string_size;
            } else {
                // Handle numeric types
                let type_size = match field.data_type.to_string().as_str() {
                    "Int8" => 1usize,
                    "Int16" => 2usize,
                    "Int32" => 4usize,
                    "Int64" => 8usize,
                    "Float32" => 4usize,
                    "Float64" => 8usize,
                    "Bool" => 1usize,
                    "Timestamp" => 8usize,
                    _ => panic!("Unsupported data type: {}", field.data_type),
                };
                
                field_defs.push(quote_spanned! {field.name.span() =>
                    remdb::types::FieldDef {
                        name: stringify!(#name),
                        data_type: remdb::types::DataType::#data_type,
                        size: #type_size,
                        offset: #current_offset,
                    }
                });
                
                current_offset += type_size;
                total_record_size += type_size;
            }
        }
        
        let primary_key = primary_key_index.unwrap_or(0);
        let secondary_index = match secondary_index_index {
            Some(index) => quote! {Some(#index)},
            None => quote! {None},
        };
        
        // Debug: Check if field_defs is empty
        if field_defs.is_empty() {
            panic!("field_defs is empty! No fields were generated!");
        }
        
        // Debug: Print how many fields were generated
        println!("Generated {} fields for table {}", field_defs.len(), table_name);
        
        quote! {
            static #table_name: remdb::types::TableDef = remdb::types::TableDef {
                id: 0,
                name: stringify!(#table_name),
                fields: &[
                    #(#field_defs),*
                ],
                primary_key: #primary_key,
                secondary_index: #secondary_index,
                secondary_index_type: remdb::types::IndexType::SortedArray,
                record_size: #total_record_size as usize,
                max_records: #max_records,
            };
            
            // Compile-time assertion that the fields array is not empty
            const _: () = assert!(#table_name.fields.len() > 0, "Fields array is empty!");
        }
    }
}

// Field definition structure
struct FieldDef {
    name: Ident,
    data_type: Ident,
    size: syn::Expr,
}

impl syn::parse::Parse for FieldDef {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse field name
        let name: Ident = input.parse()?;
        let name_span = name.span();
        
        // Parse colon
        input.parse::<Token![:]>()?;
        
        // We need to handle two cases: string types and numeric/bool types
        
        // Case 1: String type (str(20))
        if input.peek(Ident) {
            let fork = input.fork();
            let ident: Ident = fork.parse()?;
            
            if ident.to_string() == "str" && fork.peek(syn::token::Paren) {
                // It's a string type: str(20)
                let _: Ident = input.parse()?; // Consume "str"
                let content;
                syn::parenthesized!(content in input);
                let len: syn::Expr = content.parse()?;
                
                return Ok(Self {
                    name,
                    data_type: Ident::new("String", name_span),
                    size: len,
                });
            }
        }
        
        // Case 2: Numeric or bool type (i64, i32, bool, etc.)
        // Instead of trying to parse as Ident, we'll read tokens as strings
        let mut type_str = String::new();
        let mut is_done = false;
        
        // We'll use a loop to read tokens until we hit a comma or closing brace
        while !is_done && !input.is_empty() {
            // Check if we're at the end of the field
            if input.peek(Token![,]) || input.peek(syn::token::Brace) {
                is_done = true;
                break;
            }
            
            // Read the next token as a string
            let token = input.step(|cursor| {
                let (token, rest) = cursor.token_tree()
                    .ok_or_else(|| syn::Error::new(cursor.span(), "Expected token"))?;
                Ok((token.to_string(), rest))
            })?;
            
            type_str.push_str(&token);
        }
        
        type_str = type_str.trim().to_string();
        
        if type_str.is_empty() {
            return Err(syn::Error::new(input.span(), "Expected field type"));
        }
        
        // Map the type string to DataType and size
        let (data_type, size) = match type_str.as_str() {
            "i8" => ("Int8", "1"),
            "i16" => ("Int16", "2"),
            "i32" => ("Int32", "4"),
            "i64" => ("Int64", "8"),
            "f32" => ("Float32", "4"),
            "f64" => ("Float64", "8"),
            "bool" => ("Bool", "1"),
            "u64" => ("Timestamp", "8"),
            _ => {
                return Err(syn::Error::new(
                    input.span(),
                    format!("Unsupported field type: {}", type_str),
                ));
            }
        };
        
        Ok(Self {
            name,
            data_type: Ident::new(data_type, name_span),
            size: syn::parse_str(size)?,
        })
    }
}



// Database macro input structure
struct DatabaseInput {
    db_name: Ident,
    tables: Vec<Ident>,
    low_power: bool,
    low_power_max_records: Option<usize>,
}

impl syn::parse::Parse for DatabaseInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let db_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        
        // Parse tables as identifier and verify
        let tables_keyword: Ident = input.parse()?;
        if tables_keyword.to_string() != "tables" {
            return Err(syn::Error::new(
                tables_keyword.span(),
                "expected 'tables'",
            ));
        }
        input.parse::<Token![:]>()?;
        
        // Parse opening bracket
        let content;
        syn::bracketed!(content in input);
        
        let mut tables = Vec::new();
        let mut content = content;
        while !content.is_empty() {
            tables.push(content.parse()?);
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        
        let mut low_power = false;
        let mut low_power_max_records = None;
        
        // Parse optional parameters
        while !input.is_empty() {
            // Check if we have a comma before the next parameter
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
            
            // Check if we've reached the end of the macro input
            if input.is_empty() {
                break;
            }
            
            let param_name: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            
            match param_name.to_string().as_str() {
                "low_power" => {
                    let lit: syn::LitBool = input.parse()?;
                    low_power = lit.value;
                },
                "low_power_max_records" => {
                    let lit: syn::LitInt = input.parse()?;
                    let value = lit.base10_parse::<usize>().unwrap();
                    low_power_max_records = Some(value);
                },
                _ => {
                    return Err(syn::Error::new(
                        param_name.span(),
                        format!("Unexpected parameter: {}", param_name),
                    ));
                }
            }
        }
        
        Ok(Self {
            db_name,
            tables,
            low_power,
            low_power_max_records,
        })
    }
}

impl DatabaseInput {
    fn generate(&self) -> proc_macro2::TokenStream {
        let db_name = &self.db_name;
        let tables = &self.tables;
        let low_power = self.low_power;
        let low_power_max_records = match &self.low_power_max_records {
            Some(val) => quote! { Some(#val) },
            None => quote! { None },
        };
        
        quote! {
            static #db_name: remdb::config::DbConfig = remdb::config::DbConfig {
                tables: &[
                    #(#tables),*
                ],
                total_memory: 0,
                low_power_mode_supported: #low_power,
                low_power_max_records: #low_power_max_records,
            };
        }
    }
}

