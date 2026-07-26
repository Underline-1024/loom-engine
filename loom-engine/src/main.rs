//! LLM-driven RPG engine - main entry point.
use std::io::stdout;
use std::sync::OnceLock;
use anyhow::Result;
use crossterm::event::{self, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use loom_engine::app::settings::SettingsState;
use loom_engine::{llm::Narrator, app::create::CreateState};
use loom_engine::llm::tool::builtin_tools::{init_save_data, reset_save_data};
use loom_engine::config::{GameMode, LlmConfig};
use ratatui::widgets::ListState;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use loom_engine::app::{App, AppEvent, Route};
use tracing_appender;
use std::sync::Arc;

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
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let key_tx = tx.clone();

    init_logging();
    init_save_data(GameMode::default());
    
    // 1. 正常创建和初始化 Narrator
    let mut narrator_instance = Narrator::new();
    let dynamic_tool_count = 5;
    let llm_config = LlmConfig::load()?;
    narrator_instance.init(&llm_config, dynamic_tool_count).await?;
    
    // 🌟 2. 将初始化好的 narrator 包装进 Arc 中
    let narrator = Arc::new(narrator_instance);

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
                                    let _ = key_tx.send(AppEvent::Key(key));
                                }
                            },
                            Event::Resize(_, _) => {
                                let _ = key_tx.send(AppEvent::Resize);
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
        execute!(stdout(), EnableMouseCapture).unwrap();

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
                Some(event) = rx.recv() => {
                    match event {
                        // 1. 窗口大小改变：什么都不做，直接 fall through 到下一次循环触发重绘
                        AppEvent::Resize => {},
                        
                        AppEvent::LlmResponse(response) => {
                            // 1. 调用 gameplay.rs 中的方法处理 AI 回复（解析 JSON、更新 guard.history 等）
                            app.handle_llm_response(&response).await;
                            
                            // 2. 关闭 Loading 状态，并强制滚动到底部
                            if let Route::Gameplay(state) = &mut app.route {
                                state.is_processing = false;
                                state.dialogue_scroll_offset = usize::MAX; 
                            }
                        },
                        
                        AppEvent::LlmError(err) => {
                            // 发生错误时，显示系统提示并关闭 Loading
                            app.add_dialogue("System", format!("LLM Error: {}", err));
                            if let Route::Gameplay(state) = &mut app.route {
                                state.is_processing = false;
                            }
                        },
                        
                        // 4. 键盘事件：原有的路由分发逻辑
                        AppEvent::Key(key_event) => {
                            let key_code = key_event.code;
    
                            // 忽略 Null 键（兼容旧逻辑或特殊标记）
                            if key_code == KeyCode::Null {
                                continue;
                            }
    
                            // 主菜单按 Esc 退出应用
                            if key_code == KeyCode::Esc && matches!(app.route, Route::MainMenu) {
                                drop(shutdown_tx);
                                break;
                            }
                            
                            // 路由按键分发
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
                                // 🌟 Gameplay 需要传入 tx.clone() 以便 spawn 后台任务
                                Route::Gameplay(_) => app.handle_gameplay_input(key_event, narrator.clone(), tx.clone()).await,
                                
                                Route::Create(_) => app.handle_create_input(key_event, &narrator).await,
                                Route::Projects(_) => app.handle_projects_input(key_event),
                                Route::Settings(_) => app.handle_settings_input(key_event).await,
                                Route::Saves(_) => app.handle_saves_input(key_event, narrator.clone()).await,
                                Route::Help => {},
                                Route::Error(_) => {},
                            }
                            
                            // 非主菜单界面按 Esc 返回主菜单
                            if key_code == KeyCode::Esc && !matches!(app.route, Route::MainMenu) {
                                app.navigate_to(Route::MainMenu);
                            }
                        }
                    }
                },
            }
        }
        
        execute!(stdout(), crossterm::event::DisableMouseCapture).unwrap();
        
        // 清理终端
        ratatui::restore();
    }
    
    Ok(())
}