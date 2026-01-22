use remdb::sql::parse_sql_query;

fn main() {
    // 测试完整的查询，包括FROM子句
    let query = "SELECT id, vector <-> [0.0, 5.0] as distance FROM VECTOR_TABLE_2D ORDER BY distance LIMIT 10";
    println!("Testing query: {}", query);
    
    match parse_sql_query(query) {
        Ok(parsed) => {
            println!("✅ Query parsed successfully!");
            println!("   Query type: {:?}", parsed.query_type);
            println!("   Table name: {}", parsed.table_name);
            println!("   Columns: {:?}", parsed.columns);
            println!("   Order by: {:?}", parsed.order_by);
            println!("   Limit: {:?}", parsed.limit);
        },
        Err(err) => {
            println!("❌ Query parsing failed: {:?}", err);
        }
    }
}