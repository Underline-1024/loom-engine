use std::{fs, path::PathBuf};
use anyhow::{bail, Result, Context};
use crate::{config::{GameMode, WorldConfig}, llm::{tool::builtin_tools::save_data, Narrator}, save::{SaveData, SaveMeta}};
use chrono::Utc;

const PROJECTS_DIR: &str = "projects";

pub struct Project {
    pub path: PathBuf,
    config: WorldConfig,
    timestamp: i64,
}
impl Project {
    pub async fn create(name: String, mode: GameMode, prompt: String, narrator: &Narrator) -> Result<Self> {
        let path = PathBuf::from(PROJECTS_DIR).join(&name);
        if path.exists() {
            return Err(anyhow::anyhow!("Project '{}' already exists", name));
        }
        fs::create_dir_all(&path)?;
        fs::create_dir_all(path.join("saves"))?;

        let config_path = path.join("world_config.json");
        let config = WorldConfig::new(name, mode, prompt.clone());
        fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        let _ = narrator.chat(&prompt).await;
        let data = save_data();
        let guard = data.lock().unwrap();

        let project = Self {
            path,
            config,
            timestamp: Utc::now().timestamp_millis(),
        };
        let note = "".to_string();
        let _ = project.create_save(note, guard.clone());
        Ok(project)
    }
    pub fn open(name: &str, update_timestamp: bool) -> Result<Self> {
        let path = PathBuf::from(PROJECTS_DIR).join(name);
        if !path.exists() {
            return Err(anyhow::anyhow!("Project '{}' does not exist", name));
        }

        let config_path = path.join("world_config.json");
        if !config_path.exists() {
            return Err(anyhow::anyhow!("Config file missing in project '{}'", name));
        }

        let json = fs::read_to_string(&config_path)?;
        let config: WorldConfig = serde_json::from_str(&json)?;

        let mut timestamp = config.timestamp;
        if update_timestamp {
            timestamp = Utc::now().timestamp_millis();
        }
        Ok(Self {
            path,
            config,
            timestamp,
        })
    }
    pub fn save_config(&mut self, config: WorldConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(&config)?;
        fs::write(self.path.join("world_config.json"), json)?;
        self.config = config;
        Ok(())
    }
    pub fn config(&self) -> &WorldConfig {
        &self.config
    }
    pub fn create_save(&self, note: String, data: SaveData) -> Result<SaveMeta> {
        let ts = Utc::now().timestamp_millis();
        
        let mut meta = SaveMeta::new(note.clone());
        meta.timestamp = ts;
        meta.main_filename = format!("save_{}.json", ts);

        let save_path = meta.main_file_path(&self.path);
        let meta_path = meta.meta_file_path(&self.path);

        if save_path.exists() || meta_path.exists() {
            bail!("Save with timestamp {} already exists.", ts);
        }

        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, meta_json)
            .with_context(|| format!("Failed to write meta file: {:?}", meta_path))?;

        let save_json = serde_json::to_string_pretty(&data)?;
        fs::write(&save_path, save_json)
            .with_context(|| format!("Failed to write save file: {:?}", save_path))?;

