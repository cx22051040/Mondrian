pub mod winit;
pub mod tty;

use smithay::{backend::renderer::{ImportAll, ImportMem, Renderer}, reexports::calloop::LoopHandle};

use tty::Tty;
use winit::Winit;

use crate::{manager::output::OutputManager, state::GlobalData};

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

    pub fn init(&mut self, output_manager: &mut OutputManager, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Self::Winit(v) = self {
            v.init(output_manager);
        } else if let Self::Tty(v) = self {
            v.init(output_manager, loop_handle);
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