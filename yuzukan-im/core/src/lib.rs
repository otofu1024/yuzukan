//! yuzukan-im: Japanese IME engine for the macOS frontend.
//!
//! - The macOS stdio JSON-RPC server lives in [`server`] and is built
//!   as the `yuzukan-imserver` binary, bundled inside `yuzukan-macos`.
//! - Key events use XKB keysyms on the wire.

pub mod config;
pub mod core;
pub mod server;

pub use core::engine::{EngineAction, EngineResult, InputMethodEngine};
pub use core::keycode::{KeyEvent, KeyModifiers, Keysym};
pub use core::state::InputState;
