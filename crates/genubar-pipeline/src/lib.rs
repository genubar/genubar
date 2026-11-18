//! Multi-stage menu transformation pipeline: layouts, rules, inheritance, sorting, icons, and Python hooks.

pub mod gtk4_layout;
pub mod gtk4_rules;
pub mod icon_matcher;
pub mod icons;
pub mod inherit;
pub mod mandatory;
pub mod orchestrator;
pub mod postprocess;
pub mod python;
pub mod sorter;
pub mod static_builder;

pub use orchestrator::{MenuPipeline, PipelineRuntimeConfig};
pub use postprocess::{
    apply_more_overflow, sanitize_separators, strip_action_items, strip_search_bars,
};
pub use python::{DEFAULT_SCRIPT_TIMEOUT_MS, PythonRunner, ScriptMetadata};
pub use static_builder::StaticMenuBuilder;
