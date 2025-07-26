use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use smithay::{
    desktop::{Window, WindowSurface},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        compositor::{self, with_states}, foreign_toplevel_list::ForeignToplevelHandle, shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData}
    }, xwayland::X11Surface,
};

use crate::{config::windowrules::WindowRulesConfigs, layout::{container_tree::ExpansionCache, WindowLayout}, state::{GlobalData, State}};

use super::workspace::WorkspaceId;

pub enum CustomWindowSurface {
    WlSurface(WlSurface),
    X11Surface(X11Surface)
}

impl From<WlSurface> for CustomWindowSurface {
    fn from(value: WlSurface) -> Self {
        CustomWindowSurface::WlSurface(value)
    }
}

impl From<ToplevelSurface> for CustomWindowSurface {
    fn from(value: ToplevelSurface) -> Self {
        CustomWindowSurface::WlSurface(value.wl_surface().clone())
    }
}

impl From<X11Surface> for CustomWindowSurface {
    fn from(value: X11Surface) -> Self {
        CustomWindowSurface::X11Surface(value)
    }
}

pub trait WindowExt {
    fn set_layout(&self, layout: WindowLayout);
    fn get_layout(&self) -> WindowLayout;
    fn set_rect_cache(&self, rect: Rectangle<i32, Logical>);
    fn send_rect(&self, rect: Rectangle<i32, Logical>);
    fn get_rect(&self) -> Option<Rectangle<i32, Logical>>;
    fn get_title_and_id(&self) -> (Option<String>, Option<String>);
}

impl WindowExt for Window {
    fn set_layout(&self, layout: WindowLayout) {
        let layout_ref = self
            .user_data()
            .get_or_insert::<Rc<RefCell<WindowLayout>>, _>(|| {
                Rc::new(RefCell::new(layout.clone()))
            });

        *layout_ref.borrow_mut() = layout;
    }

    fn get_layout(&self) -> WindowLayout {
        self.user_data().get::<Rc<RefCell<WindowLayout>>>().unwrap().borrow().clone()
    }

    fn set_rect_cache(&self, rect: Rectangle<i32, Logical>) {
        let rect_ref = self
            .user_data()
            .get_or_insert::<Rc<RefCell<Rectangle<i32, Logical>>>, _>(|| {
                Rc::new(RefCell::new(rect.clone()))
            });

        *rect_ref.borrow_mut() = rect;

        match self.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                let is_initial = !with_states(toplevel.wl_surface(), |states| {
                    states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .initial_configure_sent
                });

                if is_initial {
                    toplevel.with_pending_state(|state| state.size = Some(rect.size));
                    toplevel.send_configure();
                }
            },
            WindowSurface::X11(_) => { }
        };
    }

    fn send_rect(&self, rect: Rectangle<i32, Logical>) {
        // for animation, the final rect is rect_cache
        match self.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                toplevel.with_pending_state(|state| state.size = Some(rect.size));
                toplevel.send_pending_configure();
            },
            WindowSurface::X11(x11) => {
                let _ = x11.configure(rect);
            }
        };
    }

    fn get_rect(&self) -> Option<Rectangle<i32, Logical>>{
        // must have rect
        self.user_data().get::<Rc<RefCell<Rectangle<i32, Logical>>>>().and_then(|rect| Some(rect.borrow().clone()))
    }

    fn get_title_and_id(&self) -> (Option<String>, Option<String>) {
        match self.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                compositor::with_states(toplevel.wl_surface(), |states| {
                    let roll = &mut states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .unwrap()
                        .lock()
                        .unwrap();
                    (roll.title.clone(), roll.app_id.clone())
                })
            },
            WindowSurface::X11(x11_surface) => {
                let title = x11_surface.title();
                let class = Some(x11_surface.class());
                (Some(title), class)
            }
        }
    }
}

pub struct WindowManager {
    mapped: Vec<Window>,
    unmapped: Vec<Window>,
    pub window_workspace: HashMap<Window, WorkspaceId>,
    pub foreign_handle: HashMap<WlSurface, ForeignToplevelHandle>,

