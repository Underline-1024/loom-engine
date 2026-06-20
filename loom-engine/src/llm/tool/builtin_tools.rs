//! Built-in tools for game state management.

use crate::llm::tool;
use crate::actor::Stat;
use crate::config::GameMode;
use crate::lim::Lim;
use crate::save::SaveData;
use anyhow::{anyhow, bail, Result};
use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Helper type for stat definitions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum StatHelper<T> {
    Numeric {
        value: T,
        min: T,
        max: T,
        include_min: bool,
        include_max: bool,
    },
    Tag,
}

// ============ Global State ============

static SAVE_DATA: OnceCell<Arc<Mutex<SaveData>>> = OnceCell::new();

/// Initialize the global save data.
pub fn init_save_data() {
    SAVE_DATA
        .set(Arc::new(Mutex::new(SaveData::new(GameMode::Author))))
        .unwrap();
}

/// Get a clone of the save data Arc.
pub fn save_data() -> Arc<Mutex<SaveData>> {
    SAVE_DATA.get().expect("SaveData not initialized!").clone()
}

// ============ Item Management Tools ============

/// Add items to inventory.
#[tool(
    result_doc = "Added {value} {name}(s). Total: {value}",
    param = (name, "Item name"),
    param = (amount, "Quantity to add"),
    embedding_doc = "Add items to player inventory"
)]
pub fn add_item(name: String, amount: u64) -> u64 {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard.add_item(&name, amount)
}

/// Remove items from inventory.
#[tool(
    result_doc = "Removed {value} {name}(s)",
    param = (name, "Item name to remove"),
    param = (amount, "Quantity to remove"),
    embedding_doc = "Remove items from player inventory"
)]
pub fn remove_item(name: String, amount: u64) -> Result<u64> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard.remove_item(&name, amount)
}

/// Clear all items from inventory.
#[tool(
    result_doc = "Inventory cleared.",
    embedding_doc = "Clear all items from player inventory"
)]
pub fn clear_inventory() {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard.clear_inventory();
}

/// Check if player has a specific item.
#[tool(
    result_doc = "Player has {name}: {value}",
    param = (name, "Item name to check"),
    embedding_doc = "Check if player has a specific item in inventory"
)]
pub fn has_item(name: String) -> bool {
    let data = save_data();
    let guard = data.lock().unwrap();
    guard.has_item(&name)
}

/// Get the count of a specific item.
#[tool(
    result_doc = "Player has {value} {name}(s).",
    param = (name, "Item name"),
    embedding_doc = "Get the count of a specific item in player inventory"
)]
pub fn get_item_count(name: String) -> u64 {
    let data = save_data();
    let guard = data.lock().unwrap();
    guard.item_count(&name)
}

// ============ Stat Management Tools ============

/// Add a numeric stat with limits.
#[tool(
    result_doc = "Added numeric stat {name} = {value} (range: {min}-{max})",
    param = (name, "Stat name, e.g., 'Health', 'Mana', 'Attack'"),
    param = (value, "Initial value"),
    param = (min, "Minimum allowed value"),
    param = (max, "Maximum allowed value"),
    param = (include_min, "Whether minimum is inclusive"),
    param = (include_max, "Whether maximum is inclusive"),
    embedding_doc = "Add a numeric stat with limits (e.g., Health, Mana, Attack)"
)]
pub fn add_numeric_stat(
    name: String,
    value: i64,
    min: i64,
    max: i64,
    include_min: bool,
    include_max: bool,
) -> Result<()> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    let lim = Lim::new(value, min, max, include_min, include_max)?;
    guard.add_stats(vec![(name, Stat::Numeric(lim))]);
    Ok(())
}

/// Add a tag/flag stat.
#[tool(
    result_doc = "Added tag stat: {name}",
    param = (name, "Tag/flag name, e.g., 'HasMetKing', 'Cursed'"),
    embedding_doc = "Add a tag/flag stat (e.g., HasMetKing, Cursed)"
)]
pub fn add_tag_stat(name: String) -> Result<()> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard.add_stats(vec![(name, Stat::Tag)]);
    Ok(())
}

