use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Context;
use crossbeam_channel::Receiver;

use crate::Config;

use super::{handle_message, Message, Signal};

pub fn start_responder(
    rx: Receiver<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
    let arc_config = Arc::clone(config);
    let handle = thread::spawn(move || -> anyhow::Result<()> {
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

    Ok(handle)
}
