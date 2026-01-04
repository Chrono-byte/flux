//! Flux GUI - A graphical frontend for the Flux dotfiles manager.
//!
//! This crate provides a GPUI-based GUI for managing dotfiles.

#![warn(missing_docs)]
#![allow(missing_docs)]

pub mod app;
pub mod async_executor;
pub mod components;
pub mod events;
pub mod state;
pub mod styles;
pub mod utils;
pub mod views;

pub use app::FluxApp;
pub use state::AppView;
