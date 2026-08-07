use std::sync::Arc;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, ListState},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use ratatui_textarea::TextArea;

use crate::{actor::Stat, llm::Narrator, save::SaveData};
use crate::story::Dialogue;
use crate::llm::tool::builtin_tools::save_data;

use super::{App, AppEvent, Route};

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
    pub input: TextArea<'static>,
    pub is_editing: bool,
    pub selected_column: ColumnType,
    pub is_processing: bool,
    pub is_loading: bool,
    
    // 列表状态管理（自动处理高亮与滚动）
    pub stats_state: ListState,
    pub tags_state: ListState,
    pub inventory_state: ListState,
    pub dialogue_scroll_offset: usize,
    pub list_scroll_offset: usize,
    pub last_rendered_dialogue_count: Option<usize>,
}

impl Default for GameplayState {
    fn default() -> Self {
        let mut ta = TextArea::default();
        ta.set_cursor_line_style(Style::default());
        
        Self {
            selected_save_data: None,
            input: ta,
            is_editing: false,
            selected_column: ColumnType::Stats,
            is_processing: false,
            is_loading: false,
            stats_state: ListState::default(),
            tags_state: ListState::default(),
            inventory_state: ListState::default(),
            dialogue_scroll_offset: usize::MAX, 
            list_scroll_offset: 0,
            last_rendered_dialogue_count: None,
        }
    }
}

impl GameplayState {
    pub fn new(save_data: SaveData) -> Self {
        let dialogue_count = save_data.history.len();
        let mut ta = TextArea::default();
        ta.set_cursor_line_style(Style::default());
        
        Self {
            selected_save_data: Some(save_data),
            input: ta,
            is_editing: false,
            selected_column: ColumnType::Stats,
            is_processing: false,
            is_loading: false,
            stats_state: ListState::default(),
            tags_state: ListState::default(),
            inventory_state: ListState::default(),
            dialogue_scroll_offset: usize::MAX, 
            list_scroll_offset: 0,
            last_rendered_dialogue_count: Some(dialogue_count),
        }
    }
}

