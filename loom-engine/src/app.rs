use create::CreateState;
use gameplay::GameplayState;
use ratatui::{self};
use ratatui::widgets::ListState;
use crate::config::GameMode;
use crate::llm::tool::builtin_tools::{reset_save_data, save_data};
use crate::save::SaveMeta;
use crate::project::{list_projects, Project};
use anyhow::{bail, Error, Result};

pub mod main_menu;
pub mod create;
pub mod projects;
pub mod gameplay;
pub mod error;
pub mod saves;

#[derive(Debug)]
pub enum Route {
    MainMenu,
    Projects(ListState),
    Settings,
    Help,
    Gameplay(GameplayState),
    Create(CreateState),
    Error(Error),
    Saves(ListState),
}
pub struct App {
    pub route: Route,
    projects: Vec<Result<Project>>,
    selected_project_id: Option<i64>,
    selected_project_index: usize,
    current_save_metas: Option<Vec<Result<SaveMeta>>>,
    selected_save_meta_id: Option<i64>,
    pub menu_state: ListState,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));
        let projects = list_projects()?;
        Ok(Self {
            route: Route::MainMenu,
            projects,
            selected_project_id: None,
            selected_project_index: 0,
            current_save_metas: None,
            selected_save_meta_id: None,
            menu_state,
        })
    }
    pub fn navigate_to(&mut self, route: Route) {
        // if let Route::Gameplay(GameplayState{ selected_save_data, .. }) = &route {
        //     if let Some(save_data) = selected_save_data {
        //         reset_save_data(save_data.game_mode.clone());
        //     }
        // }
        
        self.route = route;
    }
    
    pub fn select_save(&mut self, id: i64) -> Result<()> {
        if let Some(metas) = &self.current_save_metas {
            for item in metas {
                if let Ok(meta) = item && meta.timestamp == id {
                    self.selected_save_meta_id = Some(id);
                    return Ok(());
                }
            }
            bail!(format!("There is not a save with timestamp {}.", id))
        } else {
            bail!("Unselected save.")
        }
    }
    pub fn get_mut_save_meta(&mut self) -> Result<&mut SaveMeta> {
        if let Some(id) = self.selected_save_meta_id {
            if let Some(metas) = &mut self.current_save_metas {
                for item in metas {
                    if let Ok(meta) = item && meta.timestamp == id {
                        return Ok(meta);
                    }
                }
                bail!(format!("There is no save with timestamp of {} for the current project.", id))
            } else {
                bail!("Unselected project.")
            }
        } else {
            bail!("Unselected save.")
        }
    }
    pub fn get_save_metas(&self) -> Result<&Vec<Result<SaveMeta>>> {
        if let Some(metas) = &self.current_save_metas {
            Ok(metas)
        } else {
            bail!("Unselected project.")
        }
    }
}