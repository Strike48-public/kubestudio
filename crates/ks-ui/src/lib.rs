//! KubeStudio UI library
//!
//! This module exports the main App component and supporting modules
//! for use in both desktop and connector modes.

pub mod app;
pub mod components;
pub mod hooks;
#[cfg(feature = "connector")]
pub mod ipc;
pub mod session;
pub mod theme;
pub mod utils;

pub use app::App;
