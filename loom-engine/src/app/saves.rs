use std::sync::Arc;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use chrono::DateTime;
use ratatui_textarea::{TextArea, CursorMove};
use tokio::sync::mpsc::UnboundedSender;
use super::{gameplay::GameplayState, App};
use crate::{app::{Route, AppEvent}, config::GameMode, llm::{tool::builtin_tools::save_data, Narrator}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteEditAction {
    SaveAs(i64),
    NewSave,
    EditNote(i64),
}

#[derive(Debug)]
pub struct SavesState {
    list_state: ListState,
    is_editing: bool,
    note: TextArea<'static>,
    editing_action: Option<NoteEditAction>,
    pub is_processing: bool, // 🌟 新增：是否正在执行耗时操作
}

impl Default for SavesState {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
            is_editing: false,
            note: TextArea::default(),
            editing_action: None,
            is_processing: false,
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

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
        self.render_popup(frame);
        self.render_processing_overlay(frame); // 🌟 新增：渲染加载提示
    }

    // 🌟 新增：渲染"正在处理"覆盖层
    fn render_processing_overlay(&mut self, frame: &mut Frame) {
        let is_processing = if let Route::Saves(state) = &self.route {
            state.is_processing
        } else {
            false
        };

        if is_processing {
            let area = centered_rect(60, 30, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Please Wait ")
                .border_style(Style::default().fg(Color::Yellow));

            let text = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "⏳ CREATING & INITIALIZING WORLD...",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "This may take a moment.",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(block)
            .alignment(Alignment::Center);

            frame.render_widget(text, area);
        }
    }

    fn render_popup(&mut self, frame: &mut Frame) {
        if let Route::Saves(SavesState { list_state: _, is_editing, note, editing_action: _, is_processing: _ }) = &mut self.route {
            if *is_editing {
                let popup_area = centered_rect(60, 40, frame.area());
                frame.render_widget(Clear, popup_area);
                note.set_block(
                    Block::default()
                        .borders(Borders::all())
                        .title(" Note Editor ")
                        .title_alignment(Alignment::Center),
                );
                frame.render_widget(&*note, popup_area);
            }
        }
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
            frame.render_stateful_widget(save_list, area, &mut state.list_state);
        }
    }

    fn render_save_details(&mut self, frame: &mut Frame, area: Rect) {
        let details_block = Block::default()
            .borders(Borders::ALL)
            .title(" Save Details ")
            .border_style(Style::default().fg(Color::Cyan));

        let selected_index = if let Route::Saves(state) = &self.route {
            state.list_state.selected()
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
                        Span::styled("Project: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(project_name),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Timestamp: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(datetime),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Note: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(if meta.note.is_empty() { "[No note]" } else { &meta.note }),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Filename: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
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
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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
                    Line::from(vec![Span::styled("No save selected", Style::default().fg(Color::Gray))]),
                    Line::from(""),
                    Line::from(vec![Span::styled("Enter: Load save", Style::default().fg(Color::DarkGray))]),
                    Line::from(vec![Span::styled("Ctrl+S: Save as new", Style::default().fg(Color::DarkGray))]),
                    Line::from(vec![Span::styled("N: New save", Style::default().fg(Color::DarkGray))]),
                    Line::from(vec![Span::styled("E: Edit note", Style::default().fg(Color::DarkGray))]),
                    Line::from(vec![Span::styled("D: Delete save", Style::default().fg(Color::DarkGray))]),
                    Line::from(vec![Span::styled("R: Refresh", Style::default().fg(Color::DarkGray))]),
                    Line::from(vec![Span::styled("Esc: Back to MainMenu", Style::default().fg(Color::DarkGray))]),
                ];

                let paragraph = Paragraph::new(help_text)
                    .block(details_block)
                    .alignment(Alignment::Center);
                frame.render_widget(paragraph, area);
            }
        }
    }

    fn open_note_editor(&mut self, action: NoteEditAction, initial_text: &str) {
        if let Route::Saves(state) = &mut self.route {
            state.is_editing = true;
            state.editing_action = Some(action);
            
            let lines: Vec<String> = if initial_text.is_empty() {
                vec![String::new()]
            } else {
                initial_text.lines().map(|s| s.to_string()).collect()
            };
            
            let mut ta = TextArea::new(lines);
            ta.set_cursor_line_style(Style::default());
            ta.move_cursor(CursorMove::Bottom);
            ta.move_cursor(CursorMove::End);
            
            std::mem::swap(&mut state.note, &mut ta);
        }
    }

    // 🌟 修改：增加 tx 参数
    async fn handle_note_edit_input(&mut self, key: KeyEvent, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        let is_submit = key.code == KeyCode::Enter && key.modifiers.is_empty();
        let is_newline = (key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::ALT))
            || (key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL));
        let is_cancel = key.code == KeyCode::Esc;

        if is_submit {
            let (text, action) = {
                if let Route::Saves(state) = &mut self.route {
                    let text = state.note.lines().join("\n");
                    let action = state.editing_action.clone();
                    state.is_editing = false;
                    state.editing_action = None;
                    
                    let mut empty_ta = TextArea::default();
                    empty_ta.set_cursor_line_style(Style::default());
                    std::mem::swap(&mut state.note, &mut empty_ta);
                    (text, action)
                } else {
                    (String::new(), None)
                }
            };

            if let Some(action) = action {
                self.execute_save_action(action, text, narrator, tx).await;
            }
        } else if is_newline {
            if let Route::Saves(state) = &mut self.route {
                state.note.insert_newline();
            }
        } else if is_cancel {
            if let Route::Saves(state) = &mut self.route {
                state.is_editing = false;
                state.editing_action = None;
                let mut empty_ta = TextArea::default();
                empty_ta.set_cursor_line_style(Style::default());
                std::mem::swap(&mut state.note, &mut empty_ta);
            }
        } else {
            if let Route::Saves(state) = &mut self.route {
                state.note.input(key);
            }
        }
    }

    // 🌟 修改：增加 tx 参数，耗时操作使用 tokio::spawn
    async fn execute_save_action(&mut self, action: NoteEditAction, note: String, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        let project_id = self.selected_project_id;
        
        // 克隆 Project，以便在 tokio::spawn 中使用
        let project_clone = self.projects.iter().find_map(|item| {
            if let Ok(proj) = item {
                if Some(proj.timestamp()) == project_id {
                    return Some(proj.clone());
                }
            }
            None
        });

        let Some(project) = project_clone else { return };

        match action {
            NoteEditAction::NewSave => {
                // 🌟 设置加载状态
                if let Route::Saves(state) = &mut self.route {
                    state.is_processing = true;
                }

                let game_mode = project.config().mode.clone();
                let prompt = project.config().prompt.clone();
                
                // 🌟 后台异步执行，不阻塞 UI
                tokio::spawn(async move {
                    let result = project.save(false, None, Some(note), Some((&*narrator, game_mode, &prompt))).await;
                    
                    if let Err(e) = result {
                        tracing::error!("Failed to create new save: {}", e);
                    }
                    
                    // 通知主循环操作完成
                    let _ = tx.send(AppEvent::SaveOperationCompleted);
                });
            }
            NoteEditAction::SaveAs(ts) => {
                // 克隆 SaveMeta
                let meta_clone = self.current_save_metas.as_ref().and_then(|metas| {
                    metas.iter()
                        .find_map(|r| r.as_ref().ok())
                        .filter(|m| m.timestamp == ts)
                        .cloned()
                });

                if let Some(mut meta) = meta_clone {
                    if let Route::Saves(state) = &mut self.route {
                        state.is_processing = true;
                    }

                    tokio::spawn(async move {
                        let result = project.save(false, Some(&mut meta), Some(note), None).await;
                        
                        if let Err(e) = result {
                            tracing::error!("Failed to save as: {}", e);
                        }
                        
                        let _ = tx.send(AppEvent::SaveOperationCompleted);
                    });
                }
            }
            NoteEditAction::EditNote(ts) => {
                // 编辑备注是本地文件操作，很快，同步执行即可
                if let Some(metas) = &mut self.current_save_metas {
                    if let Some(Ok(meta)) = metas.iter_mut().find(|r| r.as_ref().map_or(false, |m| m.timestamp == ts)) {
                        let _ = project.update_save_note(meta, note);
                    }
                }
                let _ = self.refresh_saves();
            }
        }
    }

    // 🌟 新增：异步操作完成后的回调
    pub fn on_save_operation_completed(&mut self) {
        if let Route::Saves(state) = &mut self.route {
            state.is_processing = false;
        }
        let _ = self.refresh_saves();
    }

    // 🌟 修改：增加 tx 参数
    pub async fn handle_saves_input(&mut self, key: KeyEvent, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        // 如果正在处理，屏蔽所有输入
        let is_processing = if let Route::Saves(state) = &self.route {
            state.is_processing
        } else {
            false
        };
        if is_processing {
            return;
        }

        let is_editing = if let Route::Saves(state) = &self.route {
            state.is_editing
        } else {
            false
        };

        if is_editing {
            self.handle_note_edit_input(key, narrator, tx).await;
            return;
        }

        match key.code {
            KeyCode::Up => {
                if let Route::Saves(state) = &mut self.route {
                    let total = self.current_save_metas.as_ref().map(|m| m.len()).unwrap_or(0);
                    if total > 0 {
                        let i = match state.list_state.selected() {
                            Some(i) if i > 0 => i - 1,
                            Some(_) => total - 1,
                            None => 0,
                        };
                        state.list_state.select(Some(i));
                        
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
                    let total = self.current_save_metas.as_ref().map(|m| m.len()).unwrap_or(0);
                    if total > 0 {
                        let i = match state.list_state.selected() {
                            Some(i) if i + 1 < total => i + 1,
                            Some(_) => 0,
                            None => 0,
                        };
                        state.list_state.select(Some(i));
                        
                        if let Some(metas) = &self.current_save_metas {
                            if let Some(Ok(meta)) = metas.get(i) {
                                let _ = self.select_save(meta.timestamp);
                            }
                        }
                    }
                }
            }
            KeyCode::Enter => {
                let timestamp = if let Route::Saves(state) = &self.route {
                    state.list_state.selected().and_then(|i| {
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
                        let mode_directive = match game_mode {
                            GameMode::Normal => "The currently active Game Mode is: NORMAL. You MUST execute [PROTOCOL: NORMAL] immediately.",
                            GameMode::Author => "The currently active Game Mode is: AUTHOR. You MUST execute [PROTOCOL: AUTHOR] immediately.",
                        };
                
                        let dynamic_rule = format!(
                            "[WORLD SETTING - ALWAYS ACTIVE]\n{}\n\n[GAME MODE DIRECTIVE - HIGHEST PRIORITY]\n{}", 
                            project_prompt, 
                            mode_directive
                        );
                
                        if let Err(e) = narrator.update_preamble(Some(&dynamic_rule)).await {
                            tracing::error!("Failed to update system prompt: {}", e);
                        }

                        if let Ok(loaded_save) = project.load_save(ts) {
                            {
                                let data = save_data();
                                let mut guard = data.lock().unwrap();
                                *guard = loaded_save.clone();
                            }
                        
                            self.navigate_to(Route::Gameplay(GameplayState::new(loaded_save)));
                        }
                    }
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    let timestamp = if let Route::Saves(state) = &self.route {
                        state.list_state.selected().and_then(|i| {
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
                        let old_note = self.current_save_metas.as_ref().and_then(|metas| {
                            metas.iter().find_map(|r| r.as_ref().ok()).and_then(|m| if m.timestamp == ts { Some(m.note.clone()) } else { None })
                        }).unwrap_or_default();
                        
                        self.open_note_editor(NoteEditAction::SaveAs(ts), &old_note);
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.open_note_editor(NoteEditAction::NewSave, "");
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                let timestamp = if let Route::Saves(state) = &self.route {
                    state.list_state.selected().and_then(|i| {
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
                    let old_note = self.current_save_metas.as_ref().and_then(|metas| {
                        metas.iter().find_map(|r| r.as_ref().ok()).and_then(|m| if m.timestamp == ts { Some(m.note.clone()) } else { None })
                    }).unwrap_or_default();
                    
                    self.open_note_editor(NoteEditAction::EditNote(ts), &old_note);
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                let timestamp = if let Route::Saves(state) = &self.route {
                    state.list_state.selected().and_then(|i| {
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
                let _ = self.refresh_saves();
            }
            KeyCode::Esc => {
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
                                    if !found_previous {
                                        self.selected_save_meta_id = None;
                                    }
                                }

                                state.list_state.select(Some(target_idx));
                                if let Some(Ok(meta)) = metas.get(target_idx) {
                                    let _ = self.select_save(meta.timestamp);
                                }
                            } else {
                                state.list_state.select(None);
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