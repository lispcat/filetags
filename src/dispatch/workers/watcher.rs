use std::{path::PathBuf, sync::Arc, thread, time::Duration};

use crossbeam_channel::{self, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
};
use tracing::debug;

use crate::{match_event_kinds, watch_dir_indices, Config, NotifyEvent};

use crate::Message;

/// Start the notify watchers.
pub fn start_watchers(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    let watchers: Vec<(INotifyWatcher, PathBuf)> = watch_dir_indices(config)
        .map(|(rule_idx, watch_idx)| -> anyhow::Result<_> {
            let path = config.rules[rule_idx].watch_dirs[watch_idx].clone();
            Ok((create_watcher(tx.clone(), rule_idx, watch_idx)?, path))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let _handles: Vec<_> = watchers
        .into_iter()
        .map(|(mut watcher, path)| {
            thread::spawn(move || -> anyhow::Result<()> {
                // start the watcher
                watcher.watch(path.as_path(), RecursiveMode::Recursive)?;
                // keep it alive
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            })
        })
        .collect();

    Ok(())
}

fn create_watcher(
    tx: Sender<Message>,
    rule_idx: usize,
    watch_idx: usize,
) -> anyhow::Result<INotifyWatcher> {
    let watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
        Ok(event) => match event.kind {
            // only send message if filename modification or file creation
            match_event_kinds!() => {
                let new_message = Message::NotifyEvent(NotifyEvent {
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
            debug!("WATCH ERROR! {}", e);
        }
    })?;
    Ok(watcher)
}
