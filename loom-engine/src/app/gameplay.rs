use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::{actor::Stat, save::SaveData};
use crate::story::Dialogue;

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
}

impl Default for GameplayState {
    fn default() -> Self {
        Self {
            selected_save_data: None,
            scroll_offset: 0,
            input: String::new(),
            is_editing: false,
            selected_column: ColumnType::Stats,
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
        
        // 计算可见行数
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
        
        // 简单的滚动指示器（避免使用不兼容的 Scrollbar）
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
        let prefix = if gameplay_state.is_editing { "> " } else { "> " };
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

    pub fn handle_gameplay_input(&mut self, event: Event) -> Option<String> {
        let gameplay_state = match &mut self.route {
            Route::Gameplay(state) => state,
            _ => return None,
        };

        if gameplay_state.is_editing {
            match event {
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Enter => {
                            gameplay_state.is_editing = false;
                            let input = gameplay_state.input.clone();
                            gameplay_state.input.clear();
                            if !input.is_empty() {
                                if input.starts_with('/') {
                                    return Some(input);
                                }
                                return Some(input);
                            }
                            return None;
                        }
                        KeyCode::Esc => {
                            gameplay_state.is_editing = false;
                            gameplay_state.input.clear();
                            return None;
                        }
                        KeyCode::Backspace => {
                            gameplay_state.input.pop();
                        }
                        KeyCode::Char(c) => {
                            if c.is_ascii_graphic() || c.is_whitespace() {
                                gameplay_state.input.push(c);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return None;
        }
        
        match event {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Enter => {
                        gameplay_state.is_editing = true;
                        gameplay_state.input.clear();
                        return None;
                    }
                    KeyCode::Up => {
                        if gameplay_state.scroll_offset > 0 {
                            gameplay_state.scroll_offset -= 1;
                        }
                        return None;
                    }
                    KeyCode::Down => {
                        let save_data = match &gameplay_state.selected_save_data {
                            Some(data) => data,
                            None => return None,
                        };
                        
                        let max_items = match gameplay_state.selected_column {
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
                        if gameplay_state.scroll_offset < max_scroll {
                            gameplay_state.scroll_offset += 1;
                        }
                        return None;
                    }
                    KeyCode::Tab => {
                        gameplay_state.selected_column = match gameplay_state.selected_column {
                            ColumnType::Stats => ColumnType::Tags,
                            ColumnType::Tags => ColumnType::Inventory,
                            ColumnType::Inventory => ColumnType::Stats,
                        };
                        gameplay_state.scroll_offset = 0;
                        return None;
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Some("quit".to_string());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        
        None
    }

    pub fn handle_llm_response(&mut self, response: &str) -> Result<(), Box<dyn std::error::Error>> {
        let gameplay_state = match &mut self.route {
            Route::Gameplay(state) => state,
            _ => return Err("Not in gameplay mode".into()),
        };
    
        let save_data = match &mut gameplay_state.selected_save_data {
            Some(data) => data,
            None => return Err("No save data loaded".into()),
        };
    
        use serde_json::Value;
        if let Ok(parsed) = serde_json::from_str::<Value>(response) {
            // 只处理对话内容
            if let Some(content) = parsed.get("content").and_then(|c| c.as_str()) {
                save_data.history.push(Dialogue::new(
                    "Narrator".to_string(),
                    Some(content.to_string()),
                    None,
                ));
            }
            // 其他字段（stats, inventory 等）由工具调用逻辑处理
        }
        
        Ok(())
    }
}