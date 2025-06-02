use std::{sync::Arc, thread, time::Duration};

use crossbeam_channel::{self, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};

use crate::{channels::WatchEvent, match_event_kinds, Config, Message};

/// For each watch dir, spawn a notify watcher, where every `notify::Event` the watcher creates
/// is forwarded to its corresponding crossbeam channel Receiver from the calling function.
///
/// Each watcher is physically started by running the `start_watcher` function.
pub fn start_watchers_for_each_watch_dir(
    config: &Arc<Config>,
    tx: &Sender<Message>,
) -> anyhow::Result<()> {
    // start watcher for each watch_dir
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        for (watch_idx, _) in rule.watch.iter().enumerate() {
            let config_arc = Arc::clone(config);
            let tx_clone: Sender<Message> = tx.clone();
            // TODO: instead of simply cloning clone the sender and create a new instance of Message.
            thread::spawn(move || -> anyhow::Result<()> {
                start_watcher(config_arc, rule_idx, watch_idx, tx_clone)
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
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch[watch_idx];

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
        Ok(event) => match event.kind {
            // only send message if filename modification or file creation
            match_event_kinds!() => {
                let new_message = Message::Watch(WatchEvent {
                    rule_idx,
                    watch_idx,
                    event,
                });
                dbg!(&new_message);
                match tx.send(new_message) {
                    Ok(_) => println!("Watcher sent message!"),
                    Err(e) => println!("WATCHER FAILED TO SEND MESSAGE: {:?}", e),
                }
            }
            // for all other events do nothing
            _ => (),
        },
        Err(e) => {
            println!("Watch Error! {}", e);
        }
    })?;

    println!("Starting watcher at: {:?}", watch);
    watcher.watch(watch, RecursiveMode::Recursive)?;

    // Keep the watcher alive - it will send events via the closure
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
