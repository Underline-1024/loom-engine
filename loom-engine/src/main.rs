//! LLM-driven RPG engine - main entry point.
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use loom_engine::{llm::Narrator, app::create::CreateState};
use loom_engine::llm::tool::builtin_tools::init_save_data;
use loom_engine::config::{GameMode, LlmConfig};
use ratatui::widgets::ListState;
use tracing_subscriber::{fmt, EnvFilter};
use ratatui;
use loom_engine::app::{App, Route};

fn init_logging() {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .with_line_number(false)
        .with_file(false)
        .init();
}
#[tokio::main]
async fn main() -> Result<()> {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<KeyCode>();
    let key_tx = tx.clone();

    init_logging();
    init_save_data(GameMode::default());
    
    let narrator = Narrator::new();
    let dynamic_tool_count = 5;
    let llm_config = LlmConfig::load()?;
    narrator.init(&llm_config, dynamic_tool_count).await?;
    
    // let prompt = "请在背包里添加一瓶水。";
    // let res = narrator.chat(prompt).await?;
    // println!("{}", res);

    // let data = save_data();
    // let guard = data.lock().unwrap();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                },
                result = async { event::read() } => {
                    if let Ok(event) = result {
                        match event {
                            Event::Key(key) => {
                                if key.kind == KeyEventKind::Press {
                                    let _ = key_tx.send(key.code);
                                }
                            },
                            Event::Resize(_, _) => {
                                let _ = key_tx.send(KeyCode::Null);
                            },
                            _ => {},
                        }
                    }
                },
            }
            
        }
    });
    
    if let Ok(mut app) = App::new() {
        // 手动初始化终端
        let mut terminal = ratatui::init();
        
        loop {
            // 绘制
            terminal.draw(|frame| {
                match &mut app.route {
                    Route::MainMenu => app.render_main_menu(frame, frame.area()),
                    Route::Create(state) => App::render_create_form(frame, state),
                    Route::Gameplay(_) => app.render_gameplay(frame, frame.area()),
                    Route::Projects(_) => app.render_projects_view(frame, frame.area()),
                    Route::Settings => {},
                    Route::Help => {},
                    Route::Error(_) => app.render_error(frame, frame.area()),
                    Route::Saves(_) => app.render_saves(frame, frame.area()),
                }
            })?;
            
            // 异步等待按键
            tokio::select! {
                Some(key_code) = rx.recv() => {
                    if key_code == KeyCode::Null {
                        continue;
                    }

                    if key_code == KeyCode::Esc && matches!(app.route, Route::MainMenu) {
                        drop(shutdown_tx);
                        break;
                    }
                    
                    // 处理按键
                    match app.route {
                        Route::MainMenu => {
                            match key_code {
                                KeyCode::Up | KeyCode::Char('k') => app.previous_menu_item(),
                                KeyCode::Down | KeyCode::Char('j') => app.next_menu_item(),
                                KeyCode::Enter => {
                                    if let Route::MainMenu = app.route {
                                        match app.menu_state.selected() {
                                            Some(0) => app.navigate_to(Route::Create(CreateState::default())),
                                            Some(1) => {
                                                let mut state = ListState::default();
                                                state.select(Some(0));
                                                app.navigate_to(Route::Projects(state));
                                            },
                                            Some(2) => app.navigate_to(Route::Settings),
                                            _ => {},
                                        }
                                    }
                                },
                                _ => {},
                            }
                        },
                        Route::Create(_) => app.handle_create_input(key_code.into(), &narrator).await,
                        Route::Projects(_) => app.handle_projects_input(key_code.into()),
                        Route::Settings => {},
                        Route::Help => {},
                        Route::Gameplay(_) => app.handle_gameplay_input(key_code.into(), &narrator).await,
                        Route::Error(_) => {},
                        Route::Saves(_) => app.handle_saves_input(key_code.into()),
                    }
                    
                    if key_code == KeyCode::Esc && !matches!(app.route, Route::MainMenu) {
                        app.navigate_to(Route::MainMenu);
                    }
                },
            }
        }
        
        // 清理终端
        ratatui::restore();
    }
    
    Ok(())
}