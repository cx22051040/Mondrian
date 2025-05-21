#![allow(irrefutable_let_patterns)]

#[macro_use]
extern crate tracing;

mod backend;
mod config;
mod input;
mod layout;
mod manager;
mod protocol;
mod render;
mod state;

use std::sync::Arc;

use smithay::{
    reexports::{
        calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic},
        wayland_server::Display,
    },
    wayland::socket::ListeningSocketSource,
};

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{self, FmtSubscriber, layer::SubscriberExt};

use state::{ClientState, GlobalData};

pub const OUTPUT_NAME: &str = "winit";

fn main() {
    init_trace();

    let mut event_loop: EventLoop<'_, GlobalData> = EventLoop::try_new().unwrap();
    let display: Display<GlobalData> = Display::new().unwrap();

    let loop_handle = event_loop.handle();
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

    // initial listening socket source
    let source = ListeningSocketSource::new_auto().unwrap();
    let socket_name = source.socket_name().to_string_lossy().into_owned();

    loop_handle
        .insert_source(source, move |client_stream, _, data| {
            data.display_handle
                .insert_client(client_stream, Arc::new(ClientState::default()))
                .unwrap();
        })
        .expect("Failed to init wayland socket source.");

    tracing::info!(name = socket_name, "Listening on wayland socket.");

    // initial the main data
    let mut global_data = GlobalData::new(loop_handle, display_handle);

    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    unsafe { std::env::set_var("WAYLAND_DISPLAY", &socket_name) };

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            std::process::Command::new(command).spawn().ok();
        }
        _ => {}
    }

    tracing::info!("Initialization completed, starting the main loop.");

    event_loop
        .run(None, &mut global_data, move |_| {
            // Nuonuo is running
        })
        .unwrap();
}

fn init_trace() {
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "app-");

    let fmt_layer = tracing_subscriber::fmt::Layer::new()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_level(true);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO) // 不限制日志级别，记录所有日志级别
        .finish()
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");
}

