use std::{
    collections::HashMap, hash::Hash, sync::{
        atomic::{AtomicUsize, Ordering}, Arc
    }, time::Duration
};

use smithay::{
    desktop::{Space, Window, WindowSurfaceType},
    output::Output,
    reexports::{
        calloop::LoopHandle, wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle},
};

use crate::{
    config::workspace::WorkspaceConfigs,
    layout::{
        Direction,
        tiled_tree::{TiledScheme, TiledTree},
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

    tiled: Space<Window>,
    // pub floating: Space<Window>,
    // pub layout: HashMap<Window, WindowLayout>,
    scheme: TiledScheme,
    tiled_tree: Option<TiledTree>,
    focus: Option<Window>,
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
        let mut floating: Space<Window> = Default::default();
        tiled.map_output(output, output_geometry.loc);
        floating.map_output(output, output_geometry.loc);

        Self {
            workspace_id,
            tiled,
            // floating,
            // layout: HashMap::new(),
            scheme,
            tiled_tree: None,
            focus: None,
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

    pub fn focus(&self) -> Option<&Window> {
        self.focus.as_ref()
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> {
        self.tiled.elements()
    }

    pub fn window_geometry(&self, window: &Window) -> Option<Rectangle<i32, Logical>> {
        self.tiled.element_geometry(window)
    }

    pub fn is_empty(&self) -> bool {
        self.tiled_tree.is_none()
    }

    pub fn _clear(&mut self) {
        // Clear the workspace, remove all elements, send to else workspace
    }

    pub fn map_element(
        &mut self,
        window: Window,
        edges: ResizeEdge,
        activate: bool,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.refresh();

        window
            .toplevel()
            .unwrap()
            .with_pending_state(|state| state.bounds = Some(self.output_working_geometry.size));
        window.toplevel().unwrap().send_pending_configure();

        if self.tiled_tree.is_none() {
            let rec = Rectangle {
                loc: (
                    self.output_working_geometry.loc.x + self.configs.gap, 
                    self.output_working_geometry.loc.y + self.configs.gap
                ).into(),
                size: (self.output_working_geometry.size
                    - (self.configs.gap * 2, self.configs.gap * 2).into())
                .into(),
            };

            window.set_rec(rec.size);
            self.tiled.map_element(window.clone(), rec.loc, activate);
            self.tiled_tree = Some(TiledTree::new(window.clone(), self.configs.gap));

            // set focus
            if activate {
                self.focus = Some(window.clone());
            }

            loop_handle.insert_idle(move |data| {
                let mut from = rec;
                from.loc.y += from.size.h;
                data.render_manager.add_animation(
                    window,
                    from,
                    rec,
                    Duration::from_millis(30),
                    crate::animation::AnimationType::EaseInOutQuad,
                );
            });

            return;
        }

        match self.scheme {
            TiledScheme::Default => {
                if let Some(layout_tree) = &mut self.tiled_tree {
                    // TODO
                    let focus_rec = self
                        .tiled
                        .element_geometry(self.focus.as_ref().unwrap())
                        .unwrap();
                    let direction = if focus_rec.size.w > focus_rec.size.h {
                        match edges {
                            ResizeEdge::TopLeft | ResizeEdge::BottomLeft => Direction::Left,
                            ResizeEdge::TopRight | ResizeEdge::BottomRight => Direction::Right,
                            _ => Direction::default(),
                        }
                    } else {
                        match edges {
                            ResizeEdge::TopLeft | ResizeEdge::TopRight => Direction::Up,
                            ResizeEdge::BottomLeft | ResizeEdge::BottomRight => Direction::Down,
                            _ => Direction::default(),
                        }
                    };
                    layout_tree.insert_window(
                        self.focus.as_ref(),
                        window.clone(),
                        direction,
                        &mut self.tiled,
                        loop_handle,
                    );

                    #[cfg(feature = "trace_layout")]
                    layout_tree.print_tree();
                }
            }
            TiledScheme::Spiral => {
                if let Some(layout_tree) = &mut self.tiled_tree {
                    layout_tree.insert_window_spiral(window.clone(), &mut self.tiled, loop_handle);

                    #[cfg(feature = "trace_layout")]
                    layout_tree.print_tree();
                }
            }
        }

        // set focus
        if activate {
            self.focus = Some(window);
        }
    }

    pub fn unmap_element(&mut self, window: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(tiled_tree) = &mut self.tiled_tree {
            tiled_tree.remove(window, &mut self.focus, &mut self.tiled, loop_handle);

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

        self.tiled.unmap_elem(window);
    }

    pub fn invert_window(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            if let Some(focus) = &self.focus {
                layout_tree.invert_window(focus, &mut self.tiled, loop_handle);

                #[cfg(feature = "trace_layout")]
                layout_tree.print_tree();
            }
        }
    }

    pub fn exchange_window(
        &mut self,
        direction: &Direction,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            if let Some(focus) = &self.focus {
                layout_tree.exchange(focus, direction, &mut self.tiled, loop_handle);
            }
        }
    }

    pub fn tiled_expansion(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(layout_tree) = &self.tiled_tree {
            layout_tree.expansion(&mut self.tiled, loop_handle);
        }
    }

    pub fn tiled_recover(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            layout_tree.recover(&mut self.tiled, loop_handle);
        }
    }

    // pub fn _resize(&mut self, offset: Point<i32, Logical>, edges: &ResizeEdge, rec: &mut Rectangle<i32, Logical>) {
    // }

    pub fn update_output_geo(
        &mut self,
        rec: Rectangle<i32, Logical>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        if self.output_working_geometry == rec {
            return;
        }

        self.output_working_geometry = rec;
        if let Some(layout_tree) = &mut self.tiled_tree {
            let root_id = layout_tree.get_root().unwrap();
            layout_tree.modify(
                root_id,
                Rectangle::new(
                    (rec.loc.x + self.configs.gap, rec.loc.y + self.configs.gap).into(),
                    (rec.size - (self.configs.gap * 2, self.configs.gap * 2).into()).into(),
                ),
                &mut self.tiled,
                loop_handle,
            );
        }
    }

    pub fn set_focus(&mut self, window: Option<Window>) {
        match window {
            Some(window) => {
                self.raise_element(&window, true);
                self.focus = Some(window);
            }
            None => {
                if let Some(focus) = &self.focus {
                    focus.set_activated(false);
                    focus.toplevel().unwrap().send_pending_configure();
                }
                self.focus = None;
            }
        }
    }

    pub fn window_under(
        &self,
        position: Point<f64, Logical>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        self.tiled.element_under(position)
    }

    pub fn surface_under(
        &mut self,
        position: Point<f64, Logical>,
        need_focus: bool,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        if let Some((window, window_loc)) = self.window_under(position).map(|(w, p)| (w.clone(), p))
        {
            if let Some((surface, surface_loc)) = window
                .surface_under(position - window_loc.to_f64(), WindowSurfaceType::ALL)
                .map(|(surface, surface_loc)| (surface, surface_loc))
            {
                if need_focus {
                    self.set_focus(Some(window));
                }

                return Some((surface, (surface_loc + window_loc).to_f64()));
            }
        }
        None
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&Window> {
        self.elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == surface)
    }

    pub fn check_grab(
        &mut self,
        surface: &WlSurface,
    ) -> Option<(&Window, Rectangle<i32, Logical>)> {
        // TODO: check window's lock state
        let window = self.find_window(surface)?;

        let rec = self.window_geometry(window).or_else(|| {
            warn!("Failed to get window's geometry");
            None
        })?;

        Some((window, rec))
    }

    fn refresh(&mut self) {
        self.tiled.refresh();
        // self.floating.refresh();
    }

    fn deactivate(&mut self) {
        for window in self.tiled.elements() {
            window.set_activated(false);
            window.toplevel().unwrap().send_pending_configure();
        }
    }

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
    
                        let mut from = data.workspace_manager.window_geometry(window).unwrap();
                        from.loc.x = if workspace_id.0 > id.0 {
                            from.loc.x + width
                        } else {
                            from.loc.x - width
                        };
    
                        let to = data.workspace_manager.window_geometry(window).unwrap();
    
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

    pub fn window_geometry(&self, window: &Window) -> Option<Rectangle<i32, Logical>> {
        self.current_workspace().window_geometry(window)
    }

    pub fn map_element(
        &mut self,
        window: Window,
        edges: ResizeEdge,
        activate: bool,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.current_workspace_mut()
            .map_element(window, edges, activate, loop_handle);
    }

    pub fn unmap_element(&mut self, window: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut()
            .unmap_element(window, loop_handle);
    }

    pub fn invert_window(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().invert_window(loop_handle);
    }

    pub fn exchange_window(
        &mut self,
        direction: &Direction,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.current_workspace_mut()
            .exchange_window(direction, loop_handle);
    }

    pub fn tiled_expansion(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().tiled_expansion(loop_handle);
    }

    pub fn tiled_recover(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().tiled_recover(loop_handle);
    }

    // pub fn resize(&mut self, offset: Point<i32, Logical>, edges: &ResizeEdge, rec: &mut Rectangle<i32, Logical>) {
    //     self.current_workspace_mut().resize(offset, edges, rec);
    // }

    pub fn update_output_geo(
        &mut self,
        rec: Rectangle<i32, Logical>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.current_workspace_mut()
            .update_output_geo(rec, loop_handle);
    }

    pub fn surface_under(
        &mut self,
        position: Point<f64, Logical>,
        need_focus: bool,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.current_workspace_mut()
            .surface_under(position, need_focus)
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&Window> {
        // TODO: maybe can use hashmap to store the surface
        self.current_workspace().find_window(surface)
    }

    pub fn check_grab(
        &mut self,
        surface: &WlSurface,
    ) -> Option<(&Window, Rectangle<i32, Logical>)> {
        self.current_workspace_mut().check_grab(surface)
    }
}
