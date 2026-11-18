//! Session and accessibility D-Bus protocol clients and Genubar daemon D-Bus server interfaces.

pub mod introspect;
pub mod models;
pub mod protocols;
pub mod server;
pub mod unified;
pub mod utils;

pub use introspect::DbusIntrospector;
pub use models::{ActionState, AtspiActionRef, CachedGtkAction, RegisteredWindow};
pub use protocols::canonical::CanonicalFetchOptions;
pub use server::{
    AppMenuRegistrar, MenuSignalSink, RegistrarBackend, TrackerBackend, WindowTrackerService,
};
pub use unified::UnifiedMenuService;
