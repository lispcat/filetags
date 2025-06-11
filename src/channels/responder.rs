use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Context;
use crossbeam_channel::Receiver;

use crate::Config;

use super::{
    actions::{
        cleaning::{clean_and_symlink_all, clean_dir},
        fs_asserts::maybe_create_dirs_all,
        symlinking::handle_event_message,
    },
    Message,
};

pub fn start_responder(
    rx: Receiver<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
    let config_arc = Arc::clone(config);
    let handle = thread::spawn(move || -> anyhow::Result<()> {
        loop {
            match rx.recv().context("Error received from thread!")? {
                Message::Shutdown => break,
                Message::MaybeCreateDirsAll => maybe_create_dirs_all(&config_arc)?,
                Message::Watch(event) => handle_event_message(&config_arc, &event)?,
                Message::CleanAll => {
                    clean_and_symlink_all(&config_arc).context("failed to clean all link")?
                }
                Message::CleanDir(rule_idx, link_idx) => {
                    clean_dir(&config_arc, rule_idx, link_idx)?
                }
            }
        }
        Ok(())
    });

    Ok(handle)
}
