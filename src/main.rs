#![allow(irrefutable_let_patterns)]

#[macro_use]
extern crate tracing;

mod backend;
mod config;
mod handler;
mod input;
mod layout;
mod render;
mod space;
mod state;

use smithay::reexports::calloop::EventLoop;

use tracing_subscriber;

use state::NuonuoState;

pub const OUTPUT_NAME: &str = "winit";

fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let mut event_loop: EventLoop<'_, NuonuoState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();

    let mut nuonuo_state = NuonuoState::new(loop_handle);

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

    event_loop
        .run(None, &mut nuonuo_state, move |_| {
            // Nuonuo is running
        })
        .unwrap();
}
