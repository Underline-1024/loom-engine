use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
}

#[derive(Debug, Clone)]
pub struct GameplayState {
    pub selected_save_data: Option<SaveData>,
    pub scroll_offset: usize,
    pub input: String,
    pub is_editing: bool,
    pub selected_column: ColumnType,
    pub pending_llm_response: Option<String>,
}

impl Default for GameplayState {
    fn default() -> Self {
        Self {
            selected_save_data: None,
            scroll_offset: 0,
            input: String::new(),
            is_editing: false,
            selected_column: ColumnType::Stats,
            pending_llm_response: None,
        }
    }
}

impl GameplayState {
    pub fn new(save_data: SaveData) -> Self {
        Self {
            selected_save_data: Some(save_data),
            scroll_offset: 0,
            input: String::new(),
            is_editing: false,
            selected_column: ColumnType::Stats,
            pending_llm_response: None,
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

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .title(" Gameplay ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        
        let inner_area = outer_block.inner(area);
        frame.render_widget(outer_block, area);
        
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner_area);
        
        let top_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .split(main_chunks[0]);
        
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(25),
                Constraint::Percentage(35),
            ])
            .split(top_chunks[0]);
        
        let scroll_offset = gameplay_state.scroll_offset;
        
        let visible_rows = (columns[0].height as usize).saturating_sub(2);
        let visible_rows = visible_rows.max(3);
        
        let stats_count = save_data.stats.iter()
            .filter(|(_, stat)| matches!(stat, Stat::Numeric(_)))
            .count();
        let tags_count = save_data.stats.iter()
            .filter(|(_, stat)| matches!(stat, Stat::Tag))
            .count();
        let inventory_count = save_data.inventory.len();
        
        Self::render_numeric_stats(save_data, frame, columns[0], scroll_offset, stats_count, visible_rows);
        Self::render_tag_stats(save_data, frame, columns[1], scroll_offset, tags_count, visible_rows);
        Self::render_inventory(save_data, frame, columns[2], scroll_offset, inventory_count, visible_rows);
        
