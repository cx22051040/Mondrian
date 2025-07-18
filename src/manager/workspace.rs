use std::{
    collections::HashMap, hash::Hash, sync::{
        atomic::{AtomicUsize, Ordering}, Arc
    }, time::Duration
};

use smithay::{
    desktop::{Space, Window, WindowSurface},
    output::Output,
    reexports::calloop::LoopHandle,
    utils::{Logical, Point, Rectangle},
};

use crate::{
    config::workspace::WorkspaceConfigs,
    layout::{
        container_tree::ContainerTree, Direction, ResizeEdge, TiledScheme
    },
    state::GlobalData,
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

    tiled: Space<Window>, // TODO: maybe ...
    // pub floating: Space<Window>,
    // pub layout: HashMap<Window, WindowLayout>,
    scheme: TiledScheme, 
    tiled_tree: Option<ContainerTree>,

    output_working_geometry: Rectangle<i32, Logical>,

    configs: Arc<WorkspaceConfigs>,
}

impl Workspace {
    pub fn new(
        workspace_id: WorkspaceId,
        output: &Output,
        output_geometry: Rectangle<i32, Logical>,
        scheme: TiledScheme,
        configs: Arc<WorkspaceConfigs>,
    ) -> Self {
        let mut tiled: Space<Window> = Default::default();
        // let mut floating: Space<Window> = Default::default();
        tiled.map_output(output, output_geometry.loc);
        // floating.map_output(output, output_geometry.loc);

        Self {
            workspace_id,
            tiled,
            // floating,
            // layout: HashMap::new(),
            scheme,
            tiled_tree: None,
            output_working_geometry: output_geometry,

            configs,
        }
    }

    pub fn id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub fn current_space(&self) -> &Space<Window> {
        &self.tiled
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> {
        self.tiled.elements()
    }

    pub fn is_empty(&self) -> bool {
        self.tiled_tree.is_none()
    }

    pub fn _clear(&mut self) {
        // Clear the workspace, remove all elements, send to else workspace
    }

    pub fn map_window(
        &mut self,
        target: Option<&Window>,
        window: Window,
        edge: ResizeEdge,
        activate: bool,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.refresh();

        match window.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                toplevel.with_pending_state(|state| state.bounds = Some(self.output_working_geometry.size));
                toplevel.send_pending_configure();
            },
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(_) => { }
        };

        if self.tiled_tree.is_none() {

            let gap = self.configs.gap;
            let root_rec = Rectangle {
                loc: (
                    self.output_working_geometry.loc.x + gap, 
                    self.output_working_geometry.loc.y + gap
                ).into(),
                size: (
                    self.output_working_geometry.size
                    - (gap * 2, gap * 2).into()
                ).into(),
            };

            self.tiled_tree = Some(
                ContainerTree::new_with_first_node(
                    window.clone(), 
                    root_rec, 
                    self.configs.gap, 
                ));
            
            self.tiled.map_element(window.clone(), root_rec.loc, activate);

            // add animation
            loop_handle.insert_idle(move |data| {
                let mut from = root_rec;
                from.loc.y += from.size.h;
                data.render_manager.add_animation(
                    window,
                    from,
                    root_rec,
                    Duration::from_millis(30),
                    crate::animation::AnimationType::OvershootBounce,
                );
            });

            return;
        }

        match self.scheme {
            TiledScheme::Default => {
                if let Some(layout_tree) = &mut self.tiled_tree {
                    // default: must have target if layout tree is some
                    let target = target.unwrap();                    

                    let target_rec = target.get_rect();

                    let (direction, is_favour) = edge.to_direction_and_favour(target_rec);

                    layout_tree.insert(
                        target,
                        direction,
                        window.clone(),
                        is_favour,

                        loop_handle,
                    );

                    #[cfg(feature = "trace_layout")]
                    layout_tree.print_tree();
                }
            }
            TiledScheme::Scroll => {
                if let Some(_) = &mut self.tiled_tree {



                    // #[cfg(feature = "trace_layout")]
                    // layout_tree.print_tree();
                }
            }
        }

