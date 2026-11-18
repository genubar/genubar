//! Entry point for `libgenubar_gtk4_hook.so` (`LD_PRELOAD` module for GTK4 / GTK3 / Libadwaita applications).
#![allow(clippy::missing_safety_doc, clippy::too_many_arguments)]

mod action_dispatcher;
mod action_states;
pub mod bridge;
mod button_heuristics;
mod debug_dump;
pub mod exporter;
pub mod ffi;
pub mod gi_hook;
pub mod hooks;
pub mod instance_guard;
pub mod logger;
mod menu_extractor;
pub mod scanner;
pub mod state;
pub mod theme_zoom;
mod variant_utils;
mod widget_tree;

pub use hooks::*;
pub use scanner::rescan_active_window;
