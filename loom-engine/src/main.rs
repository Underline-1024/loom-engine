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
    if let Err(e) = std::fs::create_dir_all("logs") {
        panic!("❌ 无法创建 logs 目录: {}", e);
    }
    
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    LOG_GUARD.set(guard).ok();
    
    let file_log = fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_timer(fmt::time::ChronoLocal::rfc_3339())
        .with_ansi(false) 
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(file_log)
        .init();
    
    tracing::info!("✅ 日志系统初始化完成，日志保存在 logs/app.log");
}

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let key_tx = tx.clone();

    init_logging();
    init_save_data(GameMode::default());
    
    // 1. 正常创建和初始化 Narrator
    let narrator_instance = Narrator::new();
    let dynamic_tool_count = 5;
    let llm_config = LlmConfig::load()?;
    narrator_instance.init(&llm_config, dynamic_tool_count).await?;
    
    // 🌟 2. 将初始化好的 narrator 包装进 Arc 中
    let narrator = Arc::new(narrator_instance);

    std::thread::spawn(move || {
        loop {
            match event::read() {
                Ok(event) => {
                    let app_event = match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => Some(AppEvent::Key(key)),
                        Event::Resize(_, _) => Some(AppEvent::Resize),
                        _ => None,
                    };
                    
                    if let Some(evt) = app_event {
                        if key_tx.send(evt).is_err() {
                            break; 
                        }
                    }
                }
                Err(_) => break, // 读取发生致命错误，退出线程
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
                            app.handle_llm_response(&response).await;
                            
                            if let Route::Gameplay(state) = &mut app.route {
                                state.is_processing = false;
                                state.dialogue_scroll_offset = usize::MAX; 
                            }
                            
                            // 🧹 同样清理 AI 思考期间积压的按键
                            while let Ok(evt) = rx.try_recv() {
                                if matches!(evt, AppEvent::Resize) {
                                    let _ = tx.send(AppEvent::Resize);
                                    break;
                                }
                            }
                        },
                        
                        AppEvent::LlmError(err) => {
                            // 发生错误时，显示系统提示并关闭 Loading
                            app.add_dialogue("System", format!("LLM Error: {}", err));
                            if let Route::Gameplay(state) = &mut app.route {
                                state.is_processing = false;
                            }
                        },
                        AppEvent::ProjectCreated(result) => {
                            // 确保当前还在 Create 页面
                            if let Route::Create(state) = &mut app.route {
                                state.is_processing = false; // 🌟 恢复状态
                                
                                match result {
                                    Ok(project) => {
                                        // 成功：加入列表并跳转到 Projects 页面
                                        app.projects.push(Ok(project));
                                        state.error_msg = None;
                                        let mut list_state = ListState::default();
                                        list_state.select(Some(0));
                                        app.route = Route::Projects(list_state);
                                    }
                                    Err(e) => {
                                        // 失败：显示错误信息，留在当前页面
                                        state.error_msg = Some(e);
                                    }
                                }
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
                                Route::Gameplay(_) => app.handle_gameplay_input(key_event, narrator.clone(), tx.clone()).await,
                                Route::Create(_) => app.handle_create_input(key_event, narrator.clone(), tx.clone()).await,
                                Route::Projects(_) => app.handle_projects_input(key_event),
                                Route::Settings(_) => app.handle_settings_input(key_event).await,
                                Route::Saves(_) => app.handle_saves_input(key_event, narrator.clone(), tx.clone()).await,
                                Route::Help => {},
                                Route::Error(_) => {},
                            }
                            
                            // 非主菜单界面按 Esc 返回主菜单
                            if key_code == KeyCode::Esc && !matches!(app.route, Route::MainMenu) {
                                // 🔒 检查当前页面是否正在执行耗时操作，如果是，则屏蔽全局 Esc 返回
                                let is_processing = match &app.route {
                                    Route::Saves(state) => state.is_processing,
                                    Route::Create(state) => state.is_processing,
                                    Route::Gameplay(state) => state.is_processing,
                                    _ => false,
                                };
                            
                                if !is_processing {
                                    app.navigate_to(Route::MainMenu);
                                }
                            }
                        },
                        AppEvent::SaveOperationCompleted => {
                            app.on_save_operation_completed();
                            
                            // 🧹 清理积压的键盘事件（防止 Loading 期间的幽灵按键和秒退）
                            while let Ok(evt) = rx.try_recv() {
                                if matches!(evt, AppEvent::Resize) {
                                    // 如果是窗口大小改变事件，保留它（重新发回通道）
                                    let _ = tx.send(AppEvent::Resize);
                                    break; 
                                }
                                // 其他事件（主要是 Key）直接丢弃，不做任何处理
                            }
                        },
                    }
                },
            }
        }
        
        execute!(stdout(), crossterm::event::DisableMouseCapture).unwrap();
        ratatui::restore();
    }
    
    Ok(())
}