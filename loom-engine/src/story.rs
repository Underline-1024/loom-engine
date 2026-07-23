use serde::{Serialize, Deserialize};
use chrono::Utc;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dialogue {
    pub speaker: String,
    pub content: Option<String>,
    pub timestamp: i64,
}
impl Dialogue {
    pub fn new(speaker: String, content: Option<String>) -> Self {
        Self {
            speaker,
            content,
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}