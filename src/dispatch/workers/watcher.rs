use std::{future::Future, sync::Arc, time::Duration};

use anyhow::Context;
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
};
use tokio::task::JoinHandle;
use tracing::debug;

use crate::{match_event_kinds, watch_dir_indices, Config, Sender};

use crate::Message;

/// Used in `Message::NotifyEvent(NotifyEvent)`.
/// Provides needed additional info for the responder and its invoked symlinker actions.
#[derive(Clone, Debug)]
pub struct NotifyEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

/// Create and start the INotifyWatchers.
pub fn start_watchers(
    tx: &Sender<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<Vec<JoinHandle<anyhow::Result<()>>>> {
    Ok(create_watcher_closures(tx, config)?
        .into_iter()
        .map(|future| tokio::spawn(future))
        .collect::<Vec<_>>())
}

/// Create and return a Vec of closures of Watchers.
fn create_watcher_closures(
    tx: &Sender<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<Vec<impl Future<Output = anyhow::Result<()>>>> {
    watch_dir_indices(config)
        .map(|(rule_idx, watch_idx)| -> anyhow::Result<_> {
            let mut watcher = create_watcher(tx.clone(), rule_idx, watch_idx)?;
            let path = config.rules[rule_idx].watch_dirs[watch_idx].clone();
            Ok(async move {
                // start the watcher at the path
                watcher.watch(path.as_path(), RecursiveMode::Recursive)?;
                // keep it alive
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()
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
                match tx.send(Message::NotifyEvent(NotifyEvent {
                    rule_idx,
                    watch_idx,
                    event,
                })) {
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
