use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::{actor::Stat, llm::Narrator, save::SaveData};
use crate::story::Dialogue;
use crate::llm::tool::builtin_tools::save_data;

use super::{App, Route};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Stats,
    Tags,
    Inventory,
    Dialogue,
}

#[derive(Debug, Clone)]
pub struct GameplayState {
    pub selected_save_data: Option<SaveData>,
    pub scroll_offset: usize,
    pub dialogue_scroll_offset: usize,
    pub input: String,
    pub is_editing: bool,
    pub selected_column: ColumnType,
    pub pending_llm_response: Option<String>,
    pub is_processing: bool,
}

impl Default for GameplayState {
    fn default() -> Self {
        Self {
            selected_save_data: None,
            scroll_offset: 0,
            dialogue_scroll_offset: 0,
            input: String::new(),
            is_editing: false,
            selected_column: ColumnType::Stats,
            pending_llm_response: None,
            is_processing: false,
        }
    }
}

impl GameplayState {
    pub fn new(save_data: SaveData) -> Self {
        Self {
            selected_save_data: Some(save_data),
            scroll_offset: 0,
            dialogue_scroll_offset: 0,
            input: String::new(),
            is_editing: false,
            selected_column: ColumnType::Stats,
            pending_llm_response: None,
            is_processing: false,
        }
    }
}

