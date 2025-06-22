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
    clone_vars, match_event_kinds, sum_all_watch_dirs, watch_dir_indices, with_barrier, Config,
    WatchEvent,
};

use super::Message;

/// Start the notify watchers.
pub fn start_watchers(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    with_barrier!(sum_all_watch_dirs(config), |barrier| {
        start_watchers_for_each_watch_dir(config, tx, barrier)
    });

    Ok(())
}

/// Spawn a notify watcher for each watch_dir.
pub fn start_watchers_for_each_watch_dir(
    config: &Arc<Config>,
    tx: &Sender<Message>,
    barrier: &Arc<Barrier>,
) -> anyhow::Result<()> {
    // start watcher for each watch_dir
    for (rule_idx, watch_idx) in watch_dir_indices(config) {
        clone_vars!(tx, barrier, (Arc :: config => arc_config));
        thread::spawn(move || -> anyhow::Result<()> {
            start_watcher(arc_config, rule_idx, watch_idx, tx, barrier)
        });
    }
    Ok(())
}

/// Starts a notify watcher.
/// For every Event that the notify watcher produces, it's forwarded to the Reciver.
///
/// This function is meant to be ran as a new thread.
fn start_watcher(
    config: Arc<Config>,
    rule_idx: usize,
    watch_idx: usize,
    tx: Sender<Message>,
    barrier: Arc<Barrier>,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch_dirs[watch_idx];

    // create notify watcher
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

    // wait until all watchers set up
    barrier.wait();

    // Keep the watcher alive
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
