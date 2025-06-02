pub mod symlinker;
pub mod watcher;

use std::{sync::Arc, thread};

use crossbeam_channel::{Receiver, Sender};
use notify::Event;
use symlinker::handle_message;
use watcher::start_watchers_for_each_watch_dir;

use crate::Config;

// TODO: enum variants
// - Watcher
// - Shutdown

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub struct Message {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

/// Set up watchers for each watch_dir
pub fn setup_watchers(config: &Arc<Config>, event_tx: &Sender<Message>) -> anyhow::Result<()> {
    // start an async watcher for each watch_dir
    start_watchers_for_each_watch_dir(config, event_tx)?;

    // return Receiver
    Ok(())
}

// TODO: make it so Message can be of multiple types, like cleanup dest_dir time,
// so both cleanups and symlinking is handled through here synchronously.
// TODO: Message types:
// - maybe_symlink (rule_idx, watch_idx, event)
// - clean_rule (rule_idx)
// - clean_dest (rule_idx, dest_idx)
// - shutdown
pub fn start_responder(rx: Receiver<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    let config_arc = Arc::clone(config);
    thread::spawn(move || -> anyhow::Result<()> {
        loop {
            match rx.recv() {
                Ok(event) => handle_message(&config_arc, &event)?,
                Err(e) => println!("ERROR received from thread: {:?}", e),
            }
        }
    });

    Ok(())
}
