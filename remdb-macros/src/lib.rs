use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::Token;

/// Proc-macro for table configuration
#[proc_macro]
pub fn table(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as TableInput);
    parsed.generate().into()
}

/// Proc-macro for database configuration
#[proc_macro]
pub fn database(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as DatabaseInput);
    parsed.generate().into()
}

// Table macro input structure
struct TableInput {
    table_name: syn::Ident,
    max_records: syn::Expr,
    primary_key: syn::Ident,
    secondary_index: Option<syn::Ident>,
    fields: Vec<FieldDef>,
}

impl syn::parse::Parse for TableInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let table_name: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        
        let max_records: syn::Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        
        // Parse primary_key as identifier and verify
        let primary_key_keyword: syn::Ident = input.parse()?;
        if primary_key_keyword.to_string() != "primary_key" {
            return Err(syn::Error::new(
                primary_key_keyword.span(),
                "expected 'primary_key'",
            ));
        }
        input.parse::<Token![:]>()?;
        let primary_key: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        
        let mut secondary_index = None;
        if input.peek(syn::Ident) {
            let next_ident: syn::Ident = input.fork().parse()?;
            if next_ident.to_string() == "secondary_index" {
                // Consume the secondary_index keyword
                input.parse::<syn::Ident>()?;
                input.parse::<Token![:]>()?;
                secondary_index = Some(input.parse()?);
                input.parse::<Token![,]>()?;
            }
        }
        
        // Parse fields as identifier and verify
        let fields_keyword: syn::Ident = input.parse()?;
        if fields_keyword.to_string() != "fields" {
            return Err(syn::Error::new(
                fields_keyword.span(),
                "expected 'fields'",
            ));
        }
        input.parse::<Token![:]>()?;
        input.parse::<syn::token::Brace>()?;
        
        let mut fields = Vec::new();
        while !input.peek(syn::token::Brace) {
            fields.push(input.parse()?);
            if !input.peek(syn::token::Brace) {
                input.parse::<Token![,]>()?;
            }
        }
        
        input.parse::<syn::token::Brace>()?;
        
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
        let mut current_offset = 0u32;
        let mut total_record_size = 0u32;
        let mut primary_key_index = None;
        let mut secondary_index_index = None;
        
        // Generate field definitions with correct offsets
        let mut field_defs = Vec::new();
        
        for (index, field) in self.fields.iter().enumerate() {
            let name = &field.name;
            let data_type = field.data_type;
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
            if field.data_type == "String" {
                // Handle string type: str(32)
                // size is an expression like 32
                let string_size = match size {
                    syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int_lit), .. }) => {
                        int_lit.base10_parse::<u32>().unwrap()
                    },
                    _ => panic!("String field size must be a literal integer"),
                };
                
                field_defs.push(quote_spanned! {field.name.span() =>
                    #crate::types::FieldDef {
                        name: stringify!(#name),
                        data_type: #crate::types::DataType::#data_type,
                        size: #string_size,
                        offset: #current_offset,
                    }
                });
                
                current_offset += string_size;
                total_record_size += string_size;
            } else {
                // Handle numeric types
                let type_size = match field.data_type.to_string().as_str() {
                    "Int8" => 1u32,
                    "Int16" => 2u32,
                    "Int32" => 4u32,
                    "Int64" => 8u32,
                    "Float32" => 4u32,
                    "Float64" => 8u32,
                    "Bool" => 1u32,
                    "Timestamp" => 8u32,
                    _ => panic!("Unsupported data type: {}", field.data_type),
                };
                
                field_defs.push(quote_spanned! {field.name.span() =>
                    #crate::types::FieldDef {
                        name: stringify!(#name),
                        data_type: #crate::types::DataType::#data_type,
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
        
        quote! {
            static #table_name: #crate::types::TableDef = #crate::types::TableDef {
                name: stringify!(#table_name),
                fields: &[
                    #(#field_defs),*
                ],
                primary_key: #primary_key,
                secondary_index: #secondary_index,
                record_size: #total_record_size as usize,
                max_records: #max_records,
            };
        }
    }
}

// Field definition structure
struct FieldDef {
    name: syn::Ident,
    data_type: syn::Ident,
    size: syn::Expr,
}

impl syn::parse::Parse for FieldDef {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Ident) {
            let ty: syn::Ident = input.parse()?;
            
            let (data_type, size) = match ty.to_string().as_str() {
                "i8" => (syn::Ident::new("Int8", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("1", Span::call_site())),
                }),
                "i16" => (syn::Ident::new("Int16", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("2", Span::call_site())),
                }),
                "i32" => (syn::Ident::new("Int32", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("4", Span::call_site())),
                }),
                "i64" => (syn::Ident::new("Int64", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("8", Span::call_site())),
                }),
                "f32" => (syn::Ident::new("Float32", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("4", Span::call_site())),
                }),
                "f64" => (syn::Ident::new("Float64", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("8", Span::call_site())),
                }),
                "bool" => (syn::Ident::new("Bool", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("1", Span::call_site())),
                }),
                "u64" => (syn::Ident::new("Timestamp", ty.span()), syn::ExprLit {
                    attrs: Vec::new(),
                    lit: syn::Lit::Int(syn::LitInt::new("8", Span::call_site())),
                }),
                _ => {
                    return Err(syn::Error::new(
                        ty.span(),
                        "Unsupported field type",
                    ));
                }
            };
            
            Ok(Self {
                name,
                data_type,
                size: syn::Expr::Lit(size),
            })
        } else if lookahead.peek(syn::Ident) && input.peek2(syn::token::Paren) {
            // Handle string type: str(64)
            let ty: syn::Ident = input.parse()?;
            if ty.to_string() != "str" {
                return Err(syn::Error::new(
                    ty.span(),
                    "Only 'str' is supported with parentheses syntax",
                ));
            }
            
            let content; 
            syn::parenthesized!(content in input);
            let len: syn::Expr = content.parse()?;
            
            Ok(Self {
                name,
                data_type: syn::Ident::new("String", ty.span()),
                size: len,
            })
        } else {
            return Err(lookahead.error());
        }
    }
}

// Database macro input structure
struct DatabaseInput {
    db_name: syn::Ident,
    tables: Vec<syn::Ident>,
}

impl syn::parse::Parse for DatabaseInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let db_name: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        
        // Parse tables as identifier and verify
        let tables_keyword: syn::Ident = input.parse()?;
        if tables_keyword.to_string() != "tables" {
            return Err(syn::Error::new(
                tables_keyword.span(),
                "expected 'tables'",
            ));
        }
        input.parse::<Token![:]>()?;
        input.parse::<syn::token::Bracket>()?;
        
        let mut tables = Vec::new();
        while !input.peek(syn::token::Bracket) {
            tables.push(input.parse()?);
            if !input.peek(syn::token::Bracket) {
                input.parse::<Token![,]>()?;
            }
        }
        
        input.parse::<syn::token::Bracket>()?;
        
        Ok(Self {
            db_name,
            tables,
        })
    }
}

impl DatabaseInput {
    fn generate(&self) -> proc_macro2::TokenStream {
        let db_name = &self.db_name;
        let tables = &self.tables;
        
        quote! {
            static #db_name: #crate::config::DbConfig = #crate::config::DbConfig {
                tables: &[
                    #(#tables),*
                ],
                total_memory: 0,
            };
        }
    }
}
