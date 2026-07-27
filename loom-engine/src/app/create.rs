use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use ratatui::{self, Frame};
use ratatui::widgets::{Block, Borders, Clear, ListState, Paragraph};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crate::config::GameMode;
use crate::llm::Narrator;
use crate::project::Project;
use crate::app::AppEvent;
use ratatui_textarea::TextArea;
use super::{App, Route};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Default)]
pub enum CreateField {
    #[default]
    Name,
    Mode,
    Prompt,
    Confirm,
}

#[derive(Debug, Clone)]
pub struct CreateState {
    pub name: TextArea<'static>,                    // 单行，保持 String
    pub mode: GameMode,
    pub prompt: TextArea<'static>,                 // 多行编辑器，替代原来的 String
    pub focused_field: CreateField,
    pub name_cursor: usize,
    pub error_msg: Option<String>,
    pub editing_project: Option<Project>,
    pub is_processing: bool,                       // 🌟 新增：Loading 状态
}

impl CreateState {
    // 🌟 新增：从已有项目创建编辑状态 (预填充数据)
    pub fn from_project(project: Project) -> Self {
        let mut state = Self::default();
        let config = project.config();
        
        state.name.insert_str(&config.name);
        state.mode = config.mode.clone();
        state.prompt.insert_str(&config.prompt);
        
        state.editing_project = Some(project);
        state
    }
}

impl Default for CreateState {
    fn default() -> Self {
        let name = TextArea::default();

        let mut prompt = TextArea::default();
        prompt.set_line_number_style(Style::new().dark_gray());
        Self {
            name,
            mode: GameMode::Normal,
            prompt,   // 空的多行编辑器
            focused_field: CreateField::Name,
            name_cursor: 0,
            error_msg: None,
            editing_project: None,
            is_processing: false,
        }
    }
}

impl App {
    // 处理创建表单输入
    // 🌟 修改签名：接收 Arc<Narrator> 和 tx
    pub async fn handle_create_input(&mut self, event: KeyEvent, narrator: Arc<Narrator>, tx: UnboundedSender<AppEvent>) {
        if let Route::Create(ref mut state) = self.route{
            // 🌟 拦截处理中的按键，防止状态错乱
            if state.is_processing {
                return;
            }

            let key = event.code;
        
            // Tab 切换焦点
            if key == KeyCode::Tab {
                state.focused_field = match state.focused_field {
                    CreateField::Name => CreateField::Mode,
                    CreateField::Mode => CreateField::Prompt,
                    CreateField::Prompt => CreateField::Confirm,
                    CreateField::Confirm => CreateField::Name,
                };
                return;
            }

            match state.focused_field {
                CreateField::Name => {
                    // 检查是否是换行键 (Enter 或 Ctrl+M)
                    if key == KeyCode::Enter || (event.code == KeyCode::Char('m') && event.modifiers.contains(KeyModifiers::CONTROL)) {
                        // 忽略换行，不传递给 TextArea
                        return;
                    }
                    state.name.input(event);
                },
                CreateField::Mode => Self::handle_mode_input(state, key),
                CreateField::Prompt => { state.prompt.input(event); },
                CreateField::Confirm => {
                    if key == KeyCode::Enter {
                        // 验证名称不能为空
                        if state.name.is_empty() {
                            state.error_msg = Some("Project name cannot be empty".to_string());
                            return;
                        }

                        let name_text = state.name.lines().join("");
                        let prompt_text = state.prompt.lines().join("\n");
                        let mode_clone = state.mode.clone();

                        if let Some(mut old_project) = state.editing_project.clone() {
                            // 🌟 === 编辑模式：更新项目 (同步执行) ===
                            match old_project.update_config(name_text, mode_clone, prompt_text) {
                                Ok(_) => {
                                    state.error_msg = None;
                                    let mut list_state = ListState::default();
                                    list_state.select(Some(0));
                                    self.route = Route::Projects(list_state);
                                    let _ = self.refresh_projects(); // 刷新列表以显示新名字/配置
                                }
                                Err(e) => state.error_msg = Some(e.to_string()),
                            }
                        } else {
                            // 🌟 === 创建模式：新建项目 (异步执行，防止卡死) ===
                            state.is_processing = true;
                            state.error_msg = None;

                            let narrator_clone = narrator.clone();
                            let tx_clone = tx.clone();

                            tokio::spawn(async move {
                                let result = Project::create(name_text, mode_clone, prompt_text.as_str(), &narrator_clone).await;
                                
                                let event_msg = match result {
                                    Ok(p) => AppEvent::ProjectCreated(Ok(p)),
                                    Err(e) => AppEvent::ProjectCreated(Err(e.to_string())),
                                };
                                let _ = tx_clone.send(event_msg);
                            });
                        }
                    }
                }
            }
        }
    }

