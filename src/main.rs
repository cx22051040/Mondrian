#![allow(irrefutable_let_patterns)]

#[macro_use]
extern crate tracing;

mod backend;
mod config;
mod protocol;
mod input;
mod layout;
mod render;
mod space;
mod state;

use smithay::reexports::calloop::EventLoop;

use tracing_subscriber::{self, layer::SubscriberExt, FmtSubscriber};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use state::NuonuoState;

pub const OUTPUT_NAME: &str = "winit";

fn main() {

    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "app-");

    let fmt_layer = tracing_subscriber::fmt::Layer::new()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_level(true);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)  // 不限制日志级别，记录所有日志级别
        .finish()
        .with(fmt_layer);
    
    // 设置全局默认日志记录器
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");


    let mut event_loop: EventLoop<'_, NuonuoState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();

    let mut nuonuo_state = NuonuoState::new(loop_handle).expect("cannot make global state");

    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();
    
    unsafe { std::env::set_var("WAYLAND_DISPLAY", &nuonuo_state.socket_name) };

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            std::process::Command::new(command).spawn().ok();
        }
        _ => { }
    }

    tracing::info!("Initialization completed, starting the main loop.");

    event_loop
        .run(None, &mut nuonuo_state, move |_| {
            // Nuonuo is running
        })
        .unwrap();
}
