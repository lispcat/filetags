pub mod cleaning;
pub mod symlinking;
pub mod watcher;

use std::{
    sync::{Arc, Barrier},
    thread::{self, JoinHandle},
};

use anyhow::Context;
use cleaning::{clean_and_symlink_all, clean_dir};
use crossbeam_channel::{Receiver, Sender};
use notify::Event;
use symlinking::handle_event_message;
use tracing::debug;
use watcher::start_watchers_for_each_watch_dir;

use crate::Config;

// TODO: enum variants?
// - CleanRule
// - CleanAll
// - DebugPrint
// - GetStatus

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    Watch(WatchEvent),
    CleanDir(usize, usize),
    CleanAll,
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct WatchEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

/// Set up watchers for each watch_dir
pub fn start_watchers(event_tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    // set up barrier with total sum of watch dirs
    let barrier = Arc::new(Barrier::new(
        config
            .rules
            .iter()
            .map(|rule| rule.watch_dirs.len())
            .sum::<usize>()
            + 1,
    ));

    // start an async watcher for each watch_dir
    start_watchers_for_each_watch_dir(config, event_tx, &barrier)?;

    // pause execution untill all watchers started
    barrier.wait();
    debug!("BARRIER PASSED!");

    Ok(())
}

pub fn start_responder(
    rx: Receiver<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
    let config_arc = Arc::clone(config);
    let handle = thread::spawn(move || -> anyhow::Result<()> {
        loop {
            match rx.recv().context("Error received from thread!")? {
                Message::Watch(event) => handle_event_message(&config_arc, &event)?,
                Message::CleanAll => {
                    clean_and_symlink_all(&config_arc).context("failed to clean all dest")?
                }
                Message::CleanDir(rule_idx, dest_idx) => {
                    clean_dir(&config_arc, rule_idx, dest_idx)?
                }
                Message::Shutdown => break,
            }
        }
        Ok(())
    });

    Ok(handle)
}
