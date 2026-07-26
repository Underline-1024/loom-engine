use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use ratatui::{self, Frame};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crate::config::LlmConfig;
use ratatui_textarea::TextArea;
use super::{App, Route};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, Default)]
pub enum SettingsField {
    #[default]
    Provider,
    EnableDynamic,
    Model,
    EmbeddingModel,
    MaxTokens,
    MaxTurns,
    SystemPrompt,
    Confirm,
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub provider: TextArea<'static>,
    pub enable_dynamic: bool,
    pub model: TextArea<'static>,
    pub embedding_model: TextArea<'static>,
    pub max_tokens: TextArea<'static>,
    pub max_turns: TextArea<'static>,
    pub system_prompt: TextArea<'static>,
    pub focused_field: SettingsField,
    pub error_msg: Option<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        let mut provider = TextArea::default();
        let mut model = TextArea::default();
        let mut embedding_model = TextArea::default();
        let mut max_tokens = TextArea::default();
        let mut max_turns = TextArea::default();
        let mut system_prompt = TextArea::default();
        let mut enable_dynamic = false;

        // 尝试读取现有配置进行预填充
        if let Ok(config) = LlmConfig::load() {
            provider.insert_str(&config.provider);
            model.insert_str(&config.model);
            if let Some(em) = config.embedding_model {
                embedding_model.insert_str(&em);
            }
            if let Some(mt) = config.max_tokens {
                max_tokens.insert_str(&mt.to_string());
            }
            max_turns.insert_str(&config.max_turns.to_string());
            system_prompt.insert_str(&config.system_prompt);
            enable_dynamic = config.enable_dynamic;
        }

        Self {
            provider,
            enable_dynamic,
            model,
            embedding_model,
            max_tokens,
            max_turns,
            system_prompt,
            focused_field: SettingsField::Provider,
            error_msg: None,
        }
    }
}