/// Remove stats by name.
#[tool(
    result_doc = "Removed {value} stats.",
    param = (names, "List of stat names to remove"),
    embedding_doc = "Remove stats by name"
)]
pub fn remove_stats(names: Vec<String>) -> Result<usize> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    let _ = guard.remove_stats(&names);
    Ok(names.len())
}

/// Check if a stat exists.
#[tool(
    result_doc = "Stat {name} exists: {value}",
    param = (name, "Stat name to check"),
    embedding_doc = "Check if a stat exists"
)]
pub fn has_stat(name: String) -> bool {
    let data = save_data();
    let guard = data.lock().unwrap();
    guard.has_stat(&name)
}

/// List all stat names.
#[tool(
    result_doc = "Stats: {value:?}",
    embedding_doc = "List all stat names"
)]
pub fn list_stat_names() -> Vec<String> {
    let data = save_data();
    let guard = data.lock().unwrap();
    guard.list_stats().keys().cloned().collect()
}

/// Get the numeric value of a stat.
#[tool(
    result_doc = "{name} = {value}",
    param = (name, "Stat name"),
    embedding_doc = "Get the numeric value of a stat"
)]
pub fn get_stat_value(name: String) -> Result<i64> {
    let data = save_data();
    let guard = data.lock().unwrap();
    guard
        .get_stat(&name)
        .ok_or_else(|| anyhow!("Stat '{}' not found", name))?
        .value()
        .ok_or_else(|| anyhow!("Stat '{}' is not numeric", name))
}

/// Add a value to a numeric stat.
#[tool(
    result_doc = "{name} increased to {value}",
    param = (name, "Stat name"),
    param = (amount, "Amount to add"),
    embedding_doc = "Add a value to a numeric stat"
)]
pub fn add_to_stat(name: String, amount: i64) -> Result<i64> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard
        .get_stat_mut(&name)
        .ok_or_else(|| anyhow!("Stat '{}' not found", name))?
        .add(amount)
}

/// Subtract from a numeric stat.
#[tool(
    result_doc = "{name} decreased to {value}",
    param = (name, "Stat name"),
    param = (amount, "Amount to subtract"),
    embedding_doc = "Subtract from a numeric stat"
)]
pub fn sub_from_stat(name: String, amount: i64) -> Result<i64> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    guard
        .get_stat_mut(&name)
        .ok_or_else(|| anyhow!("Stat '{}' not found", name))?
        .sub(amount)
}

/// Set a numeric stat with limits.
#[tool(
    result_doc = "{name} set to {value} (range: {min}-{max})",
    param = (name, "Stat name"),
    param = (value, "Current value"),
    param = (min, "Minimum allowed value"),
    param = (max, "Maximum allowed value"),
    param = (include_min, "Whether min is inclusive"),
    param = (include_max, "Whether max is inclusive"),
    embedding_doc = "Set a numeric stat value with limits"
)]
pub fn set_stat_value(
    name: String,
    value: i64,
    min: i64,
    max: i64,
    include_min: bool,
    include_max: bool,
) -> Result<()> {
    let data = save_data();
    let mut guard = data.lock().unwrap();
    let lim = Lim::new(value, min, max, include_min, include_max)?;
    match guard.get_stat_mut(name.as_str()) {
        Some(stat) => stat.set_numeric(lim),
        None => bail!("Stat '{}' not found", name),
    }
}

// ============ Tool Registration ============

/// Get all built-in tool definitions (for non-Rig usage).
pub fn all_tools() -> Vec<Value> {
    vec![
        AddItem::definition(),
        RemoveItem::definition(),
        ClearInventory::definition(),
        HasItem::definition(),
        GetItemCount::definition(),
        AddNumericStat::definition(),
        AddTagStat::definition(),
        RemoveStats::definition(),
        HasStat::definition(),
        ListStatNames::definition(),
        GetStatValue::definition(),
        AddToStat::definition(),
        SubFromStat::definition(),
        SetStatValue::definition(),
    ]
    .into_iter()
    .map(|def| serde_json::to_value(def).unwrap_or_default())
    .collect()
}

// Note: Rig-enabled tools are automatically registered via the `linkme` crate.
// Any tool with `#[tool(embedding_doc = "...")]` will be included in
// `crate::llm::tool::get_rig_embeddable_tools()`.
