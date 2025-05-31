use std::{collections::HashMap, hash::Hash, sync::atomic::{AtomicUsize, Ordering}};

use smithay::{
    desktop::{Space, Window},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
};

use crate::{input::resize_grab::ResizeEdge, layout::tiled_tree::{TiledScheme, TiledTree}};

use super::window::WindowExt;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
const GAP: i32 = 12;

#[derive(Debug, Clone)]
pub enum WindowLayout {
    Tiled,
    Floating,
}

impl WindowLayout {
    pub fn default() -> Self {
        Self::Tiled
    }
}

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
    pub floating: Space<Window>,
    pub layout: HashMap<Window, WindowLayout>,

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
            floating,
            layout: HashMap::new(),

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
        target: Option<Window>, 
        window: Window, 
        location: Point<i32, Logical>, 
        layout: Option<WindowLayout>, 
        activate: bool,

    ) {
        let layout = layout.unwrap_or_else(|| {
            match self.layout.get(&window) {
                Some(layout) => layout.clone(),
                None => WindowLayout::default()
            }
        });

        window.toplevel().unwrap().with_pending_state(|state| {
            state.bounds = Some(self.output_geometry.size)
        });
        window.toplevel().unwrap().send_pending_configure();

        match layout {
            WindowLayout::Tiled => {
                match self.layout.insert(window.clone(), WindowLayout::Tiled) {
                    Some(_) => {
                        self.tiled.map_element(window, location, activate);
                    }
                    None => {
                        self.map_tiled_element(target, window, activate);
                    }
                }
            }
            WindowLayout::Floating => {
                self.layout.insert(window.clone(), WindowLayout::Floating);
                self.map_floating_element(window, location, activate);
            }
        }
    }

    pub fn unmap_element(&mut self, window: &Window) {
        if let Some(layout) = self.layout.remove(window) {
            // unset focus
            if let Some(focus) = &self.focus {
                if focus == window {
                    self.focus = None;
                }
            }

            match layout {
                WindowLayout::Tiled => {
                    self.unmap_tiled_element(window);
                }
                WindowLayout::Floating => {
                    self.unmap_floating_element(window);
                }
            }
        }
    }

    fn map_floating_element (
        &mut self,
        window: Window,
        location: Point<i32, Logical>,
        activate: bool,
    ) {
        self.refresh();

        self.floating.map_element(window.clone(), location, activate);
        
        // set focus
        if activate {
            self.focus = Some(window);
        }
    }

    fn map_tiled_element(
        &mut self,
        target: Option<Window>,
        window: Window,
        activate: bool,
    ) {
        self.refresh();

        if self.tiled_tree.is_none() {
            window.set_rec(
                (self.output_geometry.size - (GAP * 2, GAP * 2).into()).into(),
            );
            self.tiled_tree = Some(TiledTree::new(window.clone()));
            self.tiled.map_element(window.clone(), (GAP, GAP), activate);
        } else {
            match target {
                Some(target) => {
                    if let Some(layout_tree) = &mut self.tiled_tree {
                        layout_tree.insert_window(&Some(target), window.clone(), &mut self.tiled);
    
                        #[cfg(feature = "trace_layout")]
                        layout_tree.print_tree();
                    }
                }
                None => {
                    match self.scheme {
                        TiledScheme::Default => {
                            if let Some(layout_tree) = &mut self.tiled_tree {
                                layout_tree.insert_window(&self.focus, window.clone(), &mut self.tiled);
            
                                #[cfg(feature = "trace_layout")]
                                layout_tree.print_tree();
                            }
                        }
                    }
                }
            }
        }

        // set focus
        if activate {
            self.focus = Some(window);
        }
    }

    fn unmap_tiled_element(&mut self, window: &Window) {
        if let Some(tiled_tree) = &mut self.tiled_tree {
            tiled_tree.remove(window, &mut self.tiled);

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

    fn unmap_floating_element(&mut self, window: &Window) {
        self.floating.unmap_elem(window);
    }

    pub fn raise_element(&mut self, window: &Window, activate: bool) {
        match self.layout.get(window) {
            Some(layout) => {
                match layout {
                    WindowLayout::Tiled => {
                        self.tiled.raise_element(window, activate)
                    }
                    WindowLayout::Floating => {
                        self.floating.raise_element(window, activate)
                    }
                }
            }
            None => {
                warn!("Failed to get window's layout type");
            }
        }
    }

    pub fn refresh(&mut self) {
        self.tiled.refresh();
        self.floating.refresh();
    }

    pub fn window_under(
        &self,
        position: Point<f64, Logical>,
        extra: Option<WindowLayout>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        match extra {
            Some(layout) => {
                match layout {
                    WindowLayout::Floating => self.floating.element_under(position),
                    WindowLayout::Tiled => self.tiled.element_under(position),
                }
            }
            None => {
                self.floating.element_under(position).or_else(|| {
                    self.tiled.element_under(position)
                })
            }
        }
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> {
        self.tiled.elements().chain(self.floating.elements())
    }

    // deactivate all window
    pub fn deactivate(&mut self) {
        for window in self.tiled.elements() {
            window.set_activated(false);
            window.toplevel().unwrap().send_pending_configure();
        }
    }

    pub fn invert_window(&mut self) {
        if let Some(layout_tree) = &mut self.tiled_tree {
            if let Some(focus) = &self.focus {
                layout_tree.invert_window(focus, &mut self.tiled);
        
                #[cfg(feature = "trace_layout")]
                layout_tree.print_tree();
            }
        }
    }

    pub fn modify_windows(&mut self, rec: Rectangle<i32, Logical>) {
        self.output_geometry = rec;
        if let Some(layout_tree) = &mut self.tiled_tree {
            let root_id = layout_tree.get_root().unwrap();
            layout_tree.modify(
                root_id,
                Rectangle::new(
                    (GAP, GAP).into(),
                    (rec.size - (GAP * 2, GAP * 2).into()).into(),
                ),
                &mut self.tiled
            );
        }
    }

    pub fn resize(&mut self, offset: Point<f64, Logical>, edges: &ResizeEdge, rec: &mut Rectangle<i32, Logical>) {
        if let Some(focus) = &self.focus {
            match self.layout.get(focus) {
                Some(layout) => {
                    match layout {
                        WindowLayout::Tiled => {
                            if let Some(layout_tree) = &mut self.tiled_tree {
                                layout_tree.resize(focus, offset, &mut self.tiled);
                            }
                        }
                        WindowLayout::Floating => {

                            let mut x = offset.x as i32;
                            let mut y = offset.y as i32;

                            if edges.intersects(ResizeEdge::LEFT) {
                                rec.loc.x += x;
                                x = -x;
                            }

                            if edges.intersects(ResizeEdge::TOP) {
                                rec.loc.y += y;
                                y = -y;
                            }

                            rec.size.w += x;
                            rec.size.h += y;

                            focus.set_rec(rec.size);

                            self.map_floating_element(focus.clone(), rec.loc, true);
                        }
                    }
                }
                None => {
                    warn!("Failed to get window's layout type");
                }
            }
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

    pub fn get_focus(&self) -> &Option<Window> {
        &self.focus
    }

    pub fn current_space(&self) -> &Space<Window> {
        &self.tiled
    }

    pub fn window_geometry(&self, window: &Window) -> Option<Rectangle<i32, Logical>> {
        self.tiled.element_geometry(window).or_else(|| {
            self.floating.element_geometry(window)
        })
    }

    pub fn find_window(&self, surface: &WlSurface) ->Option<&Window> {
        self.elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == surface)
    }

    pub fn check_grab(&mut self, surface: &WlSurface) -> Option<(&Window, Rectangle<i32, Logical>, WindowLayout)> {
        // TODO: check window's lock state
        let window = self.find_window(surface)?;

        let rec = self.window_geometry(window).or_else(|| {
            warn!("Failed to get window's geometry");
            None
        })?;
    
        let layout = self.layout.get(window).cloned().or_else(|| {
            warn!("Failed to get window's layout type");
            None
        })?;

        Some((window, rec, layout))
    }

    pub fn grab_request(&mut self, window: &Window, rec: Rectangle<i32, Logical>) {
        match self.layout.get_mut(window) {
            Some(layout) => {
                match layout {
                    WindowLayout::Tiled => {
                        self.unmap_element(window);

                        window.set_rec(rec.size);
                        self.map_element(None, window.clone(), rec.loc, Some(WindowLayout::Floating), true);
                    }
                    _ => { }
                }
            }
            None => {
                warn!("Failed to get layout from window: {:?}", window);
                return
            }
        }
    }

    pub fn grab_release(&mut self, target: Option<Window>, window: &Window, layout: &WindowLayout) {
        match layout {
            WindowLayout::Tiled => {
                self.unmap_element(window);
                // TODO: set location let cursor in the middle
                self.map_element(target, window.clone(), (0, 0).into(), Some(WindowLayout::Tiled), true);
            }
            _ => { }
        }
    }

    pub fn toggle_window(&mut self) {
        if let Some(focus) = &self.focus {
            let window = focus.clone();
            match self.layout.get(focus) {
                Some(layout) => {
                    match layout {
                        WindowLayout::Tiled => {
                            self.unmap_element(&window);
                            self.map_element(None, window, (0, 0).into(), Some(WindowLayout::Floating), true);
                        }
                        WindowLayout::Floating => {
                            self.unmap_element(&window);
                            // TODO: judge pointer's location
                            self.map_element(None, window, (0, 0).into(), Some(WindowLayout::Tiled), true);
                        }
                    }
                }
                None => {
                    warn!("Failed to get window's layout type");
                }
            }
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

    pub fn map_element(&mut self, target: Option<Window>, window: Window, location: Point<i32, Logical>, layout: Option<WindowLayout>, activate: bool) {
        self.current_workspace_mut()
            .map_element(target, window, location, layout, activate);
    }

    pub fn refresh(&mut self) {
        self.current_workspace_mut().refresh();
    }

    pub fn window_under(
        &self,
        position: Point<f64, Logical>,
        extra: Option<WindowLayout>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        self.current_workspace()
            .window_under(position, extra)
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

    pub fn unmap_element(&mut self, window: &Window) {
        self.current_workspace_mut().unmap_element(window);
    }

    pub fn invert_window(&mut self) {
        self.current_workspace_mut().invert_window();
    }

    pub fn modify_windows(&mut self, rec: Rectangle<i32, Logical>) {
        self.current_workspace_mut().modify_windows(rec);
    }

    pub fn resize(&mut self, offset: Point<f64, Logical>, edges: &ResizeEdge, rec: &mut Rectangle<i32, Logical>) {
        self.current_workspace_mut().resize(offset, edges, rec);
    }

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

    pub fn check_grab(&mut self, surface: &WlSurface) -> Option<(&Window, Rectangle<i32, Logical>, WindowLayout)> {
        self.current_workspace_mut().check_grab(surface)
    }

    pub fn grab_request(&mut self, window: &Window, rec: Rectangle<i32, Logical>) {
        self.current_workspace_mut().grab_request(window, rec);
    }

    pub fn grab_release(&mut self, target: Option<Window>, window: &Window, layout: &WindowLayout) {
        self.current_workspace_mut().grab_release(target, window, layout);
    }

    pub fn toggle_window(&mut self) {
        self.current_workspace_mut().toggle_window()
    }
}
