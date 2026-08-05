use std::{fs::{self, rename}, path::PathBuf};
use anyhow::{bail, Result, Context};
use crate::{config::{GameMode, WorldConfig}, llm::{tool::builtin_tools::{reset_save_data, save_data}, Narrator}, save::{SaveData, SaveMeta}};
use chrono::Utc;

const PROJECTS_DIR: &str = "projects";

#[derive(Debug, Clone)]
pub struct Project {
    pub path: PathBuf,
    config: WorldConfig,
    timestamp: i64,
}
impl Project {
    pub async fn create(name: String, game_mode: GameMode, prompt: &str, narrator: &Narrator) -> Result<Self> {
        let path = PathBuf::from(PROJECTS_DIR).join(&name);
        if path.exists() {
            return Err(anyhow::anyhow!("Project '{}' already exists", name));
        }
        fs::create_dir_all(&path)?;
        fs::create_dir_all(path.join("saves"))?;

        let config_path = path.join("world_config.json");
        let config = WorldConfig::new(name, game_mode.clone(), prompt.to_string());
        fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        let project = Self {
            path,
            config,
            timestamp: Utc::now().timestamp_millis(),
        };
        let _ = project.save(false, None, None, Some((narrator, game_mode, prompt))).await;
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
            
            if path.to_str().map_or(false, |s| s.ends_with(".meta.json")) {
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
    
    pub fn load_save_meta(&self, timestamp: i64) -> Result<SaveMeta> {
        let path = self.path.join("saves").join(format!("save_{}.meta.json", timestamp));
        let json = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }
    
    /// 保存存档（统一入口）
    /// - overwrite: true 覆盖当前存档，false 创建新存档
    /// - meta: 存档元数据
    ///   * overwrite 为 true 时：要覆盖的存档
    ///   * overwrite 为 false 时：如果提供则加载该存档数据作为另存为的基础
    /// - note: 存档备注，None 则为空字符串
    /// - narrator_gamemode_prompt: 用于初始化世界数据（仅创建新存档且不加载旧数据时有效）
    pub async fn save(
        &self, 
        overwrite: bool,
        meta: Option<&mut SaveMeta>,
        note: Option<String>, 
        narrator_gamemode_prompt: Option<(&Narrator, GameMode, &str)>
    ) -> Result<SaveMeta> {
        let data = save_data();
        let note = note.unwrap_or_default();

        if overwrite {
            // 覆盖模式：直接修改原文件
            let meta = meta.ok_or_else(|| anyhow::anyhow!("Meta is required for overwrite"))?;
            
            let save_path = meta.main_file_path(&self.path);
            let meta_path = meta.meta_file_path(&self.path);

            if !save_path.exists() {
                bail!("Save file not found: {:?}", save_path);
            }
            if !meta_path.exists() {
                bail!("Meta file not found: {:?}", meta_path);
            }

            // 更新时间戳和备注
            let ts = Utc::now().timestamp_millis();
            let new_save_path = save_path.parent().unwrap().join(format!("save_{}.json", ts));
            let new_meta_path = meta_path.parent().unwrap().join(format!("save_{}.meta.json", ts));
            meta.timestamp = ts;
            meta.note = note;
            meta.main_filename = new_save_path.file_name().unwrap().to_str().unwrap().into();

            // 覆盖写入
            let guard = data.lock().unwrap();
            let save_json = serde_json::to_string_pretty(&guard.clone())?;
            fs::write(&save_path, save_json)
                .with_context(|| format!("Failed to write save file: {:?}", save_path))?;

            let meta_json = serde_json::to_string_pretty(&meta)?;
            fs::write(&meta_path, meta_json)
                .with_context(|| format!("Failed to write meta file: {:?}", meta_path))?;

            rename(save_path, new_save_path)?;
            rename(meta_path, new_meta_path)?;

            Ok(meta.clone())
        } else {
            // 创建新存档
            if let Some(meta) = meta {
                // 另存为：加载旧存档数据
                let old_data = self.load_save(meta.timestamp)?;
                let mut guard = data.lock().unwrap();
                *guard = old_data;
            } else if let Some((n, game_mode, prompt)) = narrator_gamemode_prompt {
                // 创建新存档：初始化世界数据
                reset_save_data(game_mode);

                let mut raw_history = Vec::new();

                let init_stats_prompt = format!(
                    "[WORLD SETTING]\n{}\n\n\
                    [INITIALIZATION DIRECTIVE]\n\
                    Now that the scene is set, initialize the player character's starting attributes and inventory based on the world setting and the current situation.\n\n\
                    CRITICAL TOOL CONSTRAINT: You MUST use the designated stat/inventory modification tools to set these values. Do NOT describe them in plain text or add any narrative.\n\n\
                    CRITICAL LANGUAGE CONSTRAINT: ALL generated content (stat names, item names, tags) MUST be written in the same language as the world setting provided above.\n\n\
                    Guidelines:\n\
                    1. Stats and items must be consistent with the world setting.\n\
                    2. Only perform tool calls. Do not output any additional text.",
                    prompt
                );
                let _ = n.stream_narrate(&init_stats_prompt, &mut raw_history).await;

                let init_prologue_prompt = format!(
                    "[WORLD SETTING]\n{}\n\n\
                    [INITIALIZATION DIRECTIVE]\n\
                    CRITICAL LANGUAGE CONSTRAINT: ALL generated content MUST be written in the same language as the world setting provided above.\n\n\
                    Based on this world setting, generate an immersive opening prologue.\n\n\
                    CRITICAL TOOL CONSTRAINT: You MUST use the designated dialogue/narration tool to output the prologue. Do NOT return the text as a plain assistant message.\n\n\
                    LENGTH GUIDANCE: The prologue should be concise — around 1 to 3 narration tool calls. If you can capture the atmosphere in a single well-crafted call, that's perfectly fine. Keep each call focused and evocative rather than sprawling. Avoid repeating the same idea across multiple calls.\n\n\
                    Rules:\n\
                    1. Focus solely on environmental descriptions, atmosphere, and background lore.\n\
                    2. Do NOT describe any actions, thoughts, or choices of the player character.\n\
                    3. Do NOT end with a question or a prompt asking for the player's next action.\n\
                    4. Output ONLY the raw text of the prologue via the tool. Do not include any conversational filler, greetings, meta-text, or attribute/item definitions.\n\
                    5. This prologue is the FINAL initialization step. Once it is generated, the initialization phase is complete and the game officially begins. The prologue must end at a natural starting point where the player character is present in the scene and ready to act, so the player can immediately take over from this moment. Do NOT add any closing remarks, summaries, or transitional text indicating the prologue has ended.",
                    prompt
                );
                let _ = n.stream_narrate(&init_prologue_prompt, &mut raw_history).await;

                
                let data = save_data();
                let mut guard = data.lock().unwrap();
                guard.raw_history = raw_history;
            }
            // 如果既没有 meta 也没有 narrator_gamemode_prompt，使用当前全局 save_data

            let ts = Utc::now().timestamp_millis();
            
            let mut new_meta = SaveMeta::new(note);
            new_meta.timestamp = ts;
            new_meta.main_filename = format!("save_{}.json", ts);

            let save_path = new_meta.main_file_path(&self.path);
            let meta_path = new_meta.meta_file_path(&self.path);

            if save_path.exists() || meta_path.exists() {
                bail!("Save with timestamp {} already exists.", ts);
            }

            let guard = data.lock().unwrap();
            let meta_json = serde_json::to_string_pretty(&new_meta)?;
            fs::write(&meta_path, meta_json)
                .with_context(|| format!("Failed to write meta file: {:?}", meta_path))?;

            let save_json = serde_json::to_string_pretty(&guard.clone())?;
            fs::write(&save_path, save_json)
                .with_context(|| format!("Failed to write save file: {:?}", save_path))?;

            Ok(new_meta)
        }
    }
    pub fn update_save_note(&self, meta: &mut SaveMeta, note: String) -> Result<()> {
        let meta_path = meta.meta_file_path(&self.path);
        
        if !meta_path.exists() {
            bail!("Meta file not found: {:?}", meta_path);
        }
        
        meta.note = note;
        
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, meta_json)
            .with_context(|| format!("Failed to write meta file: {:?}", meta_path))?;
        
        Ok(())
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

    pub fn update_config(&mut self, name: String, mode: GameMode, prompt: String) -> Result<()> {
        let old_name = self.config.name.clone();
        
        // 1. 如果名字改变，需要重命名目录
        if old_name != name {
            let old_path = self.path.clone();
            // 获取父目录 (通常是 "projects") 并拼接新名字
            let new_path = old_path.parent().unwrap_or(&PathBuf::from(PROJECTS_DIR)).join(&name);
            
            if new_path.exists() {
                bail!("Project '{}' already exists", name);
            }
            
            fs::rename(&old_path, &new_path)
                .with_context(|| format!("Failed to rename project folder to '{}'", name))?;
            self.path = new_path;
        }
        
        // 2. 构建新的 config 并保存 (保持原创建时间 timestamp 不变)
        let new_config = WorldConfig {
            name,
            mode,
            prompt,
            timestamp: self.config.timestamp, 
        };
        
        self.save_config(new_config)?;
        
        Ok(())
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