use std::sync::atomic::{AtomicUsize, Ordering};

use smithay::{
    desktop::{Space, Window},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
};

use crate::{config::WorkspaceConfigs, layout::workspace::{LayoutScheme, TiledLayoutTree}};

use super::window::WindowExt;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceID(usize);

impl WorkspaceID {
    // only for test
    pub fn new (id: usize) -> Self {
        Self(id)
    }
    #[inline]
    pub fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub id: WorkspaceID,
    pub space: Space<Window>,
    pub layout: LayoutScheme,
    pub layout_tree: Option<TiledLayoutTree>,
}

impl Workspace {
    pub fn new(output: &Output, location: (i32, i32), layout: LayoutScheme) -> Self {
        let mut space: Space<Window> = Default::default();
        space.map_output(output, location);
        Self {
            id: WorkspaceID::next(),
            space,
            layout,
            layout_tree: None,
        }
    }

    pub fn id(&self) -> WorkspaceID {
        self.id
    }

    pub fn map_element(&mut self, window: Window, location: Point<i32, Logical>, activate: bool) {
        self.space.map_element(window, location, activate);
    }

    pub fn map_tiled_element(
        &mut self,
        window: Window,
        output: &Output,
        focused_surface: Option<WlSurface>,
        activate: bool,
    ) {
        self.refresh();
        let output_geo = self.output_geometry(output);
        
        match self.layout {
            LayoutScheme::Default => {
                if self.layout_tree.is_none() {
                    let (tree, location, ) = TiledLayoutTree::new(
                        window.clone(),
                        output_geo
                    );
                    self.layout_tree = Some(tree);
                    self.map_element(window, location, activate);
                } else if let Some(layout_tree) = &mut self.layout_tree {

                    let focused_window = focused_surface.and_then(|surface| {
                        self.space.elements().find(|window| {
                            *window.toplevel().unwrap().wl_surface() == surface
                        })
                    });

                    let location = layout_tree.insert_default(window.clone(), focused_window).expect("error while interting window");
                    self.map_element(window, location, activate);
                }
            },
            LayoutScheme::BinaryTree => {
                if self.layout_tree.is_none()  {
                    let (tree, location) = TiledLayoutTree::new(
                        window.clone(),
                        output_geo
                    );
                    self.layout_tree = Some(tree);
                    self.map_element(window, location, activate);
                } else if let Some(layout_tree) = &mut self.layout_tree {
                    let location = layout_tree.insert_binary_tree(window.clone());
                    self.map_element(window, location, activate);
                }
            }
        }

        for win in self.elements() {
            win.toplevel().unwrap().send_pending_configure();
        }

    }

    pub fn raise_element(&mut self, window: &Window, activate: bool) {
        self.space.raise_element(window, activate);
    }

    pub fn refresh(&mut self) {
        self.space.refresh();
    }

    pub fn element_under(
        &self,
        position: Point<f64, Logical>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        self.space.element_under(position).map(|(w, p)| (w, p))
    }

    pub fn element_location(&self, window: &Window) -> Point<i32, Logical> {
        self.space.element_location(window).unwrap()
    }

    pub fn element_geometry(&self, window: &Window) -> Rectangle<i32, Logical> {
        self.space.element_geometry(window).unwrap()
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> + ExactSizeIterator {
        self.space.elements()
    }

    pub fn elements_count(&self) -> usize {
        self.space.elements().count()
    }

    // TODO: move this to outputmanager
    pub fn output_geometry(&self, output: &Output) -> Rectangle<i32, Logical> {
        self.space.output_geometry(output).unwrap()
    }

    // deactivate all window
    pub fn deactivate(&mut self) {
        for window in self.space.elements() {
            window.set_activated(false);
            window.toplevel().unwrap().send_pending_configure();
        }
    }
    
    pub fn _remove(&mut self) {
        todo!()
    }
}

#[derive(Debug)]
pub struct WorkspaceManager {
    pub workspaces: Vec<Workspace>,
    pub activated_workspace: Option<WorkspaceID>,
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
        location: (i32, i32),
        layout: Option<LayoutScheme>,
        activate: bool,
    ) {
        let workspace = Workspace::new(
            output,
            location,
            layout.unwrap_or_else(|| LayoutScheme::Default),
        );

        if activate {
            self.set_activated(workspace.id());
        }

        self.workspaces.push(workspace);
    }

    pub fn set_activated(&mut self, workspace_id: WorkspaceID) {
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

    pub fn map_element(&mut self, window: Window, location: Point<i32, Logical>, activate: bool) {
        self.current_workspace_mut()
            .map_element(window, location, activate);
    }

    pub fn map_tiled_element(
        &mut self,
        window: Window,
        output: &Output,
        focused_surface: Option<WlSurface>,
        activate: bool,
    ) {
        self.current_workspace_mut().map_tiled_element(window, output, focused_surface, activate);
    }

    pub fn raise_element(&mut self, window: &Window, activate: bool) {
        self.current_workspace_mut().raise_element(window, activate);
    }

    pub fn refresh(&mut self) {
        self.current_workspace_mut().refresh();
    }

    pub fn element_under(
        &self,
        position: Point<f64, Logical>,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        self.current_workspace()
            .element_under(position)
            .map(|(w, p)| (w, p))
    }

    pub fn element_location(&self, window: &Window) -> Point<i32, Logical> {
        self.current_workspace().element_location(window)
    }

    pub fn element_geometry(&self, window: &Window) -> Rectangle<i32, Logical> {
        self.current_workspace().element_geometry(window)
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> + ExactSizeIterator {
        self.current_workspace().elements()
    }

    pub fn find_window(&self, wl_surface: &WlSurface) -> &Window {
        // TODO: maybe can use hashmap to store the surface
        self.elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
            .unwrap()
    }

    pub fn output_geometry(&self, output: &Output) -> Rectangle<i32, Logical> {
        self.current_workspace().output_geometry(output)
    }

    pub fn _workspaces_counts(&self) -> usize {
        self.workspaces.iter().count()
    }
}