        Self::render_dialogue_history(save_data, frame, top_chunks[1]);
        Self::render_input_area(frame, main_chunks[1], gameplay_state);
        Self::render_help_bar(frame, main_chunks[2]);
    }

    fn render_numeric_stats(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect, 
        scroll_offset: usize,
        total_items: usize,
        visible_rows: usize,
    ) {
        let stats: Vec<(&String, &Stat)> = save_data.stats
            .iter()
            .filter(|(_, stat)| matches!(stat, Stat::Numeric(_)))
            .collect();
        
        if stats.is_empty() {
            let empty_text = "No numeric stats";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
            frame.render_widget(paragraph, area);
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
                    let bar_width = bar_width.min(50);
                    let filled = (bar_width as f64 * percentage as f64 / 100.0) as usize;
                    let empty = bar_width.saturating_sub(filled);
                    
                    let bar = if bar_width > 0 {
                        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
                    } else {
                        String::new()
                    };
                    
                    let text = if bar_width > 0 {
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
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, area);
        
        if total_items > visible_rows {
            let scroll_indicator = format!("{}/{}", scroll_offset + 1, total_items);
            let indicator = Paragraph::new(scroll_indicator)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default());
            let indicator_area = Rect {
                x: area.x + area.width - 1,
                y: area.y,
                width: 1,
                height: area.height,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_tag_stats(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect, 
        scroll_offset: usize,
        total_items: usize,
        visible_rows: usize,
    ) {
        let tags: Vec<(&String, &Stat)> = save_data.stats
            .iter()
            .filter(|(_, stat)| matches!(stat, Stat::Tag))
            .collect();
        
        if tags.is_empty() {
            let empty_text = "No tags";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
            frame.render_widget(paragraph, area);
            return;
        }
        
        let start = scroll_offset.min(tags.len().saturating_sub(1));
        let end = (start + visible_rows).min(tags.len());
        let visible_tags = &tags[start..end];
        
        let items: Vec<ListItem> = visible_tags.iter()
            .map(|(name, _)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("[{}]", name), Style::default().fg(Color::Cyan))
                ]))
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, area);
        
        if total_items > visible_rows {
            let scroll_indicator = format!("{}/{}", scroll_offset + 1, total_items);
            let indicator = Paragraph::new(scroll_indicator)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default());
            let indicator_area = Rect {
                x: area.x + area.width - 1,
                y: area.y,
                width: 1,
                height: area.height,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_inventory(
        save_data: &SaveData, 
        frame: &mut Frame, 
        area: Rect, 
        scroll_offset: usize,
        total_items: usize,
        visible_rows: usize,
    ) {
        let inventory: Vec<(&String, &u64)> = save_data.inventory
            .iter()
            .collect();
        
        if inventory.is_empty() {
            let empty_text = "Inventory empty";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
            frame.render_widget(paragraph, area);
            return;
        }
        
        let start = scroll_offset.min(inventory.len().saturating_sub(1));
        let end = (start + visible_rows).min(inventory.len());
        let visible_items = &inventory[start..end];
        
        let items: Vec<ListItem> = visible_items.iter()
            .map(|(name, count)| {
                let text = if **count > 1 {
                    format!("{} x{}", name, count)
                } else {
                    name.to_string()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(text, Style::default().fg(Color::Yellow))
                ]))
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(list, area);
        
        if total_items > visible_rows {
            let scroll_indicator = format!("{}/{}", scroll_offset + 1, total_items);
            let indicator = Paragraph::new(scroll_indicator)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default());
            let indicator_area = Rect {
                x: area.x + area.width - 1,
                y: area.y,
                width: 1,
                height: area.height,
            };
            frame.render_widget(indicator, indicator_area);
        }
    }

    fn render_dialogue_history(save_data: &SaveData, frame: &mut Frame, area: Rect) {
        let history = &save_data.history;
        let max_lines = (area.height as usize).saturating_sub(2);
        let start = if history.len() > max_lines { history.len() - max_lines } else { 0 };
        let recent: Vec<&Dialogue> = history.iter().skip(start).collect();
        
        if recent.is_empty() {
            let empty_text = "No dialogue history";
            let paragraph = Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::TOP | Borders::LEFT | Borders::RIGHT));
            frame.render_widget(paragraph, area);
            return;
        }
        
        let lines: Vec<Line> = recent.iter()
            .map(|dialogue| {
                let timestamp = chrono::DateTime::from_timestamp_millis(dialogue.timestamp)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| dialogue.timestamp.to_string());
                
                let (speaker_style, speaker_prefix) = match dialogue.speaker.as_str() {
                    "Narrator" => (Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD), "Narrator".to_string()),
                    "Player" => (Style::default().fg(Color::Green).add_modifier(Modifier::BOLD), "Player".to_string()),
                    "System" => (Style::default().fg(Color::Red), "System".to_string()),
                    _ => (Style::default().fg(Color::Cyan), dialogue.speaker.clone()),
                };
                
                let mut spans = vec![
                    Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}:", speaker_prefix), speaker_style),
                ];
                
                if let Some(content) = &dialogue.content {
                    spans.push(Span::from(" "));
                    spans.push(Span::from(content.clone()));
                }
                
                if let Some(actions) = &dialogue.actions {
                    let action_names: Vec<String> = actions.iter()
                        .filter_map(|v| {
                            v.as_object().and_then(|obj| {
                                obj.get("tool").and_then(|t| t.as_str()).map(|s| s.to_string())
                            })
                        })
                        .collect();
                    
                    if !action_names.is_empty() {
                        let action_str = format!(" [{}]", action_names.join(", "));
                        spans.push(Span::styled(action_str, Style::default().fg(Color::DarkGray)));
                    }
                }
                
                Line::from(spans)
            })
            .collect();
        
        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::TOP | Borders::LEFT | Borders::RIGHT));
        
        frame.render_widget(paragraph, area);
    }

    fn render_input_area(frame: &mut Frame, area: Rect, gameplay_state: &GameplayState) {
        let prefix = "> ";
        let text = if gameplay_state.is_editing {
            format!("{}{}", prefix, gameplay_state.input)
        } else {
            format!("{}[Press Enter to input]", prefix)
        };
        
        let style = if gameplay_state.is_editing {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        
        let paragraph = Paragraph::new(Line::from(vec![
            Span::styled(text, style)
        ]))
        .block(Block::default().borders(Borders::TOP | Borders::LEFT | Borders::RIGHT));
        
        frame.render_widget(paragraph, area);
    }

    fn render_help_bar(frame: &mut Frame, area: Rect) {
        let help_text = "Enter:Input | ↑↓:Scroll | Tab:Switch Column | /help:Commands | Esc:Cancel | Ctrl+Q:Quit";
        let paragraph = Paragraph::new(Line::from(vec![
            Span::styled(help_text, Style::default().fg(Color::DarkGray))
        ]))
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM));
        
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
                if state.is_editing {
                    self.handle_editing_input(event, narrator).await;
                } else {
                    self.handle_navigation_input(event);
                }
            }
            _ => {}
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
                        state.is_editing = false;
                        let input = state.input.clone();
                        state.input.clear();
                        (input, true)
                    }
                    KeyCode::Esc => {
                        state.is_editing = false;
                        state.input.clear();
                        (String::new(), false)
                    }
                    KeyCode::Backspace => {
                        state.input.pop();
                        (String::new(), false)
                    }
                    KeyCode::Char(c) => {
                        if c.is_ascii_graphic() || c.is_whitespace() {
                            state.input.push(c);
                        }
                        (String::new(), false)
                    }
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
            }
            "/save" => {
                self.save_game().await;
            }
            "/load" => {
                self.load_game().await;
            }
            "/quit" => {
                self.route = Route::MainMenu;
            }
            _ => {
                self.add_dialogue(
                    "System",
                    format!("Unknown command: {}", command),
                );
            }
        }
    }

    async fn handle_player_input(
        &mut self,
        input: &str,
        narrator: &Narrator,
    ) {
        // 添加玩家消息到历史
        self.add_dialogue("Player", input.to_string());
        
        // 获取上下文
        let context = self.build_game_context();
        
        // 调用 LLM
        match narrator.chat(&format!("{}\nPlayer action: {}", context, input)).await {
            Ok(response) => {
                // 解析并应用 LLM 响应
                self.apply_llm_response(&response).await;
            }
            Err(e) => {
                self.add_dialogue(
                    "System",
                    format!("Error: {}", e),
                );
            }
        }
    }

    fn add_dialogue(&mut self, speaker: &str, content: String) {
        if let Route::Gameplay(state) = &mut self.route {
            if let Some(save_data) = &mut state.selected_save_data {
                save_data.history.push(Dialogue::new(
                    speaker.to_string(),
                    Some(content),
                    None,
                ));
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

    fn build_game_context(&self) -> String {
        let mut context = String::new();
        
        if let Route::Gameplay(state) = &self.route {
            if let Some(save_data) = &state.selected_save_data {
                context.push_str("Game context:\n");
                context.push_str(&format!("- Stats: {:?}\n", save_data.stats));
                context.push_str(&format!("- Inventory: {:?}\n", save_data.inventory));
                context.push_str(&format!("- History: {} entries\n", save_data.history.len()));
                
                if let Some(last) = save_data.history.last() {
                    context.push_str(&format!("- Last action: {:?}\n", last));
                }
            }
        }
        
        context
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
                if let Some(save_data) = &state.selected_save_data {
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
                }
                KeyCode::Up => {
                    if state.scroll_offset > 0 {
                        state.scroll_offset -= 1;
                    }
                }
                KeyCode::Down => {
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
                    };
                    
                    let visible_rows = 10;
                    let max_scroll = max_items.saturating_sub(visible_rows);
                    if state.scroll_offset < max_scroll {
                        state.scroll_offset += 1;
                    }
                }
                KeyCode::Tab => {
                    state.selected_column = match state.selected_column {
                        ColumnType::Stats => ColumnType::Tags,
                        ColumnType::Tags => ColumnType::Inventory,
                        ColumnType::Inventory => ColumnType::Stats,
                    };
                    state.scroll_offset = 0;
                }
                KeyCode::Char('q') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.route = Route::MainMenu;
                }
                _ => {}
            }
        }
        
    }
}