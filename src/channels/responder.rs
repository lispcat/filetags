use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Context;
use crossbeam_channel::Receiver;

use crate::{clone_vars, Config};

use super::{
    actions::{
        cleaning::{symlink_clean_all, symlink_clean_dir},
        filesystem_asserts::create_necessary_dirs,
        symlinking::{handle_notify_event, symlink_create_all},
    },
    Message,
};

// Message Handling ///////////////////////////////////////////////////////////

/// Start the responder in a new thread.
pub fn start_responder(
    rx: Receiver<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
    clone_vars!((Arc :: config => arc_config));
    let responder_handle = thread::spawn(move || -> anyhow::Result<()> {
        loop {
            let message = rx.recv().context("Responder waiting for Message")?;

            // handle the received message. may sometimes return a Signal enum.
            if let Some(signal) =
                handle_message(&message, &arc_config).context("handling message")?
            {
                match signal {
                    Signal::ShutdownSignal => break Ok(()),
                }
            };
        }
    });

    Ok(responder_handle)
}

/// Responds to each Message variant appropriately.
fn handle_message(message: &Message, config: &Arc<Config>) -> anyhow::Result<Option<Signal>> {
    match message {
        Message::CreateNecessaryDirs => create_necessary_dirs(config)?,
        Message::SymlinkCleanAll => symlink_clean_all(config).context("cleaning all")?,
        Message::SymlinkCleanDir(rule_idx, link_idx) => {
            symlink_clean_dir(config, *rule_idx, *link_idx)?
        }
        Message::SymlinkCreateAll => symlink_create_all(config).context("maybe symlinking all")?,
        Message::Watch(event) => handle_notify_event(config, event)?,
        Message::Shutdown => return Ok(Some(Signal::ShutdownSignal)),
    }
    Ok(None)
}

/// Returned from `handle_message` for additional actions to take.
#[derive(Clone, Debug)]
enum Signal {
    ShutdownSignal,
}
