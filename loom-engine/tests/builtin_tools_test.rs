//! Integration tests for built-in tools.

use loom_engine::llm::tool::builtin_tools::*;
use loom_engine::llm::tool::Tool;
use serde_json::json;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test state (only once)
fn setup() {
    INIT.call_once(|| {
        init_save_data();
    });
    // Reset state for each test
    reset_state();
}

/// Reset state for each test
fn reset_state() {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard.stats.clear();
    guard.inventory.clear();
    guard.history.clear();
}

// ============ Item Management Tests ============

#[test]
fn test_add_item() {
    setup();
    
    let tool = AddItem;
    let args = json!({"name": "sword", "amount": 5});
    let output = tool.execute(args).unwrap();

    assert_eq!(output.tool_name, "add_item");
    assert_eq!(output.value, json!(5));
    assert!(output.description.contains("sword"));
    assert!(output.description.contains("5"));
}

#[test]
fn test_add_new_item() {
    setup();

    // Add a new item
    let tool = AddItem;
    let args = json!({"name": "potion", "amount": 3});
    let output = tool.execute(args).unwrap();

    // Should return the amount added
    assert_eq!(output.value, json!(3));
}

#[test]
fn test_add_existing_item() {
    setup();

    // Add item first
    AddItem.execute(json!({"name": "elixir", "amount": 2})).unwrap();

    // Add same item again
    let tool = AddItem;
    let args = json!({"name": "elixir", "amount": 3});
    let output = tool.execute(args).unwrap();

    // Should return the new total (2 + 3 = 5)
    assert_eq!(output.value, json!(5));
}

#[test]
fn test_add_multiple_items() {
    setup();
    let tool = AddItem;
    
    // Add first batch
    let args = json!({"name": "potion", "amount": 3});
    let output = tool.execute(args).unwrap();
    assert_eq!(output.value, json!(3));

    // Add same item again
    let args = json!({"name": "potion", "amount": 2});
    let output = tool.execute(args).unwrap();
    assert_eq!(output.value, json!(5)); // 3 + 2 = 5
}

#[test]
fn test_remove_item() {
    setup();

    // First add items
    AddItem.execute(json!({"name": "gold", "amount": 10})).unwrap();

    // Then remove some
    let tool = RemoveItem;
    let args = json!({"name": "gold", "amount": 4});
    let output = tool.execute(args).unwrap();

    // remove_item returns the amount removed
    assert_eq!(output.value, json!(4));
}

#[test]
fn test_remove_item_not_enough() {
    setup();

    // Add items
    AddItem.execute(json!({"name": "silver", "amount": 5})).unwrap();

    // Try to remove more than we have
    let tool = RemoveItem;
    let args = json!({"name": "silver", "amount": 10});
    let result = tool.execute(args);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Not enough"));
}

#[test]
fn test_remove_item_not_found() {
    setup();

    // Try to remove non-existent item
    let tool = RemoveItem;
    let args = json!({"name": "diamond", "amount": 1});
    let result = tool.execute(args);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_has_item() {
    setup();

    // Add item first
    AddItem.execute(json!({"name": "ring", "amount": 1})).unwrap();
    
    // Check existence
    let tool = HasItem;
    let args = json!({"name": "ring"});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(true));
    
    // Check non-existent item
    let args = json!({"name": "amulet"});
    let output = tool.execute(args).unwrap();
    assert_eq!(output.value, json!(false));
}

#[test]
fn test_get_item_count() {
    setup();

    // Add items
    AddItem.execute(json!({"name": "arrow", "amount": 20})).unwrap();
    
    // Check count
    let tool = GetItemCount;
    let args = json!({"name": "arrow"});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(20));
    
    // Check non-existent item
    let args = json!({"name": "bow"});
    let output = tool.execute(args).unwrap();
    assert_eq!(output.value, json!(0));
}

#[test]
fn test_clear_inventory() {
    setup();

    // Add items
    AddItem.execute(json!({"name": "test_item", "amount": 100})).unwrap();
    
    // Clear
    let tool = ClearInventory;
    let output = tool.execute(json!({})).unwrap();
    
    assert!(output.description.contains("cleared"));
    
    // Verify inventory is empty
    let args = json!({"name": "test_item"});
    let output = GetItemCount.execute(args).unwrap();
    assert_eq!(output.value, json!(0));
}

// ============ Stat Management Tests ============

