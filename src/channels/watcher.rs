use std::{
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use crossbeam_channel::{self, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};
use tracing::debug;

use crate::{
    channels::WatchEvent, clone_vars, match_event_kinds, num_watch_dirs_for_config, Config, Message,
};

/// Set up watchers for each watch_dir
pub fn start_watchers(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    // set up barrier with total sum of watch dirs
    let barrier = Arc::new(Barrier::new(1 + num_watch_dirs_for_config(config)?));

    // start an async watcher for each watch_dir
    start_watchers_for_each_watch_dir(config, tx, &barrier)?;

    // pause execution until all watchers have started
    barrier.wait();

    Ok(())
}

/// For each watch dir, spawn a notify watcher, where every `notify::Event` the watcher creates
/// is forwarded to its corresponding crossbeam channel Receiver from the calling function.
///
/// Each watcher is physically started by running the `start_watcher` function.
pub fn start_watchers_for_each_watch_dir(
    config: &Arc<Config>,
    tx: &Sender<Message>,
    barrier: &Arc<Barrier>,
) -> anyhow::Result<()> {
    // start watcher for each watch_dir
    for rule_idx in 0..config.rules.len() {
        for watch_idx in 0..config.rules[rule_idx].watch_dirs.len() {
            clone_vars!(tx, barrier, (Arc :: config => arc_config));
            thread::spawn(move || -> anyhow::Result<()> {
                start_watcher(arc_config, rule_idx, watch_idx, tx, barrier)
            });
        }
    }
    Ok(())
}

/// Starts a notify watcher, where every `notify::Event` the watcher creates is forwarded
/// to its corresponding crossbeam channel Receiver.
///
/// This function has a never-ending loop at the end to keep the watcher alive.
/// This function is meant to be ran as a new thread, specifically in the function
/// `start_watchers_for_each_watch_dir`.
fn start_watcher(
    config: Arc<Config>,
    rule_idx: usize,
    watch_idx: usize,
    tx: Sender<Message>,
    barrier: Arc<Barrier>,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch_dirs[watch_idx];

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
        Ok(event) => match event.kind {
            // only send message if filename modification or file creation
            match_event_kinds!() => {
                let new_message = Message::Watch(WatchEvent {
                    rule_idx,
                    watch_idx,
                    event,
                });
                match tx.send(new_message) {
                    Ok(_) => debug!("Watcher sent message!"),
                    Err(e) => debug!("WATCHER FAILED TO SEND MESSAGE: {:?}", e),
                }
            }
            // for all other events do nothing
            _ => (),
        },
        Err(e) => {
            debug!("Watch Error! {}", e);
        }
    })?;

    debug!("Starting watcher at: {:?}", watch);
    watcher.watch(watch, RecursiveMode::Recursive)?;

    // watcher set up, increment barrier
    barrier.wait();

    // Keep the watcher alive - it will send events via the closure
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