    configs: Arc<WindowRulesConfigs>,
}

impl WindowManager {
    pub fn new(configs: Arc<WindowRulesConfigs>) -> Self {
        Self {
            mapped: Vec::new(),
            unmapped: Vec::new(),
            window_workspace: HashMap::new(),
            foreign_handle: HashMap::new(),
            configs
        }
    }

    pub fn add_window_unmapped(&mut self, window: Window, workspace_id: WorkspaceId) {
        self.window_workspace.insert(window.clone(), workspace_id);
        self.unmapped.push(window);
    }

    pub fn get_configure(&mut self, window: &Window, state: &mut State) {
        match window.underlying_surface() {
            WindowSurface::Wayland(toplevel) => {
                // add foreign handle
                let (title, app_id) = 
                    compositor::with_states(toplevel.wl_surface(), |states| {
                        let roll= &mut states
                            .data_map
                            .get::<XdgToplevelSurfaceData>()
                            .unwrap()
                            .lock()
                            .unwrap();
                        (roll.title.clone(), roll.app_id.clone())
                    });

                let handle = state
                    .foreign_toplevel_state
                    .new_toplevel::<GlobalData>(title.unwrap_or("unkown".to_string()), app_id.unwrap_or("unkown".to_string()));

                self.foreign_handle
                    .insert(toplevel.wl_surface().clone(), handle);

                // check if is child toplevel
                self.is_child_window(window);
            },
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(_) => { }
        }
    }

    fn is_child_window(&self, window: &Window) {
        // if the window has parent
        // need be set as float
        // the rect set as half of parent if not given

        #[cfg_attr(not(feature = "xwayland"), allow(irrefutable_let_patterns))]
        if let Some(toplevel) = window.toplevel() {
            if let Some(parent) = toplevel.parent() {
                if let Some(parent_window) = self.get_mapped(&parent.into()) {
                    if let Some(rect) = compute_child_rect(&parent_window, toplevel.current_state().size) {
                        window.set_layout(WindowLayout::Floating);
                        window.set_rect_cache(rect);
                        window.send_rect(rect);
                    }
                }
            }
        }
    }

    pub fn get_mapped(&self, surface: &CustomWindowSurface) -> Option<&Window> {
        match surface {
            CustomWindowSurface::WlSurface(wl_surface) => {
                self.mapped
                    .iter()
                    .find(|w| matches!(w.toplevel(), Some(w) if w.wl_surface() == wl_surface))
            },
            CustomWindowSurface::X11Surface(x11_surface) => {
                self.mapped
                    .iter()
                    .find(|w| matches!(w.x11_surface(), Some(w) if w == x11_surface))
            }
        }
    }

    pub fn get_unmapped(&self, surface: &CustomWindowSurface) -> Option<&Window> {
        match surface {
            CustomWindowSurface::WlSurface(wl_surface) => {
                self.unmapped
                    .iter()
                    .find(|w| matches!(w.toplevel(), Some(w) if w.wl_surface() == wl_surface))
            },
            CustomWindowSurface::X11Surface(x11_surface) => {
                self.unmapped
                    .iter()
                    .find(|w| matches!(w.x11_surface(), Some(w) if w == x11_surface))
            }
        }
    }

    pub fn set_mapped(&mut self, unmapped: &Window) {
        if let Some(pos) = self.unmapped.iter().position(|w| w == unmapped) {
            let window = self.unmapped.remove(pos);

            match window.get_layout() {
                WindowLayout::Tiled => {
                    self.mapped.push(window);
                }
                WindowLayout::Floating => {
                    self.mapped.insert(0, window);
                }
            }
        }
    }
    
    pub fn set_unmapped(&mut self, mapped: &Window) -> bool {
        if self.mapped.contains(mapped) {
            // remove foreign handle
            match mapped.underlying_surface() {
                WindowSurface::Wayland(toplevel) => {
                    self.foreign_handle
                        .remove(toplevel.wl_surface());
                },            
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(_) => { }
            }

            if let Some(pos) = self.mapped.iter().position(|w| w == mapped) {
                let window = self.mapped.remove(pos);
                self.unmapped.push(window);
                return true;
            }
        }

        false
    }

