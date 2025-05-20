pub mod tty;
pub mod winit;

use smithay::{backend::allocator::dmabuf::Dmabuf, reexports::{calloop::LoopHandle, wayland_server::{protocol::wl_surface::WlSurface, DisplayHandle}}};

use tty::Tty;
use winit::Winit;

use crate::{
    manager::{
        output::OutputManager, 
        render::RenderManager
    }, 
    state::{GlobalData, State}
};

pub enum Backend {
    Tty(Tty),
    Winit(Winit),
}

impl Backend {
    pub fn tty(&mut self) -> &mut Tty {
        if let Self::Tty(v) = self {
            v
        } else {
            panic!("backend is not Tty");
        }
    }

    pub fn winit(&mut self) -> &mut Winit {
        if let Self::Winit(v) = self {
            v
        } else {
            panic!("backend is not Winit");
        }
    }

    pub fn init(
        &mut self,
        loop_handle: &LoopHandle<'_, GlobalData>,
        display_handle: &DisplayHandle,
        output_manager: &mut OutputManager,
        render_manager: &RenderManager,
        state: &mut State,
    ) {
        match self {
            Backend::Tty(tty) => tty.init(loop_handle, display_handle, output_manager, render_manager, state),
            Backend::Winit(winit) => winit.init(display_handle, output_manager, render_manager, state),
        }
    }

    pub fn seat_name(&self) -> String {
        if let Self::Winit(_) = self {
            String::from("winit")
        } else if let Self::Tty(v) = self {
            v.seat_name.clone()
        } else {
            panic!("Failed to get seat name");
        }
    }

    pub fn dmabuf_imported(&mut self, dmabuf: &Dmabuf) -> bool {
        match self {
            Backend::Tty(tty) => tty.dmabuf_imported(dmabuf),
            Backend::Winit(winit) => winit.dmabuf_imported(dmabuf),
        }
    }

    pub fn early_import(&mut self, surface: &WlSurface) {
        match self {
            Backend::Tty(tty) => tty.early_import(surface),
            Backend::Winit(_) => {},
        }
    }

}

