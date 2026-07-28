use super::{App, Route};
use ratatui::style::{Color, Style};
use ratatui::{self, Frame};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};

const TITLE: &str = r#" 
██╗      ██████╗  ██████╗ ███╗   ███╗      ███████╗███╗   ██╗ ██████╗ ██╗███╗   ██╗███████╗
██║     ██╔═══██╗██╔═══██╗████╗ ████║      ██╔════╝████╗  ██║██╔════╝ ██║████╗  ██║██╔════╝
██║     ██║   ██║██║   ██║██╔████╔██║█████╗█████╗  ██╔██╗ ██║██║  ███╗██║██╔██╗ ██║█████╗  
██║     ██║   ██║██║   ██║██║╚██╔╝██║╚════╝██╔══╝  ██║╚██╗██║██║   ██║██║██║╚██╗██║██╔══╝  
███████╗╚██████╔╝╚██████╔╝██║ ╚═╝ ██║      ███████╗██║ ╚████║╚██████╔╝██║██║ ╚████║███████╗
╚══════╝ ╚═════╝  ╚═════╝ ╚═╝     ╚═╝      ╚══════╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝╚══════╝
"#;

const JOKES: &[&str] = &[
    "todo!(\"There should have been some jokes here.\");",
];

pub fn get_random_joke() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    JOKES[(seed as usize) % JOKES.len()]
}

impl App {
    pub fn next_menu_item(&mut self) {
        let count = self.current_menu_count() as usize;
        if count == 0 {
            return;
        }
        
        let current = self.menu_state.selected().unwrap_or(0);
        let next = (current + 1) % count;
        self.menu_state.select(Some(next));
    }
    
    pub fn previous_menu_item(&mut self) {
        let count = self.current_menu_count() as usize;
        if count == 0 {
            return;
        }
        
        let current = self.menu_state.selected().unwrap_or(0);
        let prev = (current + count - 1) % count;
        self.menu_state.select(Some(prev));
    }
    
    pub fn current_menu_count(&self) -> u16 {
        match self.route {
            Route::MainMenu => 3,
            _ => 0,
        }
    }

    pub fn render_main_menu(&mut self, frame: &mut Frame, area: Rect) {
        let title_lines: Vec<&str> = TITLE.lines().collect();
        let title_max_width = title_lines.iter().map(|line| line.trim().chars().count()).max().unwrap_or(0) as u16;
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(title_lines.len() as u16 + 2),
                Constraint::Length(self.current_menu_count() + 2),
                Constraint::Min(0),
            ])
            .split(area);
        
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(title_max_width + 10),
                Constraint::Min(0),
            ])
            .split(chunks[0]);  // ← 注意：这里是 chunks[0]
        
        // 渲染标题
        let block = Block::default()
            .borders(Borders::ALL)
            .title("loom-engine v0.1.0");
        let p = Paragraph::new(TITLE)
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(p, header_chunks[0]);
        
        // 渲染笑话
        let joke = get_random_joke();
        let joke_block = Block::default()
            .borders(Borders::ALL)
            .title(" 💡 Dev Joke ")
            .title_style(Style::default().fg(Color::Yellow).bold())
            .border_style(Style::default().fg(Color::DarkGray));
        let joke_text = Paragraph::new(joke)
            .block(joke_block)
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: true });
        frame.render_widget(joke_text, header_chunks[1]);
        
        // 渲染菜单
        let items = vec![
            ListItem::new("1. Create Project"),
            ListItem::new("2. Load Project"),
            ListItem::new("3. Settings"),
        ];
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Main Menu "))
            .highlight_style(Style::default().bg(Color::DarkGray));
        
        // ✅ 修复：使用 chunks[1]（菜单区域）
        frame.render_stateful_widget(list, chunks[1], &mut self.menu_state);
    }
}