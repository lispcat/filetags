use std::{
    path::PathBuf,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Context;
use crossbeam_channel::{self, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
};
use tracing::debug;

use crate::{match_event_kinds, watch_dir_indices, Config};

use crate::Message;

/// Used in `Message::NotifyEvent(NotifyEvent)`.
/// Provides needed additional info for the responder and its invoked symlinker actions.
#[derive(Clone, Debug)]
pub struct NotifyEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

/// Create and return an INotifyWatcher. Don't start them just yet.
fn create_watcher(
    tx: Sender<Message>,
    rule_idx: usize,
    watch_idx: usize,
) -> anyhow::Result<INotifyWatcher> {
    notify::recommended_watcher(move |res: Result<Event, _>| match res {
        Ok(event) => {
            if let match_event_kinds!() = event.kind {
                let mesg = Message::NotifyEvent(NotifyEvent {
                    rule_idx,
                    watch_idx,
                    event,
                });
                match tx.send(mesg) {
                    Ok(_) => debug!("Watcher sent message!"),
                    Err(e) => debug!("WATCHER FAILED TO SEND MESSAGE: {:?}", e),
                }
            }
        }
        Err(e) => {
            debug!("WATCH ERROR! {}", e);
        }
    })
    .context("creating notify watcher")
}

/// Collect and start the INotifyWatchers.
pub fn start_watchers(
    tx: &Sender<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<Vec<JoinHandle<anyhow::Result<()>>>> {
    let watchers: Vec<(INotifyWatcher, PathBuf)> = watch_dir_indices(config)
        .map(|(rule_idx, watch_idx)| -> anyhow::Result<_> {
            let path = config.rules[rule_idx].watch_dirs[watch_idx].clone();
            Ok((create_watcher(tx.clone(), rule_idx, watch_idx)?, path))
        })
        .collect::<Result<Vec<(_, _)>, _>>()?;
    Ok(watchers
        .into_iter()
        .map(|(mut watcher, path)| {
            thread::spawn(move || -> anyhow::Result<()> {
                // start the watcher at the path
                watcher.watch(path.as_path(), RecursiveMode::Recursive)?;
                // keep it alive
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            })
        })
        .collect::<Vec<_>>())
}
