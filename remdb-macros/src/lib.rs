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

#[proc_macro]
pub fn table(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // 简化实现，实际项目中应该根据需求实现完整功能
    input
}

#[proc_macro]
pub fn database(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // 简化实现，实际项目中应该根据需求实现完整功能
    input
}
