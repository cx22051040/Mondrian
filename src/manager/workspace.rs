use std::{
    collections::HashMap, hash::Hash, sync::{
        atomic::{AtomicUsize, Ordering}, Arc
    }, time::Duration
};

use smithay::{
    desktop::{Window, WindowSurface},
    utils::{Logical, Point, Rectangle},
};

use crate::{
    config::workspace::WorkspaceConfigs, layout::{
        container_tree::ContainerTree, Direction, ResizeEdge, TiledScheme
    }, 
    manager::animation::{AnimationManager, AnimationType}
};

use super::window::WindowExt;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(usize);

impl WorkspaceId {
    // only for test
    pub fn new(id: usize) -> Self {
        Self(id)
    }
    #[inline]
    pub fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug)]
pub struct Workspace {
    workspace_id: WorkspaceId,

    scheme: TiledScheme, 
    container_tree: ContainerTree,

    output_working_geometry: Rectangle<i32, Logical>,

    configs: Arc<WorkspaceConfigs>,
}

impl Workspace {
    pub fn new(
        workspace_id: WorkspaceId,
        output_geometry: Rectangle<i32, Logical>,
        scheme: TiledScheme,
        configs: Arc<WorkspaceConfigs>,
    ) -> Self {

        let gap = configs.gap;
        let root_rect = Rectangle {
            loc: (
                output_geometry.loc.x + gap, 
                output_geometry.loc.y + gap
            ).into(),
            size: (
                output_geometry.size
                - (gap * 2, gap * 2).into()
            ).into(),
        };

        Self {
            workspace_id,
            scheme,
            container_tree: ContainerTree::new(root_rect, gap),
            output_working_geometry: output_geometry,

            configs,
        }
    }

    pub fn id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.container_tree.windows()
    }

    pub fn is_empty(&self) -> bool {
        self.container_tree.is_empty()
    }

    pub fn map_window(
        &mut self,
        target: Option<&Window>,
        window: Window,
        edge: ResizeEdge, 
        animation_manager: &mut AnimationManager,
    ) -> bool {
        self.container_tree.insert(target, window, edge, &self.scheme, animation_manager)
    }

    pub fn unmap_window(&mut self, target: &Window, animation_manager: &mut AnimationManager) {
        self.container_tree.remove(target, animation_manager);
    }

    pub fn invert_window(&mut self, target: &Window, animation_manager: &mut AnimationManager) {
        self.container_tree.invert(target, animation_manager);
    }

    pub fn exchange_window(
        &mut self,
        target: &Window,
        edge: &ResizeEdge,
        animation_manager: &mut AnimationManager,
    ) {
        // input edge is single turn
        let (direction, is_favour) = edge.to_direction_and_favour(Rectangle::default());

        self.container_tree.exchange(target, direction, is_favour, animation_manager);
    }

    pub fn expansion(&self, animation_manager: &mut AnimationManager) {
        self.container_tree.expansion(animation_manager);
    }

    pub fn recover(&mut self, animation_manager: &mut AnimationManager) {
        self.container_tree.recover(animation_manager);
    }

    pub fn grab_move(&mut self, target: &Window, offset: Point<i32, Logical>, animation_manager: &mut AnimationManager) {
        self.container_tree.grab_move(target, offset, animation_manager);
    }

    pub fn resize(&mut self, target: &Window, edge: &ResizeEdge, offset: Point<i32, Logical>) {
        for edge in edge.split() {
            let (direction, is_favour) = edge.to_direction_and_favour(Rectangle::default());

            let offset = match direction {
                Direction::Horizontal => {
                    offset.x
                },
                Direction::Vertical => {
                    offset.y
                }
            };

            self.container_tree.resize(target, direction, offset, is_favour);
        }
    }

    pub fn update_output_rect(
        &mut self,
        rect: Rectangle<i32, Logical>,
        animation_manager: &mut AnimationManager,
    ) {
        if self.output_working_geometry == rect {
            return;
        }

        self.output_working_geometry = rect;

        let gap = self.configs.gap;
        let root_rect = Rectangle {
            loc: (
                rect.loc.x + gap, 
                rect.loc.y + gap
            ).into(),
            size: (
                rect.size
                - (gap * 2, gap * 2).into()
            ).into(),
        };

        self.container_tree.update_root_rect(root_rect, animation_manager);
    }

    fn deactivate(&mut self) {
        for window in self.windows() {
            window.set_activated(false);

            match window.underlying_surface() {
                WindowSurface::Wayland(toplevel) => {
                    toplevel.send_pending_configure();
                },
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(_) => { }
            };
        }
    }
}

#[derive(Debug)]
pub struct WorkspaceManager {
    workspaces: HashMap<WorkspaceId, Workspace>,
    activated_workspace: Option<WorkspaceId>,
    configs: Arc<WorkspaceConfigs>,
}

impl WorkspaceManager {
    pub fn new(configs: Arc<WorkspaceConfigs>) -> Self {
        Self {
            workspaces: HashMap::new(),
            activated_workspace: None,
            configs,
        }
    }