        // the location info was stored at window.get_rect()
        self.tiled.map_element(window.clone(), (0, 0), activate);
    }

    pub fn unmap_window(&mut self, target: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(tiled_tree) = &mut self.tiled_tree {
            tiled_tree.remove(target, loop_handle);
            self.tiled.unmap_elem(target);

            if tiled_tree.is_empty() {
                self.tiled_tree = None;
            } else {
                #[cfg(feature = "trace_layout")]
                tiled_tree.print_tree();
            }
        } else {
            error!("empty layout tree!");
            return;
        }
    }

    pub fn invert_window(&mut self, target: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            layout_tree.invert(target, loop_handle);

            #[cfg(feature = "trace_layout")]
            layout_tree.print_tree();
        }
    }

    pub fn exchange_window(
        &mut self,
        target: &Window,
        edge: &ResizeEdge,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            // input edge is single turn
            let (direction, is_favour) = edge.to_direction_and_favour(Rectangle::default());

            layout_tree.exchange(target, direction, is_favour, loop_handle);

            #[cfg(feature = "trace_layout")]
            layout_tree.print_tree();
        }
    }

    pub fn tiled_expansion(&self, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(layout_tree) = &self.tiled_tree {
            let rect = self.output_working_geometry;
            
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

            layout_tree.expansion(root_rect, loop_handle);
        }
    }

    pub fn tiled_recover(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            let rect = self.output_working_geometry;
            
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

            layout_tree.update_root_rect_recursive(root_rect, loop_handle);
        }
    }

    pub fn resize(&mut self, target: &Window, edge: &ResizeEdge, offset: Point<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(tiled_tree) = &mut self.tiled_tree {
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

                tiled_tree.resize(target, direction, offset, is_favour, loop_handle);
            }
        } 
    }

    pub fn update_output_rect(
        &mut self,
        rect: Rectangle<i32, Logical>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        if self.output_working_geometry == rect {
            return;
        }

        self.output_working_geometry = rect;
        if let Some(layout_tree) = &mut self.tiled_tree {

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

            layout_tree.update_root_rect_recursive(root_rect, loop_handle);
        }
    }

    pub fn window_under(
        &mut self,
        position: Point<f64, Logical>,
    ) -> Option<Window> {

        for window in self.elements() {
            let window_rect = window.get_rect();

            if window_rect.contains(position.to_i32_round()) {
                return Some(window.clone())
            }
        }

        None
    }

    fn refresh(&mut self) {
        self.tiled.refresh();
        // self.floating.refresh();
    }

    fn deactivate(&mut self) {
        for window in self.tiled.elements() {
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

    #[allow(dead_code)]
    fn raise_element(&mut self, window: &Window, activate: bool) {
        self.tiled.raise_element(window, activate)
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
        output: &Output,
        output_geometry: Rectangle<i32, Logical>,
        scheme: Option<TiledScheme>,
        activate: bool,
    ) {
        let workspace = Workspace::new(
            workspace_id,
            output,
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

    pub fn switch_workspace(&mut self, workspace_id: WorkspaceId, output: &Output, output_geometry: Rectangle<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if !self.workspaces.contains_key(&workspace_id) {
            self.add_workspace(
                workspace_id,
                output, 
                output_geometry, 
                None, 
                true
            );
        } else if let Some(id) = self.activated_workspace {
            if id != workspace_id {
                self.current_workspace_mut().deactivate();
                self.activated_workspace = Some(workspace_id);

                loop_handle.insert_idle(move |data| {
                    for window in data.workspace_manager.current_workspace().elements() {
                        let width = data.workspace_manager.current_workspace().output_working_geometry.size.w;

                        let to = window.get_rect();
                        let mut from = to.clone();
                        from.loc.x = if workspace_id.0 > id.0 {
                            from.loc.x + width
                        } else {
                            from.loc.x - width
                        };
    
                        data.render_manager.add_animation(
                            window.clone(), 
                            from, 
                            to, 
                            Duration::from_millis(30), 
                            crate::animation::AnimationType::EaseInOutQuad,
                        );
                    }
                });
            }
        } else {
            self.activated_workspace = Some(workspace_id);
        }

        self.refresh();
    }

    pub fn remove_workspace(&mut self, workspace_id: WorkspaceId) {
        if self.workspaces.iter().count() <= 1 {
            warn!("Cannot remove the last workspace: {:?}", workspace_id);
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
            if self.activated_workspace == Some(workspace.id()) {
                workspace.refresh();
            } else if workspace.is_empty() {
                to_remove.push(workspace.id());
            }
        }
        
        for id in to_remove {
            self.remove_workspace(id);
        }
    }

    pub fn current_space(&self) -> &Space<Window> {
        self.current_workspace().current_space()
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

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> {
        self.current_workspace().elements()
    }

    pub fn map_window(
        &mut self,
        target: Option<&Window>,
        window: Window,
        edge: ResizeEdge,
        activate: bool,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.current_workspace_mut()
            .map_window(target, window, edge, activate, loop_handle);
    }

    pub fn unmap_window(&mut self, target: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut()
            .unmap_window(target, loop_handle);
    }

    pub fn invert_window(&mut self, target: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().invert_window(target, loop_handle);
    }

    pub fn exchange_window(
        &mut self,
        target: &Window,
        edge: &ResizeEdge,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.current_workspace_mut()
            .exchange_window(target, edge, loop_handle);
    }

    pub fn tiled_expansion(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().tiled_expansion(loop_handle);
    }

    pub fn tiled_recover(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().tiled_recover(loop_handle);
    }

    pub fn resize(&mut self, target: &Window, edge: &ResizeEdge, offset: Point<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().resize(target, edge, offset, loop_handle);
    }

    pub fn update_output_rect(
        &mut self,
        rec: Rectangle<i32, Logical>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.current_workspace_mut()
            .update_output_rect(rec, loop_handle);
    }

    pub fn window_under(
        &mut self,
        position: Point<f64, Logical>,
    ) -> Option<Window> {
        self.current_workspace_mut()
            .window_under(position)
    }
}