    // 模式字段输入（左右键切换）
    fn handle_mode_input(state: &mut CreateState, key: KeyCode) {
        match key {
            KeyCode::Left | KeyCode::Char('h') => {
                state.mode = match state.mode {
                    GameMode::Normal => GameMode::Author,
                    GameMode::Author => GameMode::Normal,
                };
            }
            KeyCode::Right | KeyCode::Char('l') => {
                state.mode = match state.mode {
                    GameMode::Normal => GameMode::Author,
                    GameMode::Author => GameMode::Normal,
                };
            }
            _ => {}
        }
    }

    pub fn render_create_form(frame: &mut Frame, state: &mut CreateState) {
        let area = frame.area();
        
        // 居中弹窗
        let popup_area = centered_rect(60, 70, area);
        frame.render_widget(Clear, popup_area);
        
        // 布局
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),   // 标题
                Constraint::Length(3),   // Name
                Constraint::Length(3),   // Mode
                Constraint::Min(3),     // Prompt
                Constraint::Length(3),   // Confirm
                Constraint::Length(1),   // Error
                Constraint::Min(0),
            ])
            .split(popup_area);
        
        // 标题
        let title_text = if state.editing_project.is_some() {
            "Edit Project"
        } else {
            "Create New Project"
        };
        
        let title = Paragraph::new(title_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(ratatui::layout::Alignment::Center);
        
        frame.render_widget(title, chunks[0]);
        
        // Name 字段
        let name_block = Block::default()
            .borders(Borders::ALL)
            .title("Name")
            .border_style(if let CreateField::Name = state.focused_field {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });

        state.name.set_block(name_block);

        if let CreateField::Name = state.focused_field {
            // 聚焦时：反转色光标 + 下划线光标行
            state.name.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            state.name.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
        } else {
            // 失焦时：清除所有光标样式
            state.name.set_cursor_style(Style::default());
            state.name.set_cursor_line_style(Style::default());
        }
        
        frame.render_widget(&state.name, chunks[1]);
        
        // Mode 字段
        let mode_hint = if let CreateField::Mode = state.focused_field {
            " ← → to switch"
        } else {
            ""
        };
        let mode_text = format!("{:?}{}", state.mode, mode_hint);
        
        let mode_style = if let CreateField::Mode = state.focused_field {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        
        let mode_block = Block::default()
            .borders(Borders::ALL)
            .title("Game Mode")
            .border_style(if let CreateField::Mode = state.focused_field {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        
        let mode_paragraph = Paragraph::new(mode_text)
            .block(mode_block)
            .style(mode_style);
        frame.render_widget(mode_paragraph, chunks[2]);
        
        // Prompt 字段
        // 注意：TextArea 自带光标和编辑功能
        let prompt_block = Block::default()
            .borders(Borders::ALL)
            .title("Prompt")
            .border_style(if let CreateField::Prompt = state.focused_field {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        
        // TextArea 需要设置 block
        state.prompt.set_block(prompt_block);

        if let CreateField::Prompt = state.focused_field {
            // 聚焦时：反转色光标 + 下划线光标行
            state.prompt.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            state.prompt.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
        } else {
            // 失焦时：清除所有光标样式
            state.prompt.set_cursor_style(Style::default());
            state.prompt.set_cursor_line_style(Style::default());
        }

        frame.render_widget(&state.prompt, chunks[3]);
        
        // Confirm 按钮
        let confirm_text = if state.is_processing {
            "⏳ CREATING & INITIALIZING WORLD..."
        } else if let CreateField::Confirm = state.focused_field {
            "✓ CONFIRM (Press Enter)"
        } else {
            "✓ CONFIRM"
        };
        
        let confirm_style = if state.is_processing {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if let CreateField::Confirm = state.focused_field {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };

        let confirm_border_style = if state.is_processing {
            Style::default().fg(Color::Yellow)
        } else if let CreateField::Confirm = state.focused_field {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        
        let confirm_block = Block::default()
            .borders(Borders::ALL)
            .border_style(confirm_border_style);
        
        let confirm_paragraph = Paragraph::new(confirm_text)
            .block(confirm_block)
            .style(confirm_style)
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(confirm_paragraph, chunks[4]);
        
        // 错误信息
        if let Some(error) = &state.error_msg {
            let error_paragraph = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(error_paragraph, chunks[5]);
        }
    }
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Length((r.height.saturating_sub(height)) / 2),
        ])
        .split(r);
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Length((r.width.saturating_sub(width)) / 2),
        ])
        .split(popup_layout[1])[1]
}