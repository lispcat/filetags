pub mod actions;
pub mod responder;
pub mod watcher;

use std::sync::Arc;

use actions::{
    cleaning::{clean_and_symlink_all, clean_dir},
    fs_asserts::maybe_create_dirs_all,
    symlinking::handle_event_message,
};
use anyhow::Context;
use notify::Event;

use crate::Config;

// Message ////////////////////////////////////////////////////////////////////

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    CreateNecessaryDirs,
    Watch(WatchEvent),
    CleanDir(usize, usize),
    CleanAll,
    Shutdown,
}

/// Used it `Message::Watch(WatchEvent)`.
/// Provides needed additional info for the responder and its invoked symlinker actions.
#[derive(Clone, Debug)]
pub struct WatchEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

// Message Handling ///////////////////////////////////////////////////////////

/// Responds to each Message variant appropriately.
pub fn handle_message(message: &Message, config: &Arc<Config>) -> anyhow::Result<Option<Signal>> {
    match message {
        Message::CreateNecessaryDirs => maybe_create_dirs_all(config)?,
        Message::Watch(event) => handle_event_message(config, event)?,
        Message::CleanAll => clean_and_symlink_all(config).context("cleaning all")?,
        Message::CleanDir(rule_idx, link_idx) => clean_dir(config, *rule_idx, *link_idx)?,
        Message::Shutdown => return Ok(Some(Signal::ShutdownSignal)),
    }
    Ok(None)
}

// Signal /////////////////////////////////////////////////////////////////////

/// Typically wrapped in an Option<T> to provide additional info to the caller function.
#[derive(Clone, Debug)]
pub enum Signal {
    ShutdownSignal,
}