    pub fn add_workspace(
        &mut self,
        workspace_id: WorkspaceId,
        output_geometry: Rectangle<i32, Logical>,
        scheme: Option<TiledScheme>,
        activate: bool,
    ) {
        let workspace = Workspace::new(
            workspace_id,
            output_geometry,
            scheme.unwrap_or_else(|| self.configs.scheme.clone()),
            self.configs.clone(),
        );

        self.set_activate(workspace.id(), activate);

        self.workspaces.insert(workspace.id(), workspace);

        self.refresh();
    }

    pub fn set_activate(&mut self, workspace_id: WorkspaceId, _activate: bool) {
        if let Some(id) = self.activated_workspace {
            if id != workspace_id {
                self.current_workspace_mut().deactivate();
                self.activated_workspace = Some(workspace_id);
            }
        } else {
            self.activated_workspace = Some(workspace_id);
        }
    }

    pub fn switch_workspace(&mut self, workspace_id: WorkspaceId, output_geometry: Rectangle<i32, Logical>, animation_manager: &mut AnimationManager) {
        if !self.workspaces.contains_key(&workspace_id) {
            self.add_workspace(
                workspace_id,
                output_geometry, 
                None, 
                true
            );
        } else if let Some(id) = self.activated_workspace {
            if id != workspace_id {
                self.current_workspace_mut().deactivate();
                self.activated_workspace = Some(workspace_id);

                // add animation
                for window in self.current_workspace().windows() {
                    let width = self.current_workspace().output_working_geometry.size.w;

                    let to = window.get_rect().unwrap();
                    let mut from = to.clone();
                    from.loc.x = if workspace_id.0 > id.0 {
                        from.loc.x + width
                    } else {
                        from.loc.x - width
                    };
    
                    animation_manager.add_animation(
                        window.clone(), 
                        from, 
                        to, 
                        Duration::from_millis(30), 
                        AnimationType::EaseInOutQuad,
                    );
                }
            }
        } else {
            self.activated_workspace = Some(workspace_id);
        }

        self.refresh();
    }

    pub fn remove_workspace(&mut self, workspace_id: WorkspaceId) {
        if self.workspaces.iter().count() <= 1 {
            return;
        }
        
        self.workspaces.remove(&workspace_id);

        if self.activated_workspace == Some(workspace_id) {
            self.activated_workspace = Some(WorkspaceId(1));
        }
    }

    pub fn refresh(&mut self) {
        let mut to_remove = vec![];

        for workspace in self.workspaces.values_mut() {
            if self.activated_workspace != Some(workspace.id()) && workspace.is_empty() {
                to_remove.push(workspace.id());
            }
        }
        
        for id in to_remove {
            self.remove_workspace(id);
        }
    }

    pub fn current_workspace(&self) -> &Workspace {
        self.activated_workspace
            .and_then(|id| self.workspaces.get(&id))
            .expect("no current_workspace")
    }

    pub fn current_workspace_mut(&mut self) -> &mut Workspace {
        self.activated_workspace
            .and_then(|id| self.workspaces.get_mut(&id))
            .expect("no current_workspace")
    }

    pub fn _workspaces_counts(&self) -> usize {
        self.workspaces.iter().count()
    }

    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.current_workspace().windows()
    }

    pub fn map_window(
        &mut self,
        target: Option<&Window>,
        window: Window,
        edge: ResizeEdge,
        animation_manager: &mut AnimationManager,
    ) -> bool {
        self.current_workspace_mut()
            .map_window(target, window, edge, animation_manager)
    }

    pub fn unmap_window(&mut self, target: &Window, animation_manager: &mut AnimationManager) {
        self.current_workspace_mut()
            .unmap_window(target, animation_manager);
    }

    pub fn invert_window(&mut self, target: &Window, animation_manager: &mut AnimationManager) {
        self.current_workspace_mut().invert_window(target, animation_manager);
    }

    pub fn exchange_window(
        &mut self,
        target: &Window,
        edge: &ResizeEdge,
        animation_manager: &mut AnimationManager,
    ) {
        self.current_workspace_mut()
            .exchange_window(target, edge, animation_manager);
    }

    pub fn tiled_expansion(&mut self, animation_manager: &mut AnimationManager) {
        self.current_workspace_mut().expansion(animation_manager);
    }

    pub fn tiled_recover(&mut self, animation_manager: &mut AnimationManager) {
        self.current_workspace_mut().recover(animation_manager);
    }

    pub fn grab_move(&mut self, target: &Window, offset: Point<i32, Logical>, animation_manager: &mut AnimationManager) {
        self.current_workspace_mut().grab_move(target, offset, animation_manager);
    }

    pub fn resize(&mut self, target: &Window, edge: &ResizeEdge, offset: Point<i32, Logical>) {
        self.current_workspace_mut().resize(target, edge, offset);
    }

    pub fn update_output_rect(
        &mut self,
        rec: Rectangle<i32, Logical>,
        animation_manager: &mut AnimationManager,
    ) {
        self.current_workspace_mut()
            .update_output_rect(rec, animation_manager);
    }
}