use std::sync::Arc;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use chrono::DateTime;

use super::{gameplay::GameplayState, App};
use crate::{app::Route, config::GameMode, llm::{tool::builtin_tools::save_data, Narrator}};

impl App {
    pub fn render_saves(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(60),
            ])
            .split(area);

        self.render_save_list(frame, chunks[0]);
        self.render_save_details(frame, chunks[1]);
    }

    fn render_save_list(&mut self, frame: &mut Frame, area: Rect) {
        let mut items: Vec<ListItem> = vec![];

        if let Some(save_metas) = &self.current_save_metas {
            for item in save_metas {
                match item {
                    Ok(meta) => {
                        let datetime = DateTime::from_timestamp_millis(meta.timestamp)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());
                        
                        let note = if meta.note.is_empty() {
                            "[No note]".to_string()
                        } else {
                            meta.note.clone()
                        };

                        items.push(
                            ListItem::new(vec![
                                Line::from(Span::styled(
                                    datetime,
                                    Style::default().fg(Color::White),
                                )),
                                Line::from(Span::styled(
                                    note,
                                    Style::default().fg(Color::DarkGray),
                                )),
                            ])
                        );
                    }
                    Err(error) => {
                        items.push(
                            ListItem::new(Text::from(format!("[ERROR] {}", error)))
                                .style(Style::default().fg(Color::Red)),
                        );
                    }
                }
            }
        }

        let save_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 💾 Saves ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            );

        if let Route::Saves(state) = &mut self.route {
            frame.render_stateful_widget(save_list, area, state);
        }
    }

    fn render_save_details(&mut self, frame: &mut Frame, area: Rect) {
        let details_block = Block::default()
            .borders(Borders::ALL)
            .title(" Save Details ")
            .border_style(Style::default().fg(Color::Cyan));

        let selected_index = if let Route::Saves(state) = &self.route {
            state.selected()
        } else {
            None
        };

        match selected_index.and_then(|i| {
            self.current_save_metas
                .as_ref()
                .and_then(|metas| metas.get(i))
        }) {
            Some(Ok(meta)) => {
                let datetime = DateTime::from_timestamp_millis(meta.timestamp)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let project_name = self
                    .projects
                    .get(self.selected_project_index)
                    .and_then(|p| p.as_ref().ok())
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let text = vec![
                    Line::from(vec![
                        Span::styled(
                            "Project: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::from(project_name),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Timestamp: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::from(datetime),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Note: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::from(if meta.note.is_empty() {
                            "[No note]"
                        } else {
                            &meta.note
                        }),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Filename: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::from(&meta.main_filename),
                    ]),
                ];

                let paragraph = Paragraph::new(text)
                    .block(details_block)
                    .wrap(Wrap { trim: true });
                frame.render_widget(paragraph, area);
            }
            Some(Err(error)) => {
                let text = vec![
                    Line::from(vec![Span::styled(
                        "[ERROR] Save Error",
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Details: ", Style::default().fg(Color::Cyan)),
                        Span::from(error.to_string()),
                    ]),
                ];

                let paragraph = Paragraph::new(text)
                    .block(details_block)
                    .style(Style::default().fg(Color::Red));
                frame.render_widget(paragraph, area);
            }
            None => {
                let help_text = vec![
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "No save selected",
                        Style::default().fg(Color::Gray),
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Enter: Load save",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "S: Overwrite save",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "Ctrl+S: Save as new",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "N: New save",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "E: Edit note",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "D: Delete save",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "R: Refresh",
                        Style::default().fg(Color::DarkGray),
                    )]),
                    Line::from(vec![Span::styled(
                        "Esc: Back to projects",
                        Style::default().fg(Color::DarkGray),
                    )]),
                ];

                let paragraph = Paragraph::new(help_text)
                    .block(details_block)
                    .alignment(Alignment::Center);
                frame.render_widget(paragraph, area);
            }
        }
    }

    pub async fn handle_saves_input(&mut self, key: KeyEvent, narrator: Arc<Narrator>) {
        match key.code {
            KeyCode::Up => {
                if let Route::Saves(state) = &mut self.route {
                    let total = self
                        .current_save_metas
                        .as_ref()
                        .map(|m| m.len())
                        .unwrap_or(0);
                    if total > 0 {
                        let i = match state.selected() {
                            Some(i) if i > 0 => i - 1,
                            Some(_) => total - 1,
                            None => 0,
                        };
                        state.select(Some(i));
                        
                        if let Some(metas) = &self.current_save_metas {
                            if let Some(Ok(meta)) = metas.get(i) {
                                let _ = self.select_save(meta.timestamp);
                            }
                        }
                    }
                }
            }
            KeyCode::Down => {
                if let Route::Saves(state) = &mut self.route {
                    let total = self
                        .current_save_metas
                        .as_ref()
                        .map(|m| m.len())
                        .unwrap_or(0);
                    if total > 0 {
                        let i = match state.selected() {
                            Some(i) if i + 1 < total => i + 1,
                            Some(_) => 0,
                            None => 0,
                        };
                        state.select(Some(i));
                        
                        if let Some(metas) = &self.current_save_metas {
                            if let Some(Ok(meta)) = metas.get(i) {
                                let _ = self.select_save(meta.timestamp);
                            }
                        }
                    }
                }
            }
            KeyCode::Enter => {
                // 先克隆需要的数据，避免借用冲突
                let timestamp = if let Route::Saves(state) = &self.route {
                    state.selected().and_then(|i| {
                        self.current_save_metas
                            .as_ref()
                            .and_then(|metas| metas.get(i))
                            .and_then(|r| r.as_ref().ok())
                            .map(|meta| meta.timestamp)
                    })
                } else {
                    None
                };
    
                if let Some(ts) = timestamp {
                    if let Ok(project) = self.get_mut_project() {
                        let project_prompt = project.config().prompt.clone();
                        let game_mode = project.config().mode.clone();
                        // 1. 构建模式规则
                        let mode_directive = match game_mode {
                            GameMode::Normal => "The currently active Game Mode is: NORMAL. You MUST execute [PROTOCOL: NORMAL] immediately.",
                            GameMode::Author => "The currently active Game Mode is: AUTHOR. You MUST execute [PROTOCOL: AUTHOR] immediately.",
                        };
                
                        // 2. 组合世界观与模式指令
                        let dynamic_rule = format!(
                            "[WORLD SETTING - ALWAYS ACTIVE]\n{}\n\n[GAME MODE DIRECTIVE - HIGHEST PRIORITY]\n{}", 
                            project_prompt, 
                            mode_directive
                        );
                
                        // 3. 更新 preamble
                        if let Err(e) = narrator.update_preamble(Some(&dynamic_rule)).await {
                            tracing::error!("Failed to update system prompt: {}", e);
                        }

                        if let Ok(loaded_save) = project.load_save(ts) {
                            {
                                let data = save_data();
                                let mut guard = data.lock().unwrap();
                                *guard = loaded_save.clone();
                            }
                        
                            self.navigate_to(Route::Gameplay(
                                GameplayState::new(loaded_save),
                            ));
                        }
                    }
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+S: 创建新存档（基于当前数据）
                    if let Ok(project) = self.get_mut_project() {
                        todo!("实现输入备注的 UI");
                        // let _ = project.save(false, None, None, None);
                        // let _ = self.refresh_saves();
                    }
                } else {
                    // S: 覆盖当前选中的存档
                    // 先获取时间戳
                    let timestamp = if let Route::Saves(state) = &self.route {
                        state.selected().and_then(|i| {
                            self.current_save_metas
                                .as_ref()
                                .and_then(|metas| metas.get(i))
                                .and_then(|r| r.as_ref().ok())
                                .map(|meta| meta.timestamp)
                        })
                    } else {
                        None
                    };
    
                    if let Some(ts) = timestamp {
                        if let Ok(project) = self.get_mut_project() {
                            // 需要通过时间戳找到对应的 meta
                            if let Some(metas) = &mut self.current_save_metas {
                                if let Some(Ok(meta)) = metas.iter_mut().find(|r| {
                                    r.as_ref().map_or(false, |m| m.timestamp == ts)
                                }) {
                                    todo!("实现输入备注的 UI");
                                    // let _ = project.save(true, Some(meta), None, None);
                                    // let _ = self.refresh_saves();
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // N: 创建全新存档
                if let Ok(project) = self.get_mut_project() {
                    todo!("实现输入备注的 UI");
                    // let _ = project.save(false, None, None, None);
                    // let _ = self.refresh_saves();
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                // E: 编辑备注
                let timestamp = if let Route::Saves(state) = &self.route {
                    state.selected().and_then(|i| {
                        self.current_save_metas
                            .as_ref()
                            .and_then(|metas| metas.get(i))
                            .and_then(|r| r.as_ref().ok())
                            .map(|meta| meta.timestamp)
                    })
                } else {
                    None
                };
    
                if let Some(ts) = timestamp {
                    if let Ok(project) = self.get_mut_project() {
                        if let Some(metas) = &mut self.current_save_metas {
                            if let Some(Ok(meta)) = metas.iter_mut().find(|r| {
                                r.as_ref().map_or(false, |m| m.timestamp == ts)
                            }) {
                                todo!("实现输入备注的 UI，获取用户输入的新备注");
                                // let new_note = "用户输入的新备注".to_string();
                                // let _ = project.update_save_note(meta, new_note);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                // D: 删除存档
                let timestamp = if let Route::Saves(state) = &self.route {
                    state.selected().and_then(|i| {
                        self.current_save_metas
                            .as_ref()
                            .and_then(|metas| metas.get(i))
                            .and_then(|r| r.as_ref().ok())
                            .map(|meta| meta.timestamp)
                    })
                } else {
                    None
                };
    
                if let Some(ts) = timestamp {
                    if let Ok(project) = self.get_mut_project() {
                        let _ = project.delete_save(ts);
                        let _ = self.refresh_saves();
                    }
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // R: 刷新
                let _ = self.refresh_saves();
            }
            KeyCode::Esc => {
                // 返回项目列表
                let selected_project_index = self.selected_project_index;
                self.navigate_to(Route::Projects(ListState::default()));
                self.selected_save_meta_id = None;
                
                if let Route::Projects(state) = &mut self.route {
                    state.select(Some(selected_project_index));
                }
            }
            _ => {}
        }
    }

    pub fn refresh_saves(&mut self) -> Result<(), anyhow::Error> {
        if let Some(project_id) = self.selected_project_id {
            for item in &self.projects {
                if let Ok(project) = item && project.timestamp() == project_id {
                    self.current_save_metas = Some(project.list_saves()?);
                    
                    if let Route::Saves(state) = &mut self.route {
                        if let Some(metas) = &self.current_save_metas {
                            if !metas.is_empty() {
                                let mut target_idx = 0;
                                let mut found_previous = false;

                                // 1. 尝试恢复之前选中的存档 (selected_save_meta_id)
                                if let Some(selected_id) = self.selected_save_meta_id {
                                    for (idx, meta_result) in metas.iter().enumerate() {
                                        if let Ok(meta) = meta_result {
                                            if meta.timestamp == selected_id {
                                                target_idx = idx;
                                                found_previous = true;
                                                break;
                                            }
                                        }
                                    }
                                    // 如果之前选中的存档已经被删除了，清空记录
                                    if !found_previous {
                                        self.selected_save_meta_id = None;
                                    }
                                }

                                // 2. 执行选中操作
                                // 这里去掉了原先 `state.selected().is_none()` 的限制。
                                // 这样无论是首次进入，还是复用旧的 ListState，都能强制刷新并同步内部状态。
                                state.select(Some(target_idx));
                                if let Some(Ok(meta)) = metas.get(target_idx) {
                                    let _ = self.select_save(meta.timestamp);
                                }
                            } else {
                                // 列表为空时清空选中状态
                                state.select(None);
                                self.selected_save_meta_id = None;
                            }
                        }
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}