impl App {
    pub fn render_gameplay(&mut self, frame: &mut Frame, area: Rect) {
        let gameplay_state = match &self.route {
            Route::Gameplay(state) => state,
            _ => {
                let error_text = "Error: Not in Gameplay mode";
                let paragraph = Paragraph::new(error_text)
                    .style(Style::default().fg(Color::Red))
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(paragraph, area);
                return;
            }
        };

        let save_data = match &gameplay_state.selected_save_data {
            Some(data) => data,
            None => {
                let error_text = "No save data loaded. Please load a save file.";
                let paragraph = Paragraph::new(error_text)
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(paragraph, area);
                return;
            }
        };

        // 主边框
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .title(" Gameplay ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        
        let inner_area = outer_block.inner(area);
        frame.render_widget(outer_block, area);
        
        // 主布局：上(列+对话) / 输入 / 帮助
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(75),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner_area);
        
        // 上部分：列(左) / 对话历史(右)
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(main_chunks[0]);
        
        // 左侧：三列
        let left_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ])
            .split(top_chunks[0]);
        
        let scroll_offset = gameplay_state.scroll_offset;
        let dialogue_scroll_offset = gameplay_state.dialogue_scroll_offset;
        
        // 计算可见行数
        let visible_rows = (left_chunks[0].height as usize).saturating_sub(2);
        let visible_rows = visible_rows.max(3);
        
        // 渲染三列
        Self::render_numeric_stats(save_data, frame, left_chunks[0], scroll_offset, visible_rows);
        Self::render_tag_stats(save_data, frame, left_chunks[1], scroll_offset, visible_rows);
        Self::render_inventory(save_data, frame, left_chunks[2], scroll_offset, visible_rows);
        
        // 右侧：对话历史
        Self::render_dialogue_history(save_data, frame, top_chunks[1], dialogue_scroll_offset);
        
        // 底部：输入和帮助
        Self::render_input_area(frame, main_chunks[1], gameplay_state);
        Self::render_help_bar(frame, main_chunks[2]);
    }

    fn render_numeric_stats(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect, 
        scroll_offset: usize,
        visible_rows: usize,
    ) {
        let stats: Vec<(&String, &Stat)> = save_data.stats
            .iter()
            .filter(|(_, stat)| matches!(stat, Stat::Numeric(_)))
            .collect();
        
        // 标题
        let title = Paragraph::new(" Stats ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(title, title_area);
        
        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        
        if stats.is_empty() {
            let empty_text = "No numeric stats";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, content_area);
            return;
        }
        
        let start = scroll_offset.min(stats.len().saturating_sub(1));
        let end = (start + visible_rows).min(stats.len());
        let visible_stats = &stats[start..end];
        
        let items: Vec<ListItem> = visible_stats.iter()
            .map(|(name, stat)| {
                if let Stat::Numeric(lim) = stat {
                    let current = *lim.value();
                    let max = *lim.max();
                    let percentage = if max > 0 { (current as f64 / max as f64 * 100.0) as u16 } else { 0 };
                    
                    let bar_width = area.width.saturating_sub(6) as usize;
                    let bar_width = bar_width.min(30);
                    let filled = (bar_width as f64 * percentage as f64 / 100.0) as usize;
                    let empty = bar_width.saturating_sub(filled);
                    
                    let bar = if bar_width > 0 && percentage > 0 {
                        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
                    } else {
                        String::new()
                    };
                    
                    let text = if !bar.is_empty() {
                        format!("{}: {}/{} {}", name, current, max, bar)
                    } else {
                        format!("{}: {}/{}", name, current, max)
                    };
                    
                    let color = if percentage > 70 { Color::Green } 
                                else if percentage > 40 { Color::Yellow } 
                                else { Color::Red };
                    
                    ListItem::new(Line::from(vec![
                        Span::styled(text, Style::default().fg(color))
                    ]))
                } else {
                    ListItem::new("")
                }
            })
            .collect();
        
        let list = List::new(items)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, content_area);
        
        if stats.len() > visible_rows {
            let scroll_indicator = format!("{}/{}", start + 1, stats.len());
            let indicator = Paragraph::new(scroll_indicator.clone())
                .style(Style::default().fg(Color::DarkGray));
            let indicator_area = Rect {
                x: content_area.x + content_area.width - scroll_indicator.len() as u16 - 1,
                y: content_area.y + content_area.height - 1,
                width: scroll_indicator.len() as u16 + 1,
                height: 1,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_tag_stats(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect, 
        scroll_offset: usize,
        visible_rows: usize,
    ) {
        let tags: Vec<(&String, &Stat)> = save_data.stats
            .iter()
            .filter(|(_, stat)| matches!(stat, Stat::Tag))
            .collect();
        
        // 标题
        let title = Paragraph::new(" Tags ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(title, title_area);
        
        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        
        if tags.is_empty() {
            let empty_text = "No tags";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, content_area);
            return;
        }
        
        let start = scroll_offset.min(tags.len().saturating_sub(1));
        let end = (start + visible_rows).min(tags.len());
        let visible_tags = &tags[start..end];
        
        let items: Vec<ListItem> = visible_tags.iter()
            .map(|(name, _)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("• {}", name), Style::default().fg(Color::Cyan))
                ]))
            })
            .collect();
        
        let list = List::new(items)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, content_area);
        
        if tags.len() > visible_rows {
            let scroll_indicator = format!("{}/{}", start + 1, tags.len());
            let indicator = Paragraph::new(scroll_indicator.clone())
                .style(Style::default().fg(Color::DarkGray));
            let indicator_area = Rect {
                x: content_area.x + content_area.width - scroll_indicator.len() as u16 - 1,
                y: content_area.y + content_area.height - 1,
                width: scroll_indicator.len() as u16 + 1,
                height: 1,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_inventory(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect, 
        scroll_offset: usize,
        visible_rows: usize,
    ) {
        let inventory: Vec<(&String, &u64)> = save_data.inventory
            .iter()
            .collect();
        
        // 标题
        let title = Paragraph::new(" Inventory ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(title, title_area);
        
        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        
        if inventory.is_empty() {
            let empty_text = "Inventory empty";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, content_area);
            return;
        }
        
        let start = scroll_offset.min(inventory.len().saturating_sub(1));
        let end = (start + visible_rows).min(inventory.len());
        let visible_items = &inventory[start..end];
        
        let items: Vec<ListItem> = visible_items.iter()
            .map(|(name, count)| {
                let text = if **count > 1 {
                    format!("{} ×{}", name, count)
                } else {
                    name.to_string()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(text, Style::default().fg(Color::Yellow))
                ]))
            })
            .collect();
        
        let list = List::new(items)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, content_area);
        
        if inventory.len() > visible_rows {
            let scroll_indicator = format!("{}/{}", start + 1, inventory.len());
            let indicator = Paragraph::new(scroll_indicator.clone())
                .style(Style::default().fg(Color::DarkGray));
            let indicator_area = Rect {
                x: content_area.x + content_area.width - scroll_indicator.len() as u16 - 1,
                y: content_area.y + content_area.height - 1,
                width: scroll_indicator.len() as u16 + 1,
                height: 1,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_dialogue_history(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect,
        scroll_offset: usize,
    ) {
        let history = &save_data.history;
        
        // 标题
        let title = Paragraph::new(" Dialogue History ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(title, title_area);
        
        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(2),
        };
        
        if history.is_empty() {
            let empty_text = "No dialogue history";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, content_area);
            return;
        }
        
        let max_lines = content_area.height as usize;
        let total_lines = history.len();
        
        // 使用 scroll_offset 控制显示范围
        let start = if total_lines > max_lines {
            scroll_offset.min(total_lines - max_lines)
        } else {
            0
        };
        let end = (start + max_lines).min(total_lines);
        let visible: Vec<&Dialogue> = history.iter().skip(start).take(end - start).collect();
        
        let lines: Vec<Line> = visible.iter()
            .map(|dialogue| {
                let timestamp = chrono::DateTime::from_timestamp_millis(dialogue.timestamp)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| dialogue.timestamp.to_string());
                
                let (speaker_style, speaker_prefix) = match dialogue.speaker.as_str() {
                    "Narrator" => (Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD), "📖"),
                    "Player" => (Style::default().fg(Color::Green).add_modifier(Modifier::BOLD), "👤"),
                    "System" => (Style::default().fg(Color::Red), "⚙️"),
                    _ => (Style::default().fg(Color::Cyan), "❓"),
                };
                
                let mut spans = vec![
                    Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} ", speaker_prefix), speaker_style),
                    Span::styled(format!("{}:", dialogue.speaker), speaker_style),
                ];
                
                if let Some(content) = &dialogue.content {
                    spans.push(Span::from(" "));
                    spans.push(Span::from(content.clone()));
                }
                
                Line::from(spans)
            })
            .collect();
        
        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: true });
        
        frame.render_widget(paragraph, content_area);
        
        if total_lines > max_lines {
            let scroll_indicator = format!("{}/{}", start + 1, total_lines);
            let indicator = Paragraph::new(scroll_indicator.clone())
                .style(Style::default().fg(Color::DarkGray));
            let indicator_area = Rect {
                x: content_area.x + content_area.width - scroll_indicator.len() as u16 - 1,
                y: content_area.y + content_area.height - 1,
                width: scroll_indicator.len() as u16 + 1,
                height: 1,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_input_area(frame: &mut Frame, area: Rect, gameplay_state: &GameplayState) {
        let prefix = "> ";
        let input_text = if gameplay_state.is_editing {
            if gameplay_state.is_processing {
                format!("{}Processing...", prefix)
            } else {
                format!("{}{}", prefix, gameplay_state.input)
            }
        } else {
            format!("{}[Press Enter to input]", prefix)
        };
        
        let style = if gameplay_state.is_editing && !gameplay_state.is_processing {
            Style::default().fg(Color::White)
        } else if gameplay_state.is_processing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        
        let paragraph = Paragraph::new(input_text)
            .style(style)
            .wrap(Wrap { trim: true })
            .block(Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Gray)));
        
        frame.render_widget(paragraph, area);
    }

    fn render_help_bar(frame: &mut Frame, area: Rect) {
        let help_text = "Enter:Input | ↑↓:Scroll | Tab:Switch Column | Esc:Cancel | Ctrl+Q:Quit";
        let paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Gray)));
        
        frame.render_widget(paragraph, area);
    }

    pub async fn handle_gameplay_input(&mut self, event: KeyEvent, narrator: &Narrator) {
        // 先检查是否有待处理的响应
        let pending_response = if let Route::Gameplay(state) = &mut self.route {
            state.pending_llm_response.take()
        } else {
            None
        };
        
        if let Some(response) = pending_response {
            self.handle_llm_response(&response).await;
            return;
        }
        
        // 处理输入
        match &mut self.route {
            Route::Gameplay(state) => {
                if state.is_editing && !state.is_processing {
                    self.handle_editing_input(event, narrator).await;
                } else {
                    self.handle_navigation_input(event);
                }
            },
            _ => {},
        }
    }

    async fn handle_editing_input(
        &mut self,
        event: KeyEvent,
        narrator: &Narrator,
    ) {
        let (input, should_process) = {
            if let Route::Gameplay(state) = &mut self.route {
                match event.code {
                    KeyCode::Enter => {
                        if !state.input.is_empty() && !state.is_processing {
                            state.is_editing = false;
                            let input = state.input.clone();
                            state.input.clear();
                            (input, true)
                        } else {
                            (String::new(), false)
                        }
                    },
                    KeyCode::Esc => {
                        state.is_editing = false;
                        state.input.clear();
                        (String::new(), false)
                    },
                    KeyCode::Backspace => {
                        state.input.pop();
                        (String::new(), false)
                    },
                    KeyCode::Char(c) => {
                        // 支持所有 Unicode 字符（包括中文）
                        state.input.push(c);
                        (String::new(), false)
                    },
                    _ => (String::new(), false),
                }
            } else {
                (String::new(), false)
            }
        };
        
        if should_process && !input.is_empty() {
            if input.starts_with('/') {
                self.handle_command(&input).await;
            } else {
                self.handle_player_input(&input, narrator).await;
            }
        }
    }

    async fn handle_command(&mut self, command: &str) {
        match command {
            "/help" => {
                self.show_help_message();
            },
            "/save" => {
                self.save_game().await;
            },
            "/load" => {
                self.load_game().await;
            },
            "/quit" => {
                self.route = Route::MainMenu;
            },
            _ => {
                self.add_dialogue(
                    "System",
                    format!("Unknown command: {}", command),
                );
            },
        }
    }

    async fn handle_player_input(&mut self, input: &str, narrator: &Narrator) {
        // 1. 在锁内：添加玩家消息到界面，并克隆原始历史
        let history_clone = {
            let data = save_data();
            let guard = data.lock().unwrap();
            // 添加玩家消息到界面（独立副本）
            self.add_dialogue("Player", input.to_string());
            // 克隆当前历史，供 LLM 使用
            guard.raw_history.clone()
        }; // 锁释放
    
        // 2. 设置 UI 处理状态
        if let Route::Gameplay(state) = &mut self.route {
            state.is_processing = true;
        }
    
        // 3. 调用 LLM（不持有锁）
        let mut history = history_clone;
        match narrator.chat(input, &mut history).await {
            Ok(response) => {
                // 4. 写回更新后的历史（短暂加锁）
                {
                    let data = save_data();
                    let mut guard = data.lock().unwrap();
                    guard.raw_history = history;
                }
                // 5. 应用响应（解析并更新界面状态、stats、inventory）
                self.apply_llm_response(&response).await;
            }
            Err(e) => {
                self.add_dialogue("System", format!("Error: {}", e));
            }
        }
    
        // 6. 清除处理状态
        if let Route::Gameplay(state) = &mut self.route {
            state.is_processing = false;
        }
    }

    fn add_dialogue(&mut self, speaker: &str, content: String) {
        if let Route::Gameplay(state) = &mut self.route {
            if let Some(save_data) = &mut state.selected_save_data {
                save_data.history.push(Dialogue::new(
                    speaker.to_string(),
                    Some(content),
                ));
                // 自动滚动到最新消息
                state.dialogue_scroll_offset = save_data.history.len().saturating_sub(1);
            }
        }
    }

    fn show_help_message(&mut self) {
        let help_text = vec![
            "Available commands:".to_string(),
            "  /help  - Show this help message".to_string(),
            "  /save  - Save current game".to_string(),
            "  /load  - Load saved game".to_string(),
            "  /quit  - Return to main menu".to_string(),
            "".to_string(),
            "Controls:".to_string(),
            "  Enter - Start typing".to_string(),
            "  ↑/↓   - Scroll".to_string(),
            "  Tab   - Switch columns".to_string(),
            "  Esc   - Cancel input".to_string(),
            "  Ctrl+Q - Quit".to_string(),
        ].join("\n");
        
        self.add_dialogue("System", help_text);
    }

    async fn save_game(&mut self) {
        // 使用全局 save_data 获取当前数据并保存
        let data = save_data();
        let guard = data.lock().unwrap();
        let current_data = guard.clone();
        drop(guard);

        if let Route::Gameplay(state) = &mut self.route {
            state.selected_save_data = Some(current_data);
            if let Some(save_meta_id) = self.selected_save_meta_id {
                if let Ok(project) = self.get_mut_project() {
                    if let Ok(save_meta) = &mut project.load_save_meta(save_meta_id) {
                        let _ = project.save(true, Some(save_meta), None, None).await;
                    }
                }
            }
            
            self.add_dialogue("System", "Game saved successfully!".to_string());
        }
    }

    async fn load_game(&mut self) {
        // 从全局 save_data 加载最新数据
        let data = save_data();
        let guard = data.lock().unwrap();
        let loaded_data = guard.clone();
        drop(guard);

        if let Route::Gameplay(state) = &mut self.route {
            state.selected_save_data = Some(loaded_data);
            self.add_dialogue("System", "Game loaded successfully!".to_string());
        }
    }

    async fn apply_llm_response(&mut self, response: &str) {
        // 解析并应用 LLM 响应
        if let Ok(parsed) = serde_json::from_str::<Value>(response) {
            // 处理对话内容
            if let Some(content) = parsed.get("content").and_then(|c| c.as_str()) {
                self.add_dialogue("Narrator", content.to_string());
            }
            
            // 收集需要更新的数据
            let mut stats_updates = Vec::new();
            let mut inventory_updates = Vec::new();
            
            if let Route::Gameplay(state) = &self.route {
                if let Some(_save_data) = &state.selected_save_data {
                    if let Some(stats) = parsed.get("stats").and_then(|s| s.as_object()) {
                        for (key, value) in stats {
                            if let Some(num) = value.as_i64() {
                                stats_updates.push((key.clone(), num));
                            }
                        }
                    }
                    
                    if let Some(inventory) = parsed.get("inventory").and_then(|i| i.as_object()) {
                        for (key, value) in inventory {
                            if let Some(count) = value.as_u64() {
                                inventory_updates.push((key.clone(), count));
                            }
                        }
                    }
                }
            }
            
            // 应用状态更新
            if let Route::Gameplay(state) = &mut self.route {
                if let Some(save_data) = &mut state.selected_save_data {
                    for (key, value) in stats_updates {
                        if let Some(stat) = save_data.stats.get_mut(&key) {
                            if let Stat::Numeric(lim) = stat {
                                let _ = lim.set_value(value);
                            }
                        }
                    }
                    
                    for (key, count) in inventory_updates {
                        if count > 0 {
                            save_data.inventory.insert(key, count);
                        } else {
                            save_data.inventory.remove(&key);
                        }
                    }
                }
            }
        } else {
            // 如果不是 JSON，直接作为叙述
            self.add_dialogue("Narrator", response.to_string());
        }
    }

    async fn handle_llm_response(&mut self, response: &str) {
        self.apply_llm_response(response).await;
    }
    
    fn handle_navigation_input(&mut self, event: KeyEvent) {
        if let Route::Gameplay(state) = &mut self.route {
            match event.code {
                KeyCode::Enter => {
                    state.is_editing = true;
                    state.input.clear();
                },
                KeyCode::Up => {
                    if state.selected_column == ColumnType::Dialogue {
                        if state.dialogue_scroll_offset > 0 {
                            state.dialogue_scroll_offset -= 1;
                        }
                    } else {
                        if state.scroll_offset > 0 {
                            state.scroll_offset -= 1;
                        }
                    }
                },
                KeyCode::Down => {
                    if state.selected_column == ColumnType::Dialogue {
                        if let Some(save_data) = &state.selected_save_data {
                            let max_lines = 10;
                            let total_lines = save_data.history.len();
                            let max_scroll = total_lines.saturating_sub(max_lines);
                            if state.dialogue_scroll_offset < max_scroll {
                                state.dialogue_scroll_offset += 1;
                            }
                        }
                    } else {
                        let save_data = match &state.selected_save_data {
                            Some(data) => data,
                            None => return,
                        };
                        
                        let max_items = match state.selected_column {
                            ColumnType::Stats => save_data.stats.iter()
                                .filter(|(_, stat)| matches!(stat, Stat::Numeric(_)))
                                .count(),
                            ColumnType::Tags => save_data.stats.iter()
                                .filter(|(_, stat)| matches!(stat, Stat::Tag))
                                .count(),
                            ColumnType::Inventory => save_data.inventory.len(),
                            ColumnType::Dialogue => 0,
                        };
                        
                        let visible_rows = 10;
                        let max_scroll = max_items.saturating_sub(visible_rows);
                        if state.scroll_offset < max_scroll {
                            state.scroll_offset += 1;
                        }
                    }
                },
                KeyCode::Tab => {
                    state.selected_column = match state.selected_column {
                        ColumnType::Stats => ColumnType::Tags,
                        ColumnType::Tags => ColumnType::Inventory,
                        ColumnType::Inventory => ColumnType::Dialogue,
                        ColumnType::Dialogue => ColumnType::Stats,
                    };
                    state.scroll_offset = 0;
                    state.dialogue_scroll_offset = 0;
                },
                KeyCode::Char('q') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.route = Route::MainMenu;
                },
                _ => {},
            }
        }
    }
}