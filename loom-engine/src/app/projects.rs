use chrono::DateTime;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span, Text}, widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap}, Frame
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use anyhow::{bail, Result};
use std::fs;

use crate::{app::App, save::SaveData};
use crate::project::{list_projects, Project};
use crate::app::Route;
use crate::config::GameMode;
use super::{create::CreateState, gameplay::GameplayState};

impl App {
    pub fn select_project(&mut self, id: i64) -> Result<()> {
        for item in &self.projects {
            if let Ok(project) = item {
                if project.timestamp() == id {
                    self.selected_project_id = Some(id);
                    let project = project;
                    self.current_save_metas = Some(project.list_saves()?);
                    return Ok(());
                }
            }
        }
        bail!(format!("The project with timestamp {} does not exist.", id))
    }
    pub fn get_mut_project(&mut self) -> Result<&mut Project> {
        if let Some(timestamp) = self.selected_project_id {
            for item in &mut self.projects {
                if let Ok(project) = item && project.timestamp() == timestamp {
                    return Ok(project);
                }
            }
            bail!(format!("There is not a project with timestamp {}.", timestamp))
        }
        bail!("Unselected project.")
    }
    pub fn get_projects(&self) -> &Vec<Result<Project>> {
        &self.projects
    }
    // 渲染项目视图
    pub fn render_projects_view(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage(65),
            ])
            .split(area);

        self.render_project_list(frame, chunks[0]);
        self.render_project_details(frame, chunks[1]);
    }

    fn render_project_list(&mut self, frame: &mut Frame, area: Rect) {
        let mut items: Vec<ListItem> = vec![];
        for item in &self.projects {
            match item {
                Ok(project) => {
                    items.push(
                        ListItem::new(project.name())
                            .style(Style::default().fg(Color::Gray))
                    );
                }, 
                Err(error) => {
                    items.push(
                        ListItem::new(Text::from(format!("[ERROR] {}", error)))
                            .style(Style::default().fg(Color::Red))
                    );
                }
            }
        }
        let project_list = List::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(" 📁 Projects ")
                .border_style(Style::default().fg(Color::Cyan)))
            .highlight_style(Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        if let Route::Projects(state) = &mut self.route {
            frame.render_stateful_widget(project_list, area, state);
            self.selected_project_index = state.selected().unwrap_or(0);
        }
    }

    fn render_project_details(&mut self, frame: &mut Frame, area: Rect) {
        let details_block = Block::default()
            .borders(Borders::ALL)
            .title(" Project Details ")
            .border_style(Style::default().fg(Color::Cyan));
    
        match self.projects.get(self.selected_project_index) {
            Some(Ok(project)) => {
                let config = project.config();
                
                let mode_text = match config.mode {
                    GameMode::Normal => "[Normal]",
                    GameMode::Author => "[Author]",
                };
    
                let datetime = DateTime::from_timestamp_millis(config.timestamp)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
    
                let text = vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(&config.name),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Mode: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(mode_text),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Prompt: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(&config.prompt),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Created: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::from(datetime),
                    ]),
                ];
    
                let paragraph = Paragraph::new(text)
                    .block(details_block)
                    .wrap(Wrap { trim: true });
                frame.render_widget(paragraph, area);
            }
            Some(Err(error)) => {
                let text = vec![
                    Line::from(vec![
                        Span::styled("[ERROR] Error", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    ]),
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
                let text = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("No project selected", Style::default().fg(Color::Gray)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Use Up/Down to navigate and select a project", Style::default().fg(Color::DarkGray)),
                    ]),
                ];
    
                let paragraph = Paragraph::new(text)
                    .block(details_block)
                    .alignment(Alignment::Center);
                frame.render_widget(paragraph, area);
            }
        }
    }

    // 处理项目视图的输入
    pub fn handle_projects_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                // 单独处理，不与其他分支共享借用
                if let Route::Projects(_) = &mut self.route {
                    self.previous_project();
                }
            }
            KeyCode::Down => {
                if let Route::Projects(_) = &mut self.route {
                    self.next_project();
                }
            }
            
            KeyCode::Enter => {
                // Enter 键只需要读取 state，不需要可变借用
                let selected_index = if let Route::Projects(state) = &self.route {
                    state.selected()
                } else {
                    None
                };
                if let Some(selected_index) = selected_index {
                    if let Some(Ok(project)) = self.projects.get(selected_index) {
                        let _ = self.select_project(project.timestamp());
                        self.navigate_to(Route::Saves(ListState::default()));
                    }
                }
            }
            
            KeyCode::Esc => {
                self.navigate_to(Route::MainMenu);
                self.selected_project_id = None;
                self.current_save_metas = None;
                self.selected_save_meta_id = None;
            }
            
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    let _ = self.refresh_projects();
                }
            }
            
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.navigate_to(Route::Create(CreateState::default()));
                }
            }
            
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    let _ = self.delete_current_project();
                }
            }
            
            KeyCode::Char('e') | KeyCode::Char('E') => {
                let selected_index = if let Route::Projects(state) = &self.route {
                    state.selected()
                } else {
                    None
                };
                
                if let Some(idx) = selected_index {
                    if let Some(Ok(project)) = self.projects.get(idx) {
                        let edit_state = CreateState::from_project(project.clone());
                        self.navigate_to(Route::Create(edit_state));
                    }
                }
            }

            _ => {}
        }
    }

    fn previous_project(&mut self) {
        if let Route::Projects(state) = &mut self.route {
            let total = self.projects.len();
            if total == 0 {
                return;
            }
            
            let i = match state.selected() {
                Some(i) => {
                    if i > 0 {
                        i - 1
                    } else {
                        total - 1
                    }
                }
                None => 0,
            };
            
            state.select(Some(i));
            
            if let Some(Ok(project)) = self.projects.get(i) {
                let _ = self.select_project(project.timestamp());
            }
        }
        
    }

    fn next_project(&mut self) {
        if let Route::Projects(state) = &mut self.route {
            let total = self.projects.len();
            if total == 0 {
                return;
            }
            
            let i = match state.selected() {
                Some(i) => {
                    if i + 1 < total {
                        i + 1
                    } else {
                        0
                    }
                }
                None => 0,
            };
            
            state.select(Some(i));
            
            if let Some(Ok(project)) = self.projects.get(i) {
                let _ = self.select_project(project.timestamp());
            }
        }
        
    }

    pub fn refresh_projects(&mut self) -> Result<()> {
        self.projects = list_projects()?;
        
        // 单独借用，不影响其他操作
        if let Route::Projects(state) = &mut self.route {
            if let Some(selected_id) = self.selected_project_id {
                let mut found = false;
                for (idx, project_result) in self.projects.iter().enumerate() {
                    if let Ok(project) = project_result {
                        if project.timestamp() == selected_id {
                            state.select(Some(idx));
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    self.selected_project_id = None;
                    self.current_save_metas = None;
                    self.selected_save_meta_id = None;
                    
                    if !self.projects.is_empty() {
                        state.select(Some(0));
                        if let Some(Ok(project)) = self.projects.get(0) {
                            let _ = self.select_project(project.timestamp());
                        }
                    } else {
                        state.select(None);
                    }
                }
            } else if !self.projects.is_empty() && state.selected().is_none() {
                state.select(Some(0));
                if let Some(Ok(project)) = self.projects.get(0) {
                    let _ = self.select_project(project.timestamp());
                }
            }
        }
        
        Ok(())
    }

    fn delete_current_project(&mut self) -> Result<()> {
        // 先获取选中的索引
        let selected_index = if let Route::Projects(state) = &self.route {
            state.selected()
        } else {
            return Ok(());
        };
        
        if let Some(selected_index) = selected_index {
            if let Some(Ok(project)) = self.projects.get(selected_index) {
                let project_path = project.path.clone();
                
                if project_path.exists() {
                    fs::remove_dir_all(&project_path)?;
                }
                
                self.refresh_projects()?;
            }
        }
        Ok(())
    }
}