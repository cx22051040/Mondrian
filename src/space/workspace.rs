use std::sync::atomic::{AtomicU32, Ordering};

use smithay::{backend::renderer::element::Id, desktop::{Space, Window}, output::Output, utils::{Logical, Point, Physical}};

use crate::layout::workspace::{LayoutScheme, WorkspaceLayout};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Debug)]
pub struct Workspace {
    pub id: u32,
    pub space: Space<Window>,
    pub layout: WorkspaceLayout,
    pub activate: bool,
}

impl Workspace {
    pub fn new(output: &Output, location: (i32, i32), layout: WorkspaceLayout, activate: bool) -> Self {
        let mut space: Space<Window> = Default::default();
        space.map_output(output, location);
        Self { 
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            space, 
            layout,
            activate,
        }
    }

    pub fn id(&self) -> &u32 {
        &self.id
    }

    pub fn space(&self) -> &Space<Window> {
        &self.space
    }

    pub fn map_element(&mut self, window: Window, location: Point<i32, Logical>, activate: bool) {
        self.space.map_element(window, location, activate);
    }

    pub fn set_active(&mut self, activate: bool) {
        self.activate = activate
    }
}


#[derive(Debug)]
pub struct WorkspaceManager {
    pub workspaces: Vec<Workspace>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: vec![],
        }
    }

    // TODO: allow more output binds
    pub fn add_workspace(&mut self, output: &Output, location: (i32, i32), layout: Option<WorkspaceLayout>, activate: bool) {
        self.workspaces.push(
            Workspace::new(output, location, layout.unwrap_or_else(WorkspaceLayout::default), activate)
        );
    }

    pub fn remove_workspace(&mut self) {
        todo!()
    }

    pub fn change_current_workspace() {
        todo!()
    }

    pub fn current_workspace(&self) -> &Workspace {
        self.workspaces.iter().find(|workspace| workspace.activate).expect("No active workspace")
    }

    pub fn map_element(&mut self, window: Window, location: Point<i32, Logical>, activate: bool) {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.activate)
            .expect("Workspace not found")
            .map_element(window, location, activate);
    }
    
    pub fn raise_element(&mut self, window: &Window, activate: bool) {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.activate)
            .expect("Workspace not found")
            .space
            .raise_element(window, activate);
    }

    pub fn refresh(&mut self) {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.activate)
            .expect("Workspace not found")
            .space
            .refresh()
    }

    pub fn current_space(&self) -> &Space<Window> {
        let workspace = self.current_workspace();
        &workspace.space()
    }

    pub fn active_workspaces() -> Vec<Workspace> {
        todo!()
    }

    pub fn workspaces_counts(&self) -> usize {
        self.workspaces.iter().count()
    }

    pub fn switch_workspace(&mut self, id: u32) {
        self.workspaces.iter_mut().for_each(|w| w.set_active(false));
        if let Some(w) = self.workspaces.iter_mut().find(|w| w.id == id) {
            w.set_active(true);
            tracing::info!("now workspace id: {:?} is active", w.id);
        }
    }

}
