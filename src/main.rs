// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Alvin

#![allow(irrefutable_let_patterns)]

#[macro_use]
extern crate tracing;
extern crate serde;
extern crate serde_json;

mod animation;
mod backend;
mod config;
mod input;
mod layout;
mod manager;
mod protocol;
mod render;
mod state;
mod utils;

use smithay::{
    reexports::{
        calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic},
        wayland_server::Display,
    },
};

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{self, layer::SubscriberExt, FmtSubscriber};

use state::GlobalData;
use utils::errors::AnyHowErr;

fn main() -> anyhow::Result<()> {
    // initial the log tracing
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "app.log");

    let fmt_layer = tracing_subscriber::fmt::Layer::new()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_level(true);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish()
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .anyhow_err("Failed to init tracing subscriber")?;

    tracy_client::Client::start();

    // initial main event loop
    let mut event_loop: EventLoop<'_, GlobalData> =
        EventLoop::try_new().anyhow_err("Failed to init main event loop")?;
    let display: Display<GlobalData> = Display::new().anyhow_err("Failed to init display")?;

    // initial the server source
    let loop_handle = event_loop.handle();
    let display_handle = display.handle();
    loop_handle
        .insert_source(
            Generic::new(display, Interest::READ, Mode::Level),
            |_, display, data| {
                // Safety: we don't drop the display
                unsafe {
                    display
                        .get_mut()
                        .dispatch_clients(data)
                        .expect("Failed to dispatch clients");
                }
                Ok(PostAction::Continue)
            },
        )
        .anyhow_err("Failed to init server source")?;

    // initial the main data
    let mut global_data =
        GlobalData::new(loop_handle, display_handle).anyhow_err("Failed to init global data")?;
    // start xwayland
    #[cfg(feature = "xwayland")]
    global_data.start_xwayland();

    global_data.configs.init();

    // start the project
    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            std::process::Command::new(command).spawn().ok();
        }
        _ => {}
    }

    info!("Initialization completed, starting the main loop.");

    event_loop
        .run(None, &mut global_data, move |data| {
            // running
            let _span = tracy_client::span!("flush_clients");
            data.refresh();
            data.display_handle.flush_clients().unwrap();
        })
        .anyhow_err("Failed to run event loop")?;

    info!("Event loop exited, exiting the program.");

    Ok(())
}

