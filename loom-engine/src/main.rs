//! LLM-driven RPG engine - main entry point.
use std::sync::OnceLock;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use loom_engine::app::settings::SettingsState;
use loom_engine::{llm::Narrator, app::create::CreateState};
use loom_engine::llm::tool::builtin_tools::{init_save_data, reset_save_data};
use loom_engine::config::{GameMode, LlmConfig};
use ratatui::widgets::ListState;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use loom_engine::app::{App, Route};
use tracing_appender;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn init_logging() {
    // 1. 创建日志目录
    std::fs::create_dir_all("logs").ok();
    
    // 2. 创建文件日志写入器
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    // 3. ⭐ 关键：保存 guard 到静态变量，防止被释放
    LOG_GUARD.set(guard).ok();
    
    // 4. 创建文件日志层（写入文件）
    let file_log = fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_timer(fmt::time::ChronoLocal::rfc_3339())
        .with_writer(non_blocking);
    
    // 5. 创建终端日志层（输出到终端）
    let terminal_log = fmt::layer()
        .with_target(false)
        .with_line_number(false)
        .with_file(false)
        .pretty()
        .with_writer(std::io::stderr);
    
    // 6. 注册所有层
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(file_log)
        .with(terminal_log)
        .init();
    
    // 7. 测试日志
    tracing::info!("✅ 日志系统初始化完成，日志保存在 logs/app.log");
}

#[tokio::main]
async fn main() -> Result<()> {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    // ✅ Channel 类型已改为 KeyEvent
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<KeyEvent>(); 
    let key_tx = tx.clone();

    init_logging();
    init_save_data(GameMode::default());
    
    let mut narrator = Narrator::new();
    let dynamic_tool_count = 5;
    let llm_config = LlmConfig::load()?;
    narrator.init(&llm_config, dynamic_tool_count).await?;

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
                                    let _ = key_tx.send(key);
                                }
                            },
                            Event::Resize(_, _) => {
                                // ✅ 发送一个特殊的 Null KeyEvent 作为 Resize 标记
                                let null_key = KeyEvent::new(KeyCode::Null, KeyModifiers::NONE);
                                let _ = key_tx.send(null_key);
                            },
                            _ => {},
                        }
                    }
                },
            }
        }
    });
    
    if let Ok(mut app) = App::new() {
        let mut terminal = ratatui::init();
        
        loop {
            // 绘制
            terminal.draw(|frame| {
                match &mut app.route {
                    Route::MainMenu => app.render_main_menu(frame, frame.area()),
                    Route::Create(state) => App::render_create_form(frame, state),
                    Route::Gameplay(_) => app.render_gameplay(frame, frame.area()),
                    Route::Projects(_) => app.render_projects_view(frame, frame.area()),
                    Route::Settings(_) => app.render_settings_form(frame, frame.area()),
                    Route::Help => {},
                    Route::Error(_) => app.render_error(frame, frame.area()),
                    Route::Saves(_) => app.render_saves(frame, frame.area()),
                }
            })?;
            
            // 异步等待按键
            tokio::select! {
                Some(key_event) = rx.recv() => {
                    // ✅ 从 KeyEvent 中提取 code 用于基础流程控制
                    let key_code = key_event.code;

                    if key_code == KeyCode::Null {
                        continue;
                    }

                    if key_code == KeyCode::Esc && matches!(app.route, Route::MainMenu) {
                        drop(shutdown_tx);
                        break;
                    }
                    
                    // ✅ 处理按键：直接传递 key_event，不再使用 .into()
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
                                            Some(2) => app.navigate_to(Route::Settings(SettingsState::default())),
                                            _ => {},
                                        }
                                    }
                                },
                                _ => {},
                            }
                        },
                        Route::Create(_) => app.handle_create_input(key_event, &narrator).await,
                        Route::Projects(_) => app.handle_projects_input(key_event),
                        Route::Settings(_) => app.handle_settings_input(key_event).await,
                        Route::Help => {},
                        Route::Gameplay(_) => app.handle_gameplay_input(key_event, &narrator).await,
                        Route::Error(_) => {},
                        Route::Saves(_) => app.handle_saves_input(key_event, &mut narrator).await,
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