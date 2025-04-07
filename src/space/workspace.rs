use std::sync::atomic::{AtomicUsize, Ordering};

use smithay::{
    desktop::{Space, Window},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
};

use crate::{config::WorkspaceConfigs, layout::workspace::WorkspaceLayout};

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
    pub layout: WorkspaceLayout,
}

impl Workspace {
    pub fn new(output: &Output, location: (i32, i32), layout: WorkspaceLayout) -> Self {
        let mut space: Space<Window> = Default::default();
        space.map_output(output, location);
        Self {
            id: WorkspaceID::next(),
            space,
            layout,
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
        gap: i32,
        activate: bool,
    ) {
        self.refresh();

        self.map_element(window, (0, 0).into(), activate);

        let output_geo = self.output_geometry(output);
        let output_width = output_geo.size.w;
        let output_height = output_geo.size.h;

        let windows = self.elements();
        let windows_count = self.elements_count() as i32;

        let mut abc: Vec<(Window, Point<i32, Logical>)> = vec![];
        
        for (i, window) in windows.enumerate() {
            let (mut x, mut y) = (gap, gap);
            let (mut width, mut height) = (output_width - 2*gap, output_height - 2*gap);
            if windows_count > 1 {
                width -= gap;
                width /= 2;
            }

            if i > 0 {
                height /= windows_count-1;
                x += width+gap;
                y += height * (i as i32 - 1);
            }

            if i > 1 {
                height -= gap;
                y += gap;
            }

            window.toplevel().unwrap().with_pending_state(|state|
                state.size = Some((width, height).into())
            );

            window.toplevel().unwrap().send_pending_configure();
            abc.push((window.clone(), Point::from((x, y))));
        }

        for (window, location) in abc {
            self.map_element(window, location, false);
        }

        // self.layout.mapped_windows(&mut self.space);
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
    pub config: WorkspaceConfigs,
    pub activated_workspace: Option<WorkspaceID>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: vec![],
            config: WorkspaceConfigs::default(),
            activated_workspace: None,
        }
    }

    // TODO: allow more output binds
    pub fn add_workspace(
        &mut self,
        output: &Output,
        location: (i32, i32),
        layout: Option<WorkspaceLayout>,
        activate: bool,
    ) {
        let workspace = Workspace::new(
            output,
            location,
            layout.unwrap_or_else(WorkspaceLayout::default),
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
        activate: bool,
    ) {
        let gap = self.config.gap;
        self.current_workspace_mut().map_tiled_element(window, output, gap, activate);
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