        Ok(meta)
    }
    pub fn list_saves(&self) -> Result<Vec<Result<SaveMeta>>> {
        let saves_dir = self.path.join("saves");
        
        if !saves_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results: Vec<Result<SaveMeta>> = Vec::new();

        for entry in fs::read_dir(&saves_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    results.push(Err(anyhow::anyhow!(e)));
                    continue;
                }
            };
            
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "meta.json") {
                let res = (|| -> Result<SaveMeta> {
                    let content = fs::read_to_string(&path)
                        .with_context(|| format!("Failed to read meta: {:?}", path))?;
                    
                    let meta: SaveMeta = serde_json::from_str(&content)
                        .with_context(|| format!("Failed to parse meta: {:?}", path))?;
                    
                    let main_path = meta.main_file_path(&self.path);
                    
                    if !main_path.exists() {
                        bail!("Main save file missing for meta: {:?}", path);
                    }

                    Ok(meta)
                })();

                results.push(res);
            }
        }
        
        results.sort_by(|a, b| {
            match (a, b) {
                // 两个都成功：比较时间戳 (b.cmp(a) 表示降序)
                (Ok(meta_a), Ok(meta_b)) => meta_b.timestamp.cmp(&meta_a.timestamp),
                // a 成功，b 失败：a 在前
                (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                // a 失败，b 成功：b 在前
                (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                // 两个都失败：保持相对顺序不变
                (Err(_), Err(_)) => std::cmp::Ordering::Equal,
            }
        });

        Ok(results)
    }
    pub fn load_save(&self, timestamp: i64) -> Result<SaveData> {
        let path = self.path.join("saves").join(format!("save_{}.json", timestamp));
        let json = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }
    pub fn save(&self, old_meta: SaveMeta, note: String, data: SaveData) -> Result<SaveMeta> {
        let new_ts = Utc::now().timestamp_millis();

        let old_save_path = old_meta.main_file_path(&self.path);
        let old_meta_path = old_meta.meta_file_path(&self.path);

        let new_save_filename = format!("save_{}.json", new_ts);
        let new_save_path = self.path.join("saves").join(&new_save_filename);
        
        let new_meta_filename = format!("save_{}.meta.json", new_ts);
        let new_meta_path = self.path.join("saves").join(&new_meta_filename);

        if !old_save_path.exists() {
            bail!("Old save file not found: {:?}", old_save_path);
        }
        if !old_meta_path.exists() {
            bail!("Old meta file not found: {:?}", old_meta_path);
        }

        let mut new_meta = SaveMeta::new(note);
        new_meta.timestamp = new_ts;
        new_meta.main_filename = new_save_filename.clone();

        let save_json = serde_json::to_string_pretty(&data)?;
        fs::write(&old_save_path, &save_json)
            .with_context(|| format!("Failed to write content to {:?}", old_save_path))?;
        
        fs::rename(&old_save_path, &new_save_path)
            .with_context(|| format!("Failed to rename save {:?} to {:?}", old_save_path, new_save_path))?;

        let meta_json = serde_json::to_string_pretty(&new_meta)?;
        fs::write(&old_meta_path, &meta_json)
            .with_context(|| format!("Failed to write content to {:?}", old_meta_path))?;
        
        fs::rename(&old_meta_path, &new_meta_path)
            .with_context(|| format!("Failed to rename meta {:?} to {:?}", old_meta_path, new_meta_path))?;

        Ok(new_meta)
    }
    pub fn delete_save(&self, timestamp: i64) -> Result<()> {
        let save_path = self.path.join("saves").join(format!("save_{}.json", timestamp));
        let meta_path = self.path.join("saves").join(format!("save_{}.meta.json", timestamp));

        let mut errors = Vec::new();

        if save_path.exists() {
            if let Err(e) = fs::remove_file(&save_path) {
                errors.push(format!("Save file: {}", e));
            }
        }
        
        if meta_path.exists() {
            if let Err(e) = fs::remove_file(&meta_path) {
                errors.push(format!("Meta file: {}", e));
            }
        }

        if !errors.is_empty() {
            bail!("Failed to delete save {}: {}", timestamp, errors.join("; "));
        }

        Ok(())
    }
    pub fn name(&self) -> &str {
        &self.config.name
    }
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

pub fn list_projects() -> Result<Vec<Result<Project>>> {
    let projects_path = PathBuf::from(PROJECTS_DIR);
    if !projects_path.exists() {
        return Ok(vec![]);
    }
    let mut projects: Vec<Result<Project>> = Vec::new();
    for entry in fs::read_dir(projects_path)? {
        let path = entry?.path();
        if path.is_dir() {
            if let Some(dir_name) = path.file_name() {
                if let Some(project_name) = dir_name.to_str() {
                    let project = Project::open(project_name, false);
                    projects.push(project);
                }
            }
        }
    }
    projects.sort_by(|a, b| {
        match (a, b) {
            // 两个都成功：按时间戳倒序
            (Ok(p1), Ok(p2)) => p2.timestamp.cmp(&p1.timestamp),
            // 一个成功，一个失败：成功的排前面
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            // 两个都失败：保持相对顺序稳定
            (Err(_), Err(_)) => std::cmp::Ordering::Equal,
        }
    });
    Ok(projects)
}