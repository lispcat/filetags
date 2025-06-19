pub mod actions;
pub mod responder;
pub mod watcher;

use std::sync::Arc;

use actions::{
    cleaning::{clean_all, clean_dir},
    fs_asserts::maybe_create_dirs_all,
    symlinking::{handle_event_message, maybe_symlink_all},
};
use anyhow::Context;
use watcher::WatchEvent;

use crate::Config;

// Message ////////////////////////////////////////////////////////////////////

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    CreateNecessaryDirs,
    CleanAll,
    MaybeSymlinkAll,
    CleanDir(usize, usize),
    Watch(WatchEvent),
    Shutdown,
}

// Message Handling ///////////////////////////////////////////////////////////

/// Responds to each Message variant appropriately.
pub fn handle_message(message: &Message, config: &Arc<Config>) -> anyhow::Result<Option<Signal>> {
    match message {
        Message::CreateNecessaryDirs => maybe_create_dirs_all(config)?,
        Message::CleanAll => clean_all(config).context("cleaning all")?,
        Message::MaybeSymlinkAll => maybe_symlink_all(config).context("maybe symlinking all")?,
        Message::CleanDir(rule_idx, link_idx) => clean_dir(config, *rule_idx, *link_idx)?,
        Message::Watch(event) => handle_event_message(config, event)?,
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