// 辅助函数：手动按宽度切割文本，避免依赖外部 crate
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    let max_width = max_width.max(1); 
    
    for c in text.chars() {
        let c_width = if c.is_ascii() { 1 } else { 2 };
        if current_width + c_width > max_width {
            lines.push(current_line);
            current_line = String::new();
            current_width = 0;
        }
        current_line.push(c);
        current_width += c_width;
    }
    if !current_line.is_empty() { lines.push(current_line); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

// 🌟 辅助函数：处理列表项文本的水平滚动与截断
fn process_list_item_text(full_text: &str, max_width: usize, is_highlighted: bool, scroll_offset: &mut usize) -> String {
    let char_count = full_text.chars().count();
    if char_count <= max_width {
        if is_highlighted {
            *scroll_offset = 0; // 文本够短，无需滚动，重置偏移量
        }
        return full_text.to_string();
    }
    
    if is_highlighted {
        let max_scroll = char_count.saturating_sub(max_width);
        if *scroll_offset > max_scroll {
            *scroll_offset = max_scroll; // 🌟 限制最大滚动量，防止无效按键堆积
        }
        full_text.chars().skip(*scroll_offset).take(max_width).collect()
    } else {
        // 未选中的项，始终从行首开始截断（保持行首对齐）
        full_text.chars().take(max_width).collect()
    }
}

impl App {
    pub fn render_gameplay(&mut self, frame: &mut Frame, area: Rect) {
        // 0. 渲染前同步全局数据到局部 UI 状态
        if let Route::Gameplay(state) = &mut self.route {
            let data = save_data();
            if let Ok(guard) = data.lock() {
                // 1. 🌟 零拷贝获取当前对白数量（直接读引用，极快）
                let new_count = guard.history.len();
                
                // 2. 同步完整状态到 UI (这里必须 clone，因为要脱离 Mutex 锁的生命周期)
                state.selected_save_data = Some(guard.clone());

                // 3. 🌟 优雅的对比逻辑
                let should_scroll = match state.last_rendered_dialogue_count {
                    Some(old_count) => new_count > old_count,
                    None => true, // 首次渲染，强制滚到底
                };

                if should_scroll {
                    state.dialogue_scroll_offset = usize::MAX;
                }
                
                // 4. 记录当前的数量，供下一帧对比
                state.last_rendered_dialogue_count = Some(new_count);
            }
        }

        // 1. 提取并接管状态
        let (mut save_data_opt, mut stats_state, mut tags_state, mut inv_state, mut dialogue_scroll, mut list_scroll_offset, selected_col) = match &mut self.route {
            Route::Gameplay(s) => (
                s.selected_save_data.take(),
                std::mem::take(&mut s.stats_state),
                std::mem::take(&mut s.tags_state),
                std::mem::take(&mut s.inventory_state),
                s.dialogue_scroll_offset,
                s.list_scroll_offset,
                s.selected_column
            ),
            _ => return,
        };

        let save_data = match &save_data_opt {
            Some(d) => d,
            None => {
                let error_text = "No save data loaded. Please load a save file.";
                let paragraph = Paragraph::new(error_text)
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(paragraph, area);
                if let Route::Gameplay(s) = &mut self.route { s.selected_save_data = save_data_opt; }
                return;
            }
        };

        // 2. 主布局
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .title(" Gameplay ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        
        let inner_area = outer_block.inner(area);
        frame.render_widget(outer_block, area);
        
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(75), Constraint::Length(3), Constraint::Length(1)])
            .split(inner_area);
        
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_chunks[0]);
        
        let left_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)])
            .split(top_chunks[0]);
        
        // 3. 渲染各组件
        Self::render_numeric_stats(save_data, frame, left_chunks[0], &mut stats_state, selected_col == ColumnType::Stats, &mut list_scroll_offset);
        Self::render_tag_stats(save_data, frame, left_chunks[1], &mut tags_state, selected_col == ColumnType::Tags, &mut list_scroll_offset);
        Self::render_inventory(save_data, frame, left_chunks[2], &mut inv_state, selected_col == ColumnType::Inventory, &mut list_scroll_offset);
        Self::render_dialogue_history(save_data, frame, top_chunks[1], &mut dialogue_scroll, selected_col == ColumnType::Dialogue);
        
        // 4. 恢复状态并渲染底部
        if let Route::Gameplay(s) = &mut self.route {
            s.selected_save_data = save_data_opt;
            s.stats_state = stats_state;
            s.tags_state = tags_state;
            s.inventory_state = inv_state;
            s.dialogue_scroll_offset = dialogue_scroll;
            s.list_scroll_offset = list_scroll_offset;
            Self::render_input_area(frame, main_chunks[1], s); // 🌟 s 已经是可变引用
        }
        Self::render_help_bar(frame, main_chunks[2]);
    }

    fn render_numeric_stats(
        save_data: &SaveData, frame: &mut Frame, area: Rect, list_state: &mut ListState, is_selected: bool, scroll_offset: &mut usize
    ) {
        let stats: Vec<(&String, &Stat)> = save_data.stats.iter().filter(|(_, stat)| matches!(stat, Stat::Numeric(_))).collect();
        let border_style = if is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
        
        let block = Block::default().borders(Borders::ALL).border_style(border_style).title(" Stats ").title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let content_area = block.inner(area);
        frame.render_widget(block, area);
        
        if stats.is_empty() {
            frame.render_widget(Paragraph::new("No numeric stats").style(Style::default().fg(Color::DarkGray)), content_area);
            return;
        }
        
        let max_width = content_area.width.saturating_sub(1) as usize;
        let items: Vec<ListItem> = stats.iter().enumerate().map(|(index, (name, stat))| {
            if let Stat::Numeric(lim) = stat {
                let current = *lim.value(); let max = *lim.max();
                let percentage = if max > 0 { (current as f64 / max as f64 * 100.0) as u16 } else { 0 };
                let bar_width = content_area.width.saturating_sub(6).min(30) as usize;
                let filled = (bar_width as f64 * percentage as f64 / 100.0) as usize;
                let bar = if bar_width > 0 && percentage > 0 { format!("[{}{}]", "█".repeat(filled), "░".repeat(bar_width - filled)) } else { String::new() };
                let full_text = if !bar.is_empty() { format!("{}: {}/{} {}", name, current, max, bar) } else { format!("{}: {}/{}", name, current, max) };
                let color = if percentage > 70 { Color::Green } else if percentage > 40 { Color::Yellow } else { Color::Red };
                
                let is_highlighted = is_selected && list_state.selected() == Some(index);
                let display_text = process_list_item_text(&full_text, max_width, is_highlighted, scroll_offset);
                
                ListItem::new(Line::from(vec![Span::styled(display_text, Style::default().fg(color))]))
            } else { ListItem::new("") }
        }).collect();
        
        frame.render_stateful_widget(List::new(items).highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)), content_area, list_state);
    }

    fn render_tag_stats(
        save_data: &SaveData, frame: &mut Frame, area: Rect, list_state: &mut ListState, is_selected: bool, scroll_offset: &mut usize
    ) {
        let tags: Vec<(&String, &Stat)> = save_data.stats.iter().filter(|(_, stat)| matches!(stat, Stat::Tag)).collect();
        let border_style = if is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
        
        let block = Block::default().borders(Borders::ALL).border_style(border_style).title(" Tags ").title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let content_area = block.inner(area);
        frame.render_widget(block, area);
        
        if tags.is_empty() {
            frame.render_widget(Paragraph::new("No tags").style(Style::default().fg(Color::DarkGray)), content_area);
            return;
        }
        
        let max_width = content_area.width.saturating_sub(1) as usize;
        let items: Vec<ListItem> = tags.iter().enumerate().map(|(index, (name, _))| {
            let full_text = format!("• {}", name);
            let is_highlighted = is_selected && list_state.selected() == Some(index);
            let display_text = process_list_item_text(&full_text, max_width, is_highlighted, scroll_offset);
            ListItem::new(Line::from(vec![Span::styled(display_text, Style::default().fg(Color::Cyan))]))
        }).collect();
        
        frame.render_stateful_widget(List::new(items).highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)), content_area, list_state);
    }

    fn render_inventory(
        save_data: &SaveData, frame: &mut Frame, area: Rect, list_state: &mut ListState, is_selected: bool, scroll_offset: &mut usize
    ) {
        let inventory: Vec<(&String, &u64)> = save_data.inventory.iter().collect();
        let border_style = if is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
        
        let block = Block::default().borders(Borders::ALL).border_style(border_style).title(" Inventory ").title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let content_area = block.inner(area);
        frame.render_widget(block, area);
        
        if inventory.is_empty() {
            frame.render_widget(Paragraph::new("Inventory empty").style(Style::default().fg(Color::DarkGray)), content_area);
            return;
        }
        
        let max_width = content_area.width.saturating_sub(1) as usize;
        let items: Vec<ListItem> = inventory.iter().enumerate().map(|(index, (name, count))| {
            let full_text = if **count > 1 { format!("{} ×{}", name, count) } else { name.to_string() };
            let is_highlighted = is_selected && list_state.selected() == Some(index);
            let display_text = process_list_item_text(&full_text, max_width, is_highlighted, scroll_offset);
            ListItem::new(Line::from(vec![Span::styled(display_text, Style::default().fg(Color::Yellow))]))
        }).collect();
        
        frame.render_stateful_widget(List::new(items).highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)), content_area, list_state);
    }

    fn render_dialogue_history(
        save_data: &SaveData, frame: &mut Frame, area: Rect, scroll_offset: &mut usize, is_selected: bool
    ) {
        let history = &save_data.history;
        let border_style = if is_selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
        
        let block = Block::default().borders(Borders::ALL).border_style(border_style).title(" Dialogue History ").title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let content_area = block.inner(area);
        frame.render_widget(block, area);
        
        if history.is_empty() {
            frame.render_widget(Paragraph::new("No dialogue history").style(Style::default().fg(Color::DarkGray)), content_area);
            return;
        }
        
        let mut all_lines: Vec<Line> = Vec::new();
        let width = content_area.width.max(1) as usize;
        
        for dialogue in history {
            let timestamp = chrono::DateTime::from_timestamp_millis(dialogue.timestamp)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| dialogue.timestamp.to_string());
            
            let (speaker_style, speaker_prefix) = match dialogue.speaker.as_str() {
                "Narrator" => (Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD), "📖"),
                "Player" => (Style::default().fg(Color::Green).add_modifier(Modifier::BOLD), "👤"),
                "System" => (Style::default().fg(Color::Red), "⚙️"),
                _ => (Style::default().fg(Color::Cyan), "❓"),
            };
            
            let prefix_str = format!("[{}] {} {}: ", timestamp, speaker_prefix, dialogue.speaker);
            let content_str = dialogue.content.as_deref().unwrap_or("");

            all_lines.push(Line::from(vec![Span::styled(prefix_str, speaker_style)]));

            let content_lines = wrap_text(content_str, width);

            for line_str in content_lines {
                all_lines.push(Line::from(vec![Span::styled(
                    line_str,
                    Style::default().fg(Color::White),
                )]));
            }
        }
        
        let max_scroll = all_lines.len().saturating_sub(content_area.height as usize);
        if *scroll_offset > max_scroll { *scroll_offset = max_scroll; }
        
        let text = Text::from(all_lines);
        let paragraph = Paragraph::new(text).scroll((*scroll_offset as u16, 0));
        frame.render_widget(paragraph, content_area);
    }

    // 🌟 更新为接收 &mut GameplayState 并渲染 TextArea
    fn render_input_area(frame: &mut Frame, area: Rect, gameplay_state: &mut GameplayState) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Gray));
            
        if gameplay_state.is_editing && !gameplay_state.is_processing {
            gameplay_state.input.set_block(block);
            gameplay_state.input.set_style(Style::default().fg(Color::White));
            frame.render_widget(&gameplay_state.input, area);
        } else if gameplay_state.is_processing {
            let input_text = "> Processing...";
            let style = Style::default().fg(Color::Yellow);
            let paragraph = Paragraph::new(input_text).style(style).block(block);
            frame.render_widget(paragraph, area);
        } else {
            let input_text = "> [Press Enter to input]";
            let style = Style::default().fg(Color::DarkGray);
            let paragraph = Paragraph::new(input_text).style(style).block(block);
            frame.render_widget(paragraph, area);
        }
    }

    fn render_help_bar(frame: &mut Frame, area: Rect) {
        let help_text = "Enter:Send | Alt+Enter / Ctrl+O:Newline | Paste:MultiLine | ↑↓:Scroll | Tab:Switch | Esc:Back";
        let paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Gray)));
        frame.render_widget(paragraph, area);
    }

    pub async fn handle_gameplay_input(&mut self, event: KeyEvent, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        match &mut self.route {
            Route::Gameplay(state) => {
                match event.code {
                    KeyCode::Tab => {
                        state.is_editing = false; 
                        state.list_scroll_offset = 0;
                        state.selected_column = match state.selected_column {
                            ColumnType::Stats => ColumnType::Tags,
                            ColumnType::Tags => ColumnType::Inventory,
                            ColumnType::Inventory => ColumnType::Dialogue,
                            ColumnType::Dialogue => ColumnType::Stats,
                        };
                        return;
                    },
                    _ => {},
                }

                if state.is_editing && !state.is_processing {
                    self.handle_editing_input(event, narrator, tx).await;
                } else {
                    self.handle_navigation_input(event);
                }
            },
            _ => {},
        }
    }

    async fn handle_editing_input(&mut self, event: KeyEvent, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        let (input, should_process) = {
            if let Route::Gameplay(state) = &mut self.route {
                let is_submit = event.code == KeyCode::Enter && event.modifiers.is_empty();

                let is_newline_intent =
                    (event.code == KeyCode::Enter && event.modifiers.contains(KeyModifiers::ALT))
                    || (event.code == KeyCode::Char('o') && event.modifiers.contains(KeyModifiers::CONTROL));

                if is_submit {
                    let text = state.input.lines().join("\n");
                    if !text.trim().is_empty() && !state.is_processing {
                        state.is_editing = false;
                        let mut empty_ta = TextArea::default();
                        empty_ta.set_cursor_line_style(Style::default());
                        std::mem::swap(&mut state.input, &mut empty_ta);
                        (text, true)
                    } else {
                        (String::new(), false)
                    }
                } else if is_newline_intent {
                    state.input.insert_newline();
                    (String::new(), false)
                } else {
                    state.input.input(event);
                    (String::new(), false)
                }
            } else { (String::new(), false) }
        };

        if should_process && !input.is_empty() {
            if input.starts_with('/') {
                self.handle_command(&input).await;
            } else {
                self.handle_player_input(&input, narrator, tx).await;
            }
        }
    }

    async fn handle_command(&mut self, command: &str) {
        match command {
            "/help" => self.show_help_message(),
            "/save" => self.save_game().await,
            "/load" => self.load_game().await,
            "/quit" => self.route = Route::MainMenu,
            _ => self.add_dialogue("System", format!("Unknown command: {}", command)),
        }
    }
    
    async fn handle_player_input(&mut self, input: &str, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        let history_clone = {
            let data = save_data();
            let mut guard = data.lock().unwrap();
            let dialogue = Dialogue::new("Player".to_string(), Some(input.to_string()));
            guard.history.push(dialogue.clone());
            if let Route::Gameplay(state) = &mut self.route {
                if let Some(save_data) = &mut state.selected_save_data {
                    save_data.history.push(dialogue);
                }
            }
            guard.raw_history.clone()
        };

        if let Route::Gameplay(state) = &mut self.route {
            state.is_processing = true;
        }

        let narrator_clone = narrator.clone();
        let input_clone = input.to_string();

        tokio::spawn(async move {
            let mut history = history_clone;
            let result = narrator_clone.stream_narrate(&input_clone, &mut history).await;

            {
                let data = save_data();
                if let Ok(mut guard) = data.lock() {
                    guard.raw_history = history;
                }
            }

            match result {
                Ok(response) => {
                    let _ = tx.send(AppEvent::LlmResponse(response));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::LlmError(e.to_string()));
                }
            }
        });
    }

    pub fn add_dialogue(&mut self, speaker: &str, content: String) {
        if let Route::Gameplay(state) = &mut self.route {
            if let Some(save_data) = &mut state.selected_save_data {
                save_data.history.push(Dialogue::new(speaker.to_string(), Some(content)));
                state.dialogue_scroll_offset = usize::MAX; 
            }
        }
    }

    fn show_help_message(&mut self) {
        let help_text = vec![
            "Available commands:".to_string(), "  /help  - Show this help message".to_string(),
            "  /save  - Save current game".to_string(), "  /load  - Load saved game".to_string(),
            "  /quit  - Return to main menu".to_string(), "".to_string(),
            "Controls:".to_string(), "  Enter - Start typing".to_string(),
            "  ↑/↓   - Scroll".to_string(), "  ←/→   - View long text".to_string(),
            "  Tab   - Switch columns".to_string(),
            "  Esc   - Back to previous screen".to_string(),
        ].join("\n");
        self.add_dialogue("System", help_text);
    }

    async fn save_game(&mut self) {
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
        if let Ok(parsed) = serde_json::from_str::<Value>(response) {
            let mut stats_updates: Vec<(String, i64)> = Vec::new();
            let mut inventory_updates: Vec<(String, u64)> = Vec::new();
    
            if let Route::Gameplay(state) = &mut self.route {
                if let Some(save_data) = &mut state.selected_save_data {
                    for (key, value) in stats_updates {
                        if let Some(stat) = save_data.stats.get_mut(&key) {
                            if let Stat::Numeric(lim) = stat { let _ = lim.set_value(value); }
                        }
                    }
                    for (key, count) in inventory_updates {
                        if count > 0 { save_data.inventory.insert(key, count); } 
                        else { save_data.inventory.remove(&key); }
                    }
                }
            }
        } else {
            let dialogue = Dialogue::new("Narrator".to_string(), Some(response.to_string()));
            {
                let data = save_data();
                let mut guard = data.lock().unwrap();
                guard.history.push(dialogue.clone());
                if let Route::Gameplay(state) = &mut self.route {
                    if let Some(save_data) = &mut state.selected_save_data { save_data.history.push(dialogue); }
                }
            }
        }
    }

    pub async fn handle_llm_response(&mut self, response: &str) {
        self.apply_llm_response(response).await;
    }
    
    // 🌟 进入编辑模式时重置 TextArea 状态
    fn handle_navigation_input(&mut self, event: KeyEvent) {
        if let Route::Gameplay(state) = &mut self.route {
            match event.code {
                KeyCode::Enter => {
                    state.is_editing = true;
                    let mut ta = TextArea::default();
                    ta.set_cursor_line_style(Style::default());
                    state.input = ta;
                },
                KeyCode::Up => {
                    match state.selected_column {
                        ColumnType::Stats => {
                            let i = match state.stats_state.selected() { Some(i) => i.saturating_sub(1), None => 0 };
                            state.stats_state.select(Some(i));
                            state.list_scroll_offset = 0;
                        },
                        ColumnType::Tags => {
                            let i = match state.tags_state.selected() { Some(i) => i.saturating_sub(1), None => 0 };
                            state.tags_state.select(Some(i));
                            state.list_scroll_offset = 0;
                        },
                        ColumnType::Inventory => {
                            let i = match state.inventory_state.selected() { Some(i) => i.saturating_sub(1), None => 0 };
                            state.inventory_state.select(Some(i));
                            state.list_scroll_offset = 0;
                        },
                        ColumnType::Dialogue => {
                            if state.dialogue_scroll_offset > 0 { state.dialogue_scroll_offset -= 1; }
                        }
                    }
                },
                KeyCode::Down => {
                    match state.selected_column {
                        ColumnType::Stats => {
                            let i = match state.stats_state.selected() { Some(i) => i + 1, None => 0 };
                            state.stats_state.select(Some(i));
                            state.list_scroll_offset = 0;
                        },
                        ColumnType::Tags => {
                            let i = match state.tags_state.selected() { Some(i) => i + 1, None => 0 };
                            state.tags_state.select(Some(i));
                            state.list_scroll_offset = 0;
                        },
                        ColumnType::Inventory => {
                            let i = match state.inventory_state.selected() { Some(i) => i + 1, None => 0 };
                            state.inventory_state.select(Some(i));
                            state.list_scroll_offset = 0;
                        },
                        ColumnType::Dialogue => {
                            state.dialogue_scroll_offset = state.dialogue_scroll_offset.saturating_add(1);
                        }
                    }
                },
                KeyCode::Left => {
                    match state.selected_column {
                        ColumnType::Stats | ColumnType::Tags | ColumnType::Inventory => {
                            state.list_scroll_offset = state.list_scroll_offset.saturating_sub(1);
                        }
                        _ => {}
                    }
                },
                KeyCode::Right => {
                    match state.selected_column {
                        ColumnType::Stats | ColumnType::Tags | ColumnType::Inventory => {
                            state.list_scroll_offset = state.list_scroll_offset.saturating_add(1);
                        }
                        _ => {}
                    }
                },
                _ => {},
            }
        }
    }
}