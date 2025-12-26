use quote::quote;
use proc_macro2::Span;
use super::ddl_parser::TableDef;

pub fn generate_code(table_defs: Vec<TableDef>) -> proc_macro::TokenStream {
    let mut generated = vec![];
    
    for table in table_defs {
        let table_name = table.name;
        let struct_name = syn::Ident::new(&table_name, Span::call_site());
        
        let fields = table.columns.iter().map(|col| {
            let field_name = syn::Ident::new(&col.name, Span::call_site());
            let field_type = syn::Ident::new(&col.typ, Span::call_site());
            
            quote! {
                #field_name: #field_type
            }
        });
        
        generated.push(quote! {
            pub struct #struct_name {
                #(#fields,)*
            }
        });
    }
    
    let output = quote! {
        #(#generated)*
    };
    
    output.into()
}
