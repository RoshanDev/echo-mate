// EchoMate - AI Reply Copilot for WeChat

pub mod agent;
pub mod app;
pub mod domain;
pub mod memory;
pub mod platform;
pub mod provider;
pub mod security;
pub mod store;
pub mod ui;

// Re-export run for main.rs
pub use app::run;
