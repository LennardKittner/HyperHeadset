use std::cell::RefCell;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Barrier};
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;

use super::popup::{EqPopupController, PopupCommand, PopupState};
use super::TrayCommand;

/// GTK4-based popup controller. Holds a std mpsc Sender (which is Send+Sync).
pub struct Gtk4PopupController {
    tx: Sender<PopupCommand>,
}

impl EqPopupController for Gtk4PopupController {
    fn send(&self, cmd: PopupCommand) {
        let _ = self.tx.send(cmd);
    }
}

/// Spawn the GTK4 event loop on a dedicated thread and return a controller.
///
/// `command_tx` is used by the popup to send `TrayCommand`s back to the main loop.
/// Blocks until GTK is initialized (via barrier).
pub fn spawn_gtk_popup_thread(command_tx: Sender<TrayCommand>) -> Gtk4PopupController {
    let (popup_tx, popup_rx) = mpsc::channel::<PopupCommand>();
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();

    std::thread::Builder::new()
        .name("gtk4-popup".into())
        .spawn(move || {
            gtk4::init().expect("Failed to initialize GTK4");

            // Window and state, managed inside the GTK thread (not Send)
            let window: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
            let visible: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

            // Signal that GTK init is done
            barrier_clone.wait();

            // Poll the mpsc receiver from the GTK main loop via a timeout source
            let command_tx = command_tx;
            glib::timeout_add_local(Duration::from_millis(50), move || {
                while let Ok(cmd) = popup_rx.try_recv() {
                    match cmd {
                        PopupCommand::Show { x: _, y: _, state } => {
                            if *visible.borrow() {
                                // Toggle off
                                if let Some(ref w) = *window.borrow() {
                                    w.set_visible(false);
                                }
                                *visible.borrow_mut() = false;
                            } else {
                                // Destroy old window if any
                                if let Some(ref w) = *window.borrow() {
                                    w.destroy();
                                }
                                let w = build_popup_window(&state, &command_tx);

                                // Focus-out handler: hide on focus loss
                                let vis = visible.clone();
                                let w_weak = w.downgrade();
                                let focus_ctl = gtk4::EventControllerFocus::new();
                                focus_ctl.connect_leave(move |_| {
                                    if let Some(w) = w_weak.upgrade() {
                                        w.set_visible(false);
                                    }
                                    *vis.borrow_mut() = false;
                                });
                                w.add_controller(focus_ctl);

                                w.set_default_size(250, -1);
                                w.present();
                                *window.borrow_mut() = Some(w);
                                *visible.borrow_mut() = true;
                            }
                        }
                        PopupCommand::Hide => {
                            if let Some(ref w) = *window.borrow() {
                                w.set_visible(false);
                            }
                            *visible.borrow_mut() = false;
                        }
                        PopupCommand::UpdateState(state) => {
                            if *visible.borrow() {
                                // Rebuild in place
                                if let Some(ref w) = *window.borrow() {
                                    w.destroy();
                                }
                                let w = build_popup_window(&state, &command_tx);

                                let vis = visible.clone();
                                let w_weak = w.downgrade();
                                let focus_ctl = gtk4::EventControllerFocus::new();
                                focus_ctl.connect_leave(move |_| {
                                    if let Some(w) = w_weak.upgrade() {
                                        w.set_visible(false);
                                    }
                                    *vis.borrow_mut() = false;
                                });
                                w.add_controller(focus_ctl);

                                w.set_default_size(250, -1);
                                w.present();
                                *window.borrow_mut() = Some(w);
                            }
                        }
                    }
                }
                glib::ControlFlow::Continue
            });

            let main_loop = glib::MainLoop::new(None::<&glib::MainContext>, false);
            main_loop.run();
        })
        .expect("Failed to spawn GTK4 popup thread");

    barrier.wait();
    Gtk4PopupController { tx: popup_tx }
}

use std::rc::Rc;

fn build_popup_window(state: &PopupState, command_tx: &Sender<TrayCommand>) -> gtk4::Window {
    let window = gtk4::Window::builder()
        .title("EQ Preset")
        .decorated(false)
        .resizable(false)
        .deletable(false)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    vbox.set_margin_top(8);
    vbox.set_margin_bottom(8);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    // Header label
    let header = gtk4::Label::new(Some("EQ Preset"));
    header.add_css_class("heading");
    vbox.append(&header);

    let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    vbox.append(&separator);

    // Radio group for presets
    let mut first_button: Option<gtk4::CheckButton> = None;

    for preset_name in &state.presets {
        let label_text = if !state.synced
            && state.active_preset.as_deref() == Some(preset_name.as_str())
        {
            format!("{} (applying...)", preset_name)
        } else {
            preset_name.clone()
        };

        let button = gtk4::CheckButton::with_label(&label_text);
        button.set_sensitive(state.is_connected);

        // Set as active if it's the selected preset
        if state.active_preset.as_deref() == Some(preset_name.as_str()) {
            button.set_active(true);
        }

        // Join radio group
        if let Some(ref first) = first_button {
            button.set_group(Some(first));
        } else {
            first_button = Some(button.clone());
        }

        // Click handler: send command to main loop
        let tx = command_tx.clone();
        let name = preset_name.clone();
        button.connect_toggled(move |btn| {
            if btn.is_active() {
                let _ = tx.send(TrayCommand::ApplyEqPreset(name.clone()));
                // Update own label to show applying state
                btn.set_label(Some(&format!("{} (applying...)", name)));
            }
        });

        vbox.append(&button);
    }

    if !state.is_connected {
        let status = gtk4::Label::new(Some("Headset not connected"));
        status.add_css_class("dim-label");
        status.set_margin_top(4);
        vbox.append(&status);
    }

    window.set_child(Some(&vbox));
    window
}
