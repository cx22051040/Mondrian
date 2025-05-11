pub mod keybindings;
pub mod move_grab;
pub mod resize_grab;

use smithay::{
    backend::
        input::{
            AbsolutePositionEvent, ButtonState, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent
        },
    desktop::{layer_map_for_output, WindowSurfaceType}, input::{
        keyboard::{xkb::keysym_get_name, FilterResult}, pointer::{ButtonEvent, MotionEvent},
    }, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::{Logical, Point, SERIAL_COUNTER}, wayland::{compositor::get_parent, shell::wlr_layer::Layer as WlrLayer}
    };

    use crate::{
    input::keybindings::{FunctionEnum, KeyAction}, manager::workspace::WorkspaceId, state::GlobalData
};

impl GlobalData {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let event_state = event.state();
                let conf_priority_map = self
                    .configs
                    .conf_keybinding_manager
                    .conf_priority_map
                    .clone();

                let keyboard = self.input_manager.get_keyboard();
                let keyboard = match keyboard {
                    Some(k) => k,
                    None => {
                        error!("get keyboard error");
                        return
                    }
                };
    
                keyboard.input::<(), _>(
                    self,
                    event.key_code(),
                    event_state,
                    serial,
                    time,
                    |state, _modifiers, _keysym_handle| {
                        if event_state == KeyState::Pressed {
                            let mut pressed_keys_name: Vec<String> =
                                keyboard.with_pressed_keysyms(|keysym_handles| {
                                    keysym_handles
                                        .iter()
                                        .map(|keysym_handle| {
                                            let keysym_value = keysym_handle.modified_sym();
                                            keysym_get_name(keysym_value)
                                        })
                                        .collect()
                                });
    
                            pressed_keys_name
                                .sort_by_key(|key| conf_priority_map.get(key).cloned().unwrap_or(3));
    
                            let keys = pressed_keys_name.join("+");
    
                            #[cfg(feature = "trace_input")]
                            tracing::info!("Keys: {:?}", keys);
    
                            state.action_keys(keys);
                        }
    
                        FilterResult::Forward
                    },
                );
            }
    
            InputEvent::PointerMotion { .. } => {
                // TODO
            }
    
            InputEvent::PointerMotionAbsolute { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                
                let output = self.output_manager.current_output();
                let output_geo = self.workspace_manager.output_geometry(output);

                // because the absolute move, need to plus the output location
                let position = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
    
                let pointer = self.input_manager.get_pointer();
                let pointer = match pointer {
                    Some(k) => k,
                    None => {
                        error!("get pointer error");
                        return
                    }
                };
    
                let under = self.surface_under(position);
    
                // set focus
                if let Some((surface, _)) = under.clone() {
                    self.set_focus(surface);
                } else {
                    self.modify_all_windows_state(false);
                }

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: position,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
    
            InputEvent::PointerButton { event, .. } => {

                let pointer = self.input_manager.get_pointer();
                let pointer = match pointer {
                    Some(k) => k,
                    None => {
                        error!("get pointer error");
                        return
                    }
                };

                let serial = SERIAL_COUNTER.next_serial();
    
                let button = event.button_code();
                let button_state = event.state();
    
                #[cfg(feature = "trace_input")]
                tracing::info!(
                    "The PointerButton event, button: {button}, location: {:?}",
                    pointer.current_location()
                );
    
                if button_state == ButtonState::Pressed && !pointer.is_grabbed() {
                    let position = pointer.current_location();
                    self.action_pointer_button(position);
                }
    
                // modify pointer state
                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
    
            InputEvent::PointerAxis { .. } => {
                // TODO
            }
    
            InputEvent::DeviceAdded { .. } => {
                // TODO
                #[cfg(feature = "trace_input")]
                tracing::info!("DeviceAdded Event, device: {:?} ", device);
            }
    
            InputEvent::DeviceRemoved { .. } => {
                // TODO
                #[cfg(feature = "trace_input")]
                tracing::info!("DeviceRemoved Event, device: {:?} ", device);
            }
            _ => {}
        }
    }
    
    pub fn surface_under (&mut self, position: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        // get the surface under giving position,
        let output = self.output_manager.current_output().clone();
        let output_geo = self.workspace_manager.output_geometry(&output);
        let layer_map = layer_map_for_output(&output);

        if let Some(layer) = layer_map
            .layer_under(WlrLayer::Overlay, position - output_geo.loc.to_f64())
            .or_else(|| layer_map.layer_under(WlrLayer::Top, position - output_geo.loc.to_f64()))
        {
            let layer_surface_loc = layer_map.layer_geometry(layer).unwrap().loc;
            layer
                .surface_under(
                    position - output_geo.loc.to_f64() - layer_surface_loc.to_f64(),
                    WindowSurfaceType::ALL,
                )
                .map(|(surface, loc)| {
                    (
                        surface,
                        (loc + layer_surface_loc + output_geo.loc).to_f64(),
                    )
                })
        } else if let Some((surface, location)) = self
            .workspace_manager
            .element_under(position)
            .and_then(|(window, location)| {
                window
                    .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
        {
            Some((surface, location))
        } else {            
            None
        }
    }

    pub fn action_pointer_button(&mut self, position: Point<f64, Logical>) {

        let serial = SERIAL_COUNTER.next_serial();

        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return
            }
        };

        let output = self.output_manager.current_output().clone();
        let output_geo = self.workspace_manager.output_geometry(&output);
        let layer_map = layer_map_for_output(&output);

        // TODO: First is full screen
        if let Some(layer) = layer_map
            .layer_under(WlrLayer::Overlay, position - output_geo.loc.to_f64())
            .or_else(|| layer_map.layer_under(WlrLayer::Top, position - output_geo.loc.to_f64()))
        {
            if layer.can_receive_keyboard_focus() {
                if let Some((_, _)) = layer.surface_under(
                    position - output_geo.loc.to_f64() - layer_map.layer_geometry(layer).unwrap().loc.to_f64(), 
                    WindowSurfaceType::ALL,
                ) {
                    keyboard.set_focus(
                        self, 
                        Some(layer.wl_surface().clone()),
                        serial
                    );
                    return
                }
            }
        } else if let Some((window, location)) = self
            .workspace_manager
            .element_under(position)
            .map(|(w, l)| (w.clone(), l.clone()))
        {
            let surface = window
                .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, _)| s);

            // unfocus all window
            self.modify_all_windows_state(false);

            self.workspace_manager.raise_element(&window, true);

            keyboard.set_focus(
                self,
                surface,
                serial,
            );
        } else if let Some(layer) = layer_map
            .layer_under(WlrLayer::Bottom, position - output_geo.loc.to_f64())
            .or_else(|| layer_map.layer_under(WlrLayer::Background, position - output_geo.loc.to_f64()))
        {
            if layer.can_receive_keyboard_focus() {
                if let Some((_, _)) = layer.surface_under(
                    position - output_geo.loc.to_f64() - layer_map.layer_geometry(layer).unwrap().loc.to_f64(), 
                    WindowSurfaceType::ALL,
                ) {
                    keyboard.set_focus(
                        self, 
                        Some(layer.wl_surface().clone()),
                        serial
                    );
                    return
                }
            }
        } else {
            keyboard.set_focus(self, None, serial);
            self.modify_all_windows_state(false);
        }
    }

    pub fn action_keys(&mut self, keys: String) {

        let conf_keybindings = self
            .configs
            .conf_keybinding_manager
            .conf_keybindings
            .clone();

        if let Some(command) = conf_keybindings.get(&keys) {
            match command {
                KeyAction::Command(cmd) => {
                    tracing::info!("Command: {}", cmd);
                    std::process::Command::new(cmd).spawn().ok();
                }
                KeyAction::Internal(func) => {
                    match func {
                        FunctionEnum::SwitchWorkspace1 => {
                            let serial = SERIAL_COUNTER.next_serial();
                            // TODO: move cursor to first window and set focus or none
                            self.modify_all_windows_state(false);

                            let keyboard = self.input_manager.get_keyboard();
                            let keyboard = match keyboard {
                                Some(k) => k,
                                None => {
                                    error!("get keyboard error");
                                    return
                                }
                            };
                    
                            keyboard.set_focus(self, None, serial);
                            self
                                .workspace_manager
                                .set_activated(WorkspaceId::new(1));
                        }
                        FunctionEnum::SwitchWorkspace2 => {
                            let serial = SERIAL_COUNTER.next_serial();
                            self.modify_all_windows_state(false);

                            let keyboard = self.input_manager.get_keyboard();
                            let keyboard = match keyboard {
                                Some(k) => k,
                                None => {
                                    error!("get keyboard error");
                                    return
                                }
                            };
                    
                            keyboard.set_focus(self, None, serial);
                            self
                                .workspace_manager
                                .set_activated(WorkspaceId::new(2));
                        },
                        FunctionEnum::InvertWindow => {
                            let focused_surface = self.get_focus();
                            self
                                .workspace_manager
                                .invert_window(focused_surface);
                        },
                        FunctionEnum::Quit => {
                            tracing::info!("Quit");
                            std::process::exit(0);
                        }
                    }
                }
            }
        }
    }

    pub fn modify_all_windows_state(&self, activate: bool) {
        for win in self.workspace_manager.elements() {
            win.set_activated(activate);
            win.toplevel().unwrap().send_pending_configure();
        }
    }

    pub fn set_focus(&mut self, surface: WlSurface) {
        // set giving surface's root surface as focus
        let serial = SERIAL_COUNTER.next_serial();

        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return
            }
        };

        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }

        // unfocus all window
        self.modify_all_windows_state(false);

        keyboard.set_focus(
            self,
            Some(root),
            serial,
        );

        self.workspace_manager.set_focus(surface);
    }

    pub fn get_focus(&self) -> Option<WlSurface> {
        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return None
            }
        };

        keyboard.current_focus()
    }

}
