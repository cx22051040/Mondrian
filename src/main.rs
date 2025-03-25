#![allow(irrefutable_let_patterns)]

#[macro_use]
extern crate tracing;

mod backend;
mod config;
mod input;
mod render;
mod state;
mod elements;
mod xdg_shell;

use smithay::
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction}, 
        wayland_server::Display
};

use tracing_subscriber;

use state::NuonuoState;

pub const OUTPUT_NAME: &str = "winit";

fn main (){
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }


    let mut event_loop: EventLoop<'_, NuonuoState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();

    let display: Display<NuonuoState> = Display::new().unwrap();
    let display_handle = display.handle();

    loop_handle
      .insert_source(
          Generic::new(display, Interest::READ, Mode::Level),
          |_, display, data| {
              // Safety: we don't drop the display
              unsafe {
                  display.get_mut().dispatch_clients(data).unwrap();
              }
              Ok(PostAction::Continue)
          },
      )
      .expect("Failed to init wayland server source");

    #[cfg(feature = "winit")]
    let backend_data = backend::winit::init_winit(&loop_handle, &display_handle);
    let mut nuonuo_state = NuonuoState::new(display_handle, &loop_handle, backend_data);


    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    unsafe { std::env::set_var("WAYLAND_DISPLAY", &nuonuo_state.socket_name) };

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            std::process::Command::new(command).spawn().ok();
        }
        _ => {
            std::process::Command::new("weston-terminal").spawn().ok();
        }
    }

    tracing::info!("Initialization completed, starting the main loop.");

    event_loop.run(None, &mut nuonuo_state, move |_| {
        // Nuonuo is running
    }).unwrap();

}