    pub fn mapped_windows(&self, workspace_id: WorkspaceId) -> impl Iterator<Item = &Window> {
        self.mapped.iter().filter(move |window| {
            self.window_workspace.get(*window) == Some(&workspace_id)
        })
    }

    pub fn remove_unmapped(&mut self, unmapped: &Window) -> Option<Window> {
        if self.unmapped.contains(unmapped) {
            self.window_workspace.remove(unmapped);
            
            match unmapped.underlying_surface() {
                WindowSurface::Wayland(toplevel) => {
                    toplevel.send_close();
                },            
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(x11_surface) => {
                    let _ = x11_surface.close();
                }
            }
    
            if let Some(pos) = self.unmapped.iter().position(|w| w == unmapped) {
                return Some(self.unmapped.remove(pos));
            }
        }

        None
    }

    pub fn window_under(&self, pointer_loc: Point<f64, Logical>, workspace_id: WorkspaceId) -> Option<Window> {
        for window in &self.mapped {
            if Some(&workspace_id) != self.window_workspace.get(window) {
                continue;
            }

            // expansion window
            if let Some(guard) = window.user_data().get::<ExpansionCache>() {
                if let Some(window_rect) = guard.get() {
                    if window_rect.contains(pointer_loc.to_i32_round()) {
                        return Some(window.clone())
                    }
                    continue;
                }
            }

            if let Some(window_rect) = window.get_rect() {
                if window_rect.contains(pointer_loc.to_i32_round()) {
                    return Some(window.clone())
                }
            }
        }

        None
    }

    pub fn window_under_tiled(&self, pointer_loc: Point<f64, Logical>, workspace_id: WorkspaceId) -> Option<Window> {
        for window in &self.mapped {
            if Some(&workspace_id) != self.window_workspace.get(window) {
                continue;
            }

            if matches!(window.get_layout(), WindowLayout::Floating) {
                continue;
            }
            
            if let Some(window_rect) = window.get_rect() {
                if window_rect.contains(pointer_loc.to_i32_round()) {
                    return Some(window.clone())
                }
            }
        }

        None
    }

    pub fn switch_layout(&mut self, window: &Window) {
        if self.mapped.contains(window) {
            let layout = window.get_layout();

            self.mapped.retain(|w| w != window);
            match layout {
                WindowLayout::Tiled => {
                    self.mapped.insert(0, window.clone());
                    window.set_layout(WindowLayout::Floating);
                }
                WindowLayout::Floating => {
                    self.mapped.push(window.clone());
                    window.set_layout(WindowLayout::Tiled);
                }
            }
        }
    }

    pub fn raise_window(&mut self, window: &Window) {
        if self.mapped.contains(window) {
            self.mapped.retain(|w| w != window);
            self.mapped.insert(0, window.clone());
        }
    }

    pub fn get_opacity(&self, window: &Window) -> Option<f32> {
        let (_, app_id) = window.get_title_and_id();
        if let Some(app_id) = app_id {
            self.configs.global_opacity.get(&app_id).cloned()
        } else {
            None
        }
    }

    pub fn get_foreign_handle(&self, surface: &WlSurface) -> Option<&ForeignToplevelHandle> {
        self.foreign_handle.get(surface)
    }
}

fn compute_child_rect(parent_window: &Window, size_opt: Option<Size<i32, Logical>>) -> Option<Rectangle<i32, Logical>>{
    parent_window.get_rect().and_then(|parent_rect| {
        let parent_rect = parent_rect.to_f64();
        let center = parent_rect.loc + parent_rect.size / 2.0;
    
        let size: Size<f64, Logical> = size_opt.map(|s| s.to_f64()).unwrap_or(parent_rect.size / 2.0);
        let loc = center - size / 2.0;
        Some(Rectangle::new(loc, size).to_i32_round())
    })
}