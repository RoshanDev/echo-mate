// EchoMate - AI Reply Copilot for WeChat

pub mod app;
pub mod platform;
pub mod agent;
pub mod provider;
pub mod store;
pub mod memory;
pub mod security;
pub mod ui;
pub mod domain;

// Re-export run for main.rs
pub use app::run;
