use remdb::json::JsonDocument;

#[test]
fn test_json_document_creation() {
    // 测试空JSON
    let empty_json = "";
    let empty_doc = JsonDocument::from_json(empty_json).unwrap();
    assert!(empty_doc.is_null());
    assert_eq!(empty_doc.size(), 0);
}

#[test]
fn test_json_to_string() {
    // 测试JSON文档转换为字符串
    let json_str = r#"{"name": "test", "age": 25}"#;
    let doc = JsonDocument::from_json(json_str).unwrap();

    let result = doc.to_json();
    assert!(result.is_ok());
    let json_result = result.unwrap();
    assert!(!json_result.is_empty());
}

#[test]
fn test_json_storage() {
    // 测试内联存储（小JSON）
    let small_json = r#"{"name": "test"}"#;
    let small_doc = JsonDocument::from_json(small_json).unwrap();
    assert!(!small_doc.is_null());
}

#[test]
fn test_json_null() {
    // 测试null JSON
    let null_json = "null";
    let null_doc = JsonDocument::from_json(null_json).unwrap();
    assert!(!null_doc.is_null());
    assert!(null_doc.size() > 0);
}

#[test]
fn test_json_parse_error() {
    // 测试无效JSON - from_json存储原始字符串，解析时才会验证
    let invalid_json = r#"{"name": "test", "age": 25"#; // 缺少结束大括号
    let result = JsonDocument::from_json(invalid_json);
    // from_json不验证JSON，只存储原始字符串
    assert!(result.is_ok());
    let doc = result.unwrap();
    // 解析时才会验证JSON并返回错误
    let parse_result = doc.parse_json();
    assert!(parse_result.is_err());
}

#[test]
fn test_json_clone() {
    // 测试JSON文档克隆
    let json_str = r#"{"name": "test", "age": 25}"#;
    let doc1 = JsonDocument::from_json(json_str).unwrap();
    let doc2 = doc1.clone();

    assert_eq!(doc1.size(), doc2.size());
    assert_eq!(doc1, doc2);
}

#[test]
fn test_json_equality() {
    // 测试JSON文档相等性
    let json_str = r#"{"name": "test", "age": 25}"#;
    let doc1 = JsonDocument::from_json(json_str).unwrap();
    let doc2 = JsonDocument::from_json(json_str).unwrap();

    assert_eq!(doc1, doc2);
}

#[test]
fn test_json_is_null() {
    // 测试is_null方法
    let null_json = "";
    let null_doc = JsonDocument::from_json(null_json).unwrap();
    assert!(null_doc.is_null());
}