#[test]
fn test_add_numeric_stat() {
    setup();

    let tool = AddNumericStat;
    let args = json!({
        "name": "Health",
        "value": 100,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    });
    let output = tool.execute(args).unwrap();

    assert_eq!(output.tool_name, "add_numeric_stat");
    assert!(output.description.contains("Health"));
    assert!(output.description.contains("100"));
}

#[test]
fn test_add_tag_stat() {
    setup();

    let tool = AddTagStat;
    let args = json!({"name": "HasMetKing"});
    let output = tool.execute(args).unwrap();

    assert_eq!(output.tool_name, "add_tag_stat");
    assert!(output.description.contains("HasMetKing"));
}

#[test]
fn test_has_stat() {
    setup();

    // Add stat first
    AddNumericStat.execute(json!({
        "name": "Mana",
        "value": 50,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    })).unwrap();
    
    // Check existence
    let tool = HasStat;
    let args = json!({"name": "Mana"});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(true));
    
    // Check non-existent stat
    let args = json!({"name": "Stamina"});
    let output = tool.execute(args).unwrap();
    assert_eq!(output.value, json!(false));
}

#[test]
fn test_get_stat_value() {
    setup();

    // Add stat
    AddNumericStat.execute(json!({
        "name": "Attack",
        "value": 25,
        "min": 0,
        "max": 999,
        "include_min": true,
        "include_max": true
    })).unwrap();
    
    // Get value
    let tool = GetStatValue;
    let args = json!({"name": "Attack"});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(25));
}

#[test]
fn test_add_to_stat() {
    setup();

    // Add stat first
    AddNumericStat.execute(json!({
        "name": "Experience",
        "value": 100,
        "min": 0,
        "max": 1000,
        "include_min": true,
        "include_max": true
    })).unwrap();
    
    // Add to it
    let tool = AddToStat;
    let args = json!({"name": "Experience", "amount": 50});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(150));
}

#[test]
fn test_sub_from_stat() {
    setup();

    // Add stat first
    AddNumericStat.execute(json!({
        "name": "Health",
        "value": 100,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    })).unwrap();
    
    // Subtract from it
    let tool = SubFromStat;
    let args = json!({"name": "Health", "amount": 30});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(70));
}

#[test]
fn test_list_stat_names() {
    setup();

    // Add some stats
    AddNumericStat.execute(json!({
        "name": "Strength",
        "value": 10,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    })).unwrap();
    
    AddTagStat.execute(json!({"name": "Blessed"})).unwrap();
    
    // List names
    let tool = ListStatNames;
    let output = tool.execute(json!({})).unwrap();
    
    let names: Vec<String> = serde_json::from_value(output.value).unwrap();
    assert!(names.contains(&"Strength".to_string()));
    assert!(names.contains(&"Blessed".to_string()));
}

#[test]
fn test_remove_stats() {
    setup();

    // Add stat first
    AddNumericStat.execute(json!({
        "name": "TempStat",
        "value": 50,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    })).unwrap();
    
    // Remove it
    let tool = RemoveStats;
    let args = json!({"names": ["TempStat"]});
    let output = tool.execute(args).unwrap();
    
    assert_eq!(output.value, json!(1));
    
    // Verify it's gone
    let output = HasStat.execute(json!({"name": "TempStat"})).unwrap();
    assert_eq!(output.value, json!(false));
}

#[test]
fn test_set_stat_value() {
    setup();

    // Add stat first
    AddNumericStat.execute(json!({
        "name": "Speed",
        "value": 50,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    })).unwrap();

    // Set new value
    let tool = SetStatValue;
    let args = json!({
        "name": "Speed",
        "value": 80,
        "min": 0,
        "max": 120,
        "include_min": true,
        "include_max": true
    });
    let _ = tool.execute(args).unwrap();
    
    // Verify new value
    let output = GetStatValue.execute(json!({"name": "Speed"})).unwrap();
    assert_eq!(output.value, json!(80));
}

// ============ Error Handling Tests ============

#[test]
fn test_get_nonexistent_stat() {
    setup();

    let tool = GetStatValue;
    let args = json!({"name": "NonExistent"});
    let result = tool.execute(args);
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_add_to_nonexistent_stat() {
    setup();

    let tool = AddToStat;
    let args = json!({"name": "NonExistent", "amount": 10});
    let result = tool.execute(args);
    
    assert!(result.is_err());
}

#[test]
fn test_invalid_stat_range() {
    setup();

    let tool = AddNumericStat;
    // value outside range
    let args = json!({
        "name": "Invalid",
        "value": 150,
        "min": 0,
        "max": 100,
        "include_min": true,
        "include_max": true
    });
    let result = tool.execute(args);
    
    assert!(result.is_err());
}