impl App {
    pub async fn handle_settings_input(&mut self, event: KeyEvent) {
        if let Route::Settings(ref mut state) = self.route {
            let key = event.code;
        
            // Tab 切换焦点
            if key == KeyCode::Tab {
                state.focused_field = match state.focused_field {
                    SettingsField::Provider => SettingsField::EnableDynamic,
                    SettingsField::EnableDynamic => SettingsField::Model,
                    SettingsField::Model => SettingsField::EmbeddingModel,
                    SettingsField::EmbeddingModel => SettingsField::MaxTokens,
                    SettingsField::MaxTokens => SettingsField::MaxTurns,
                    SettingsField::MaxTurns => SettingsField::SystemPrompt,
                    SettingsField::SystemPrompt => SettingsField::Confirm,
                    SettingsField::Confirm => SettingsField::Provider,
                };
                return;
            }

            match state.focused_field {
                SettingsField::Provider => {
                    if key == KeyCode::Enter || (event.code == KeyCode::Char('m') && event.modifiers.contains(KeyModifiers::CONTROL)) {
                        return;
                    }
                    state.provider.input(event);
                },
                SettingsField::EnableDynamic => {
                    match key {
                        KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                            state.enable_dynamic = !state.enable_dynamic;
                        }
                        _ => {}
                    }
                },
                SettingsField::Model => {
                    if key == KeyCode::Enter || (event.code == KeyCode::Char('m') && event.modifiers.contains(KeyModifiers::CONTROL)) {
                        return;
                    }
                    state.model.input(event);
                },
                SettingsField::EmbeddingModel => {
                    if key == KeyCode::Enter || (event.code == KeyCode::Char('m') && event.modifiers.contains(KeyModifiers::CONTROL)) {
                        return;
                    }
                    state.embedding_model.input(event);
                },
                SettingsField::MaxTokens => {
                    if key == KeyCode::Enter || (event.code == KeyCode::Char('m') && event.modifiers.contains(KeyModifiers::CONTROL)) {
                        return;
                    }
                    state.max_tokens.input(event);
                },
                SettingsField::MaxTurns => {
                    if key == KeyCode::Enter || (event.code == KeyCode::Char('m') && event.modifiers.contains(KeyModifiers::CONTROL)) {
                        return;
                    }
                    state.max_turns.input(event);
                },
                SettingsField::SystemPrompt => { 
                    state.system_prompt.input(event); 
                },
                SettingsField::Confirm => {
                    if key == KeyCode::Enter {
                        let provider_text = state.provider.lines().join("");
                        let model_text = state.model.lines().join("");
                        let embedding_text = state.embedding_model.lines().join("");
                        let max_tokens_text = state.max_tokens.lines().join("");
                        let max_turns_text = state.max_turns.lines().join("");
                        let prompt_text = state.system_prompt.lines().join("\n");

                        if provider_text.is_empty() || model_text.is_empty() {
                            state.error_msg = Some("Provider and Model cannot be empty".to_string());
                            return;
                        }

                        let embedding_model = if embedding_text.is_empty() {
                            None
                        } else {
                            Some(embedding_text)
                        };

                        let max_tokens = if max_tokens_text.is_empty() {
                            None
                        } else {
                            match max_tokens_text.parse::<u64>() {
                                Ok(v) => Some(v),
                                Err(_) => {
                                    state.error_msg = Some("Max Tokens must be a valid number".to_string());
                                    return;
                                }
                            }
                        };

                        let max_turns = match max_turns_text.parse::<usize>() {
                            Ok(v) => v,
                            Err(_) => {
                                state.error_msg = Some("Max Turns must be a valid number".to_string());
                                return;
                            }
                        };

                        // 加载旧配置，如果不存在则使用默认值兜底
                        let mut config = match LlmConfig::load() {
                            Ok(c) => c,
                            Err(_) => LlmConfig {
                                provider: String::new(),
                                enable_dynamic: false,
                                model: String::new(),
                                embedding_model: None,
                                system_prompt: String::new(),
                                max_tokens: None,
                                max_turns: 10,
                            }
                        };

                        config.provider = provider_text;
                        config.enable_dynamic = state.enable_dynamic;
                        config.model = model_text;
                        config.embedding_model = embedding_model;
                        config.max_tokens = max_tokens;
                        config.max_turns = max_turns;
                        config.system_prompt = prompt_text;

                        let path = PathBuf::from("./configs/llm_config.json");
                        match serde_json::to_string_pretty(&config) {
                            Ok(json) => {
                                if let Err(e) = fs::write(&path, json) {
                                    state.error_msg = Some(format!("Failed to save config: {}", e));
                                    return;
                                }
                            },
                            Err(e) => {
                                state.error_msg = Some(format!("Failed to serialize config: {}", e));
                                return;
                            }
                        }
                        
                        state.error_msg = None;
                        self.route = Route::MainMenu;
                    }
                }
            }
        }
    }

    pub fn render_settings_form(&mut self, frame: &mut Frame, area: Rect) {
        if let Route::Settings(state) = &mut self.route {
            // 增加高度以容纳更多字段
            let popup_area = centered_rect(60, 85, area);
            frame.render_widget(Clear, popup_area);
            
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(3),   // 标题
                    Constraint::Length(3),   // Provider
                    Constraint::Length(3),   // Enable Dynamic
                    Constraint::Length(3),   // Model
                    Constraint::Length(3),   // Embedding Model
                    Constraint::Length(3),   // Max Tokens
                    Constraint::Length(3),   // Max Turns
                    Constraint::Min(5),      // System Prompt
                    Constraint::Length(3),   // Confirm
                    Constraint::Length(1),   // Error
                    Constraint::Min(0),
                ])
                .split(popup_area);
            
            let title = Paragraph::new("LLM Settings")
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(title, chunks[0]);
            
            // 1. Provider
            let provider_block = Block::default()
                .borders(Borders::ALL)
                .title("Provider")
                .border_style(if let SettingsField::Provider = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            state.provider.set_block(provider_block);
            if let SettingsField::Provider = state.focused_field {
                state.provider.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
                state.provider.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            } else {
                state.provider.set_cursor_style(Style::default());
                state.provider.set_cursor_line_style(Style::default());
            }
            frame.render_widget(&state.provider, chunks[1]);

            // 2. Enable Dynamic (Boolean)
            let dynamic_hint = if let SettingsField::EnableDynamic = state.focused_field { " ← → to switch" } else { "" };
            let dynamic_text = format!("{}{}", state.enable_dynamic, dynamic_hint);
            let dynamic_style = if let SettingsField::EnableDynamic = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() };
            let dynamic_block = Block::default()
                .borders(Borders::ALL)
                .title("Enable Dynamic")
                .border_style(if let SettingsField::EnableDynamic = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            let dynamic_paragraph = Paragraph::new(dynamic_text).block(dynamic_block).style(dynamic_style);
            frame.render_widget(dynamic_paragraph, chunks[2]);

            // 3. Model
            let model_block = Block::default()
                .borders(Borders::ALL)
                .title("Model")
                .border_style(if let SettingsField::Model = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            state.model.set_block(model_block);
            if let SettingsField::Model = state.focused_field {
                state.model.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
                state.model.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            } else {
                state.model.set_cursor_style(Style::default());
                state.model.set_cursor_line_style(Style::default());
            }
            frame.render_widget(&state.model, chunks[3]);

            // 4. Embedding Model
            let embedding_block = Block::default()
                .borders(Borders::ALL)
                .title("Embedding Model (Optional)")
                .border_style(if let SettingsField::EmbeddingModel = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            state.embedding_model.set_block(embedding_block);
            if let SettingsField::EmbeddingModel = state.focused_field {
                state.embedding_model.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
                state.embedding_model.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            } else {
                state.embedding_model.set_cursor_style(Style::default());
                state.embedding_model.set_cursor_line_style(Style::default());
            }
            frame.render_widget(&state.embedding_model, chunks[4]);

            // 5. Max Tokens
            let max_tokens_block = Block::default()
                .borders(Borders::ALL)
                .title("Max Tokens (Optional)")
                .border_style(if let SettingsField::MaxTokens = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            state.max_tokens.set_block(max_tokens_block);
            if let SettingsField::MaxTokens = state.focused_field {
                state.max_tokens.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
                state.max_tokens.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            } else {
                state.max_tokens.set_cursor_style(Style::default());
                state.max_tokens.set_cursor_line_style(Style::default());
            }
            frame.render_widget(&state.max_tokens, chunks[5]);

            // 6. Max Turns
            let max_turns_block = Block::default()
                .borders(Borders::ALL)
                .title("Max Turns")
                .border_style(if let SettingsField::MaxTurns = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            state.max_turns.set_block(max_turns_block);
            if let SettingsField::MaxTurns = state.focused_field {
                state.max_turns.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
                state.max_turns.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            } else {
                state.max_turns.set_cursor_style(Style::default());
                state.max_turns.set_cursor_line_style(Style::default());
            }
            frame.render_widget(&state.max_turns, chunks[6]);
            
            // 7. System Prompt
            let prompt_block = Block::default()
                .borders(Borders::ALL)
                .title("System Prompt")
                .border_style(if let SettingsField::SystemPrompt = state.focused_field { Style::default().fg(Color::Yellow) } else { Style::default() });
            state.system_prompt.set_block(prompt_block);
            if let SettingsField::SystemPrompt = state.focused_field {
                state.system_prompt.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
                state.system_prompt.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            } else {
                state.system_prompt.set_cursor_style(Style::default());
                state.system_prompt.set_cursor_line_style(Style::default());
            }
            frame.render_widget(&state.system_prompt, chunks[7]);
            
            // 8. Confirm
            let confirm_text = if let SettingsField::Confirm = state.focused_field { "✓ SAVE & EXIT (Press Enter)" } else { "✓ SAVE & EXIT" };
            let confirm_style = if let SettingsField::Confirm = state.focused_field { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Green) };
            let confirm_block = Block::default()
                .borders(Borders::ALL)
                .border_style(if let SettingsField::Confirm = state.focused_field { Style::default().fg(Color::Green) } else { Style::default() });
            let confirm_paragraph = Paragraph::new(confirm_text)
                .block(confirm_block)
                .style(confirm_style)
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(confirm_paragraph, chunks[8]);
            
            // 9. Error
            if let Some(error) = &state.error_msg {
                let error_paragraph = Paragraph::new(error.as_str())
                    .style(Style::default().fg(Color::Red))
                    .alignment(ratatui::layout::Alignment::Center);
                frame.render_widget(error_paragraph, chunks[9]);
            }
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