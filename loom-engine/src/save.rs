use std::path::PathBuf;
use hashbrown::HashMap;
use serde::{Serialize, Deserialize};
use chrono::Utc;
use anyhow::Result;
use crate::{actor::Stat, config::GameMode, story::Dialogue};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SaveMeta {
    pub timestamp: i64,
    pub note: String,
    pub main_filename: String,
}
impl SaveMeta {
    pub fn new(note: String) -> Self {
        let ts = Utc::now().timestamp_millis();
        Self {
            timestamp: ts,
            note,
            main_filename: format!("save_{}.json", ts),
        }
    }
    pub fn main_file_path(&self, project_root: &std::path::Path) -> PathBuf {
        project_root.join("saves").join(&self.main_filename)
    }
    pub fn meta_file_path(&self, project_root: &std::path::Path) -> PathBuf {
        project_root.join("saves").join(format!("save_{}.meta.json", self.timestamp))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SaveData {
    pub game_mode: GameMode,
    pub stats: HashMap<String, Stat>,
    pub inventory: HashMap<String, u64>,
    pub history: Vec<Dialogue>,
}
impl SaveData {
    pub fn new(game_mode: GameMode) -> Self {
        Self {
            game_mode,
            stats: HashMap::new(),
            inventory: HashMap::new(),
            history: Vec::new(),
        }
    }
    pub fn add_item(&mut self, name: &str, amount:u64) -> u64 {
        let count = self.inventory.entry(name.to_string()).or_insert(0);
        *count += amount;
        *count
    }
    pub fn remove_item(&mut self, name: &str, amount: u64) -> Result<u64, anyhow::Error> {
        match self.inventory.get_mut(name) {
            Some(count) if *count >= amount => {
                *count -= amount;
                Ok(amount)
            }
            Some(count) => {
                // Not enough items, remove all and return error
                let remaining = *count;
                self.inventory.remove(name);
                Err(anyhow::anyhow!(
                    "Not enough '{}' in inventory: have {}, tried to remove {}",
                    name, remaining, amount
                ))
            }
            None => {
                Err(anyhow::anyhow!(
                    "Item '{}' not found in inventory",
                    name
                ))
            }
        }
    }
    pub fn clear_inventory(&mut self) {
        self.inventory = HashMap::new();
    }
    pub fn has_item(&self, name: &str) -> bool {
        self.inventory.contains_key(name)
    }
    pub fn item_count(&self, name: &str) -> u64 {
        self.inventory.get(name).copied().unwrap_or(0)
    }
    pub fn add_stats(&mut self, stats: Vec<(String, Stat)>) {
        self.stats.extend(stats);
    }
    pub fn remove_stats(&mut self, stats: &[String]) -> anyhow::Result<()> {
        for stat in stats {
            if self.stats.remove(stat).is_none() {
                anyhow::bail!("Stat '{}' does not exist", stat);
            }
        }
        Ok(())
    }
    pub fn has_stat(&self, name: &str) -> bool {
        self.stats.contains_key(name)
    }
    pub fn get_stat(&self, name: &str) -> Option<&Stat> {
        self.stats.get(name)
    }
    pub fn get_stat_mut(&mut self, name: &str) -> Option<&mut Stat> {
        self.stats.get_mut(name)
    }
    pub fn list_stats(&self) -> &HashMap<String, Stat> {
        &self.stats
    }
    pub fn add_dialogue(&mut self, dialogue: Dialogue) {
        self.history.push(dialogue);
    }
    pub fn all_history(&self) -> &[Dialogue] {
        &self.history
    }
    pub fn recent_history(&self, num: usize) -> &[Dialogue] {
        let start = self.history.len().saturating_sub(num);
        &self.history[start..]
    }
    pub fn reset(&mut self, game_mode: GameMode) {
        *self = Self::new(game_mode);
    }
}