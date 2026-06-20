//! Rig 集成测试

use crate::llm::tool::get_tools;

#[test]
fn test_get_rig_tools() {
    // 测试能否获取 Rig 工具列表
    let tools = get_tools();
    
    // 应该有 14 个内置工具（都添加了 embedding_doc）
    assert_eq!(tools.len(), 14, "应该有 14 个 Rig 工具，实际有 {} 个", tools.len());
    
    // 验证工具名称
    let tool_names: Vec<String> = tools.iter().map(|t| t.name()).collect();
    
    // 检查一些关键工具是否存在
    assert!(tool_names.contains(&"add_item".to_string()), "应该有 add_item 工具");
    assert!(tool_names.contains(&"remove_item".to_string()), "应该有 remove_item 工具");
    assert!(tool_names.contains(&"has_item".to_string()), "应该有 has_item 工具");
    assert!(tool_names.contains(&"add_numeric_stat".to_string()), "应该有 add_numeric_stat 工具");
    assert!(tool_names.contains(&"add_to_stat".to_string()), "应该有 add_to_stat 工具");
    
    println!("✓ Rig 工具列表测试通过");
    println!("  工具数量：{}", tools.len());
    println!("  工具名称：{:?}", tool_names);
}

#[test]
fn test_tool_definition() {
    // 测试工具定义是否正确
    let tools = get_tools();
    
    for tool in &tools {
        let name = tool.name();
        // 验证工具名称不为空
        assert!(!name.is_empty(), "工具名称不能为空");
        
        println!("✓ 工具 '{}' 已注册", name);
    }
}
