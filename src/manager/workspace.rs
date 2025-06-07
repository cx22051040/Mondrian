use std::{hash::Hash, sync::atomic::{AtomicUsize, Ordering}, time::Duration};

use smithay::{
    desktop::{Space, Window},
    output::Output,
    reexports::{calloop::LoopHandle, wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge, wayland_server::protocol::wl_surface::WlSurface},
    utils::{Logical, Point, Rectangle},
};

use crate::{layout::{tiled_tree::{TiledScheme, TiledTree}, Direction}, state::GlobalData};

use super::window::WindowExt;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
const GAP: i32 = 12;

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
    pub id: WorkspaceId,

    pub tiled: Space<Window>,
    // pub floating: Space<Window>,
    // pub layout: HashMap<Window, WindowLayout>,

    pub scheme: TiledScheme,
    pub tiled_tree: Option<TiledTree>,
    pub focus: Option<Window>,
    pub output_geometry: Rectangle<i32, Logical>,
}

impl Workspace {
    pub fn new(
        output: &Output,
        output_geometry: Rectangle<i32, Logical>,
        scheme: TiledScheme,
    ) -> Self {
        let mut tiled: Space<Window> = Default::default();
        let mut floating: Space<Window> = Default::default();
        tiled.map_output(output, output_geometry.loc);
        floating.map_output(output, output_geometry.loc);

        Self {
            id: WorkspaceId::next(),
            tiled,
            // floating,
            // layout: HashMap::new(),

            scheme,
            tiled_tree: None,
            focus: None,
            output_geometry,
        }
    }

    pub fn id(&self) -> WorkspaceId {
        self.id
    }

