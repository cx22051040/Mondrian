pub mod tty;
pub mod winit;

use smithay::{backend::renderer::gles::GlesRenderer, reexports::calloop::LoopHandle};

use tty::Tty;
use winit::Winit;

use crate::{manager::{output::OutputManager, render::RenderManager}, render::AsGlesRenderer, state::GlobalData};

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
        output_manager: &mut OutputManager,
        render_manager: &RenderManager,
    ) {
        if let Self::Winit(v) = self {
            v.init(output_manager, render_manager);
        } else if let Self::Tty(v) = self {
            v.init(loop_handle, output_manager, render_manager);
        } else {
            panic!("backend is not Winit");
        }
    }

    pub fn seat_name(&self) -> String {
        if let Self::Winit(_) = self {
            String::from("winit")
        } else if let Self::Tty(v) = self {
            v.seat_name.clone()
        } else {
            panic!("backend is not Winit");
        }
    }
}

