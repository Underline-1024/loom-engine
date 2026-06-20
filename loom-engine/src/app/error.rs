use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect}, 
    style::{Color, Modifier, Style}, 
    text::{Span, Text}, 
    widgets::{Block, BorderType, Borders, Paragraph}, 
    Frame,
};
use super::{App, Route};

impl App {
    pub fn render_error(&self, frame: &mut Frame, area: Rect) {
        if let Route::Error(error) = &self.route {
            // 获取错误信息
            let error_message = error.to_string();
            
            // 构建错误链
            let mut chain = Vec::new();
            chain.push(error_message.clone());
            let mut source = error.source();
            while let Some(err) = source {
                chain.push(format!("  ⤷  {}", err));
                source = err.source();
            }
            
            // 构建显示文本
            let display_text = if chain.len() > 1 {
                format!(
                    "{}\n\n{}",
                    chain[0],
                    chain[1..].join("\n")
                )
            } else {
                error_message
            };
            
            // 创建错误块
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .border_type(BorderType::Rounded)
                .title(Span::styled(
                    "  ❌  Error  ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                ))
                .title_alignment(Alignment::Center);
            
            // 错误内容
            let text = Text::from(display_text);
            let paragraph = Paragraph::new(text)
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Center)
                .block(block);
            
            // 底部提示
            let help_text = Text::from("Press ESC to return to main menu");
            let help_paragraph = Paragraph::new(help_text)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            
            // 布局 - 使用传入的 area 参数
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(35),   // 上边距
                    Constraint::Length(10),       // 错误信息
                    Constraint::Length(3),        // 帮助信息
                    Constraint::Percentage(35),   // 下边距
                ])
                .split(area);  // 使用传入的 area
            
            frame.render_widget(paragraph, layout[1]);
            frame.render_widget(help_paragraph, layout[2]);
        }
    }
}