    pub fn map_element(
        &mut self,
        window: Window,
        edges: ResizeEdge,
        activate: bool,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        self.refresh();

        window.toplevel().unwrap().with_pending_state(|state| {
            state.bounds = Some(self.output_geometry.size)
        });
        window.toplevel().unwrap().send_pending_configure();

        if self.tiled_tree.is_none() {
            let rec = Rectangle {
                loc: (GAP, GAP).into(),
                size: (self.output_geometry.size - (GAP * 2, GAP * 2).into()).into()
            };

            window.set_rec(rec.size);
            self.tiled.map_element(window.clone(), rec.loc, activate);
            self.tiled_tree = Some(TiledTree::new(window.clone()));

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
                    let focus_rec = self.tiled.element_geometry(self.focus.as_ref().unwrap()).unwrap();
                    let direction = if focus_rec.size.w > focus_rec.size.h {
                        match edges {
                            ResizeEdge::TopLeft | ResizeEdge::BottomLeft => {
                                Direction::Left
                            }
                            ResizeEdge::TopRight | ResizeEdge::BottomRight => {
                                Direction::Right
                            }
                            _ => { Direction::default() }
                        }
                    } else {
                        match edges {
                            ResizeEdge::TopLeft | ResizeEdge::TopRight => {
                                Direction::Up
                            }
                            ResizeEdge::BottomLeft | ResizeEdge::BottomRight => {
                                Direction::Down
                            }
                            _ => { Direction::default() }
                        }
                    };
                    layout_tree.insert_window(self.focus.as_ref(), window.clone(), direction, &mut self.tiled, loop_handle);

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

    pub fn refresh(&mut self) {
        self.tiled.refresh();
        // self.floating.refresh();
    }

    pub fn window_under(
        &self,
        position: Point<f64, Logical>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        self.tiled.element_under(position)
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> {
        self.tiled.elements()
    }

    pub fn raise_element(&mut self, window: &Window, activate: bool) {
        self.tiled.raise_element(window, activate)
    }

    pub fn deactivate(&mut self) {
        for window in self.tiled.elements() {
            window.set_activated(false);
            window.toplevel().unwrap().send_pending_configure();
        }
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

    pub fn modify_windows(&mut self, rec: Rectangle<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.output_geometry = rec;
        if let Some(layout_tree) = &mut self.tiled_tree {
            let root_id = layout_tree.get_root().unwrap();
            layout_tree.modify(
                root_id,
                Rectangle::new(
                    (GAP, GAP).into(),
                    (rec.size - (GAP * 2, GAP * 2).into()).into(),
                ),
                &mut self.tiled,
                loop_handle,
            );
        }
    }

    // pub fn _resize(&mut self, offset: Point<i32, Logical>, edges: &ResizeEdge, rec: &mut Rectangle<i32, Logical>) {
    // }

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

    pub fn get_focus(&self) -> &Option<Window> {
        &self.focus
    }

    pub fn current_space(&self) -> &Space<Window> {
        &self.tiled
    }

    pub fn window_geometry(&self, window: &Window) -> Option<Rectangle<i32, Logical>> {
        self.tiled.element_geometry(window)
    }

    pub fn find_window(&self, surface: &WlSurface) ->Option<&Window> {
        self.elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == surface)
    }

    pub fn check_grab(&mut self, surface: &WlSurface) -> Option<(&Window, Rectangle<i32, Logical>)> {
        // TODO: check window's lock state
        let window = self.find_window(surface)?;

        let rec = self.window_geometry(window).or_else(|| {
            warn!("Failed to get window's geometry");
            None
        })?;
    
        Some((window, rec))
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
}

#[derive(Debug)]
pub struct WorkspaceManager {
    pub workspaces: Vec<Workspace>,
    pub activated_workspace: Option<WorkspaceId>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: vec![],
            activated_workspace: None,
        }
    }

    // TODO: allow more output binds
    pub fn add_workspace(
        &mut self,
        output: &Output,
        output_geometry: Rectangle<i32, Logical>,
        scheme: Option<TiledScheme>,
        activate: bool,
    ) {
        let workspace = Workspace::new(
            output,
            output_geometry,
            scheme.unwrap_or_else(|| TiledScheme::Default),
        );

        if activate {
            self.set_activated(workspace.id());
        }

        self.workspaces.push(workspace);
    }

    pub fn set_activated(&mut self, workspace_id: WorkspaceId) {
        if let Some(id) = self.activated_workspace {
            if id != workspace_id {
                self.current_workspace_mut().deactivate();
                self.activated_workspace = Some(workspace_id);
            }
        } else {
            self.activated_workspace = Some(workspace_id);
        }
    }

    pub fn _remove_workspace(&mut self, _workspace_id: usize) {
        // move windows
        todo!()
    }

    pub fn current_workspace(&self) -> &Workspace {
        self.activated_workspace
            .and_then(|id| self.workspaces.iter().find(|w| w.id() == id))
            .expect("no current_workspace")
    }

    pub fn current_workspace_mut(&mut self) -> &mut Workspace {
        self.activated_workspace
            .and_then(|id| self.workspaces.iter_mut().find(|w| w.id() == id))
            .expect("no current_workspace")
    }

    pub fn map_element(&mut self, window: Window, edges: ResizeEdge, activate: bool, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut()
            .map_element(window, edges, activate, loop_handle);
    }

    pub fn refresh(&mut self) {
        self.current_workspace_mut().refresh();
    }

    pub fn window_under(
        &self,
        position: Point<f64, Logical>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        self.current_workspace()
            .window_under(position)
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> {
        self.current_workspace().elements()
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&Window> {
        // TODO: maybe can use hashmap to store the surface
         self.current_workspace().find_window(surface)
    }

    pub fn _workspaces_counts(&self) -> usize {
        self.workspaces.iter().count()
    }

    pub fn unmap_element(&mut self, window: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().unmap_element(window, loop_handle);
    }

    pub fn invert_window(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().invert_window(loop_handle);
    }

    pub fn modify_windows(&mut self, rec: Rectangle<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().modify_windows(rec, loop_handle);
    }

    // pub fn resize(&mut self, offset: Point<i32, Logical>, edges: &ResizeEdge, rec: &mut Rectangle<i32, Logical>) {
    //     self.current_workspace_mut().resize(offset, edges, rec);
    // }

    pub fn set_focus(&mut self, window: Option<Window>) {
        self.current_workspace_mut().set_focus(window);
    }

    pub fn get_focus(&self) -> &Option<Window> {
        &self.current_workspace().get_focus()
    }

    pub fn current_space(&self) -> &Space<Window> {
        self.current_workspace().current_space()
    }

    pub fn window_geometry(&self, window: &Window) -> Option<Rectangle<i32, Logical>> {
        self.current_workspace().window_geometry(window)
    }

    pub fn check_grab(&mut self, surface: &WlSurface) -> Option<(&Window, Rectangle<i32, Logical>)> {
        self.current_workspace_mut().check_grab(surface)
    }

    pub fn tiled_expansion(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().tiled_expansion(loop_handle);
    }

    pub fn tiled_recover(&mut self, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.current_workspace_mut().tiled_recover(loop_handle);
    }
}
