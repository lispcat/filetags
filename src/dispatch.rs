use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};
use notify::Event;
use symlinks::{
    cleaning::{symlink_clean_all, symlink_clean_dir},
    filesystem::make_necessary_dirs,
    symlinking::{handle_notify_event, symlink_create_all},
    Action,
};
use workers::{periodic_cleaner::start_periodic_cleaners, watcher::start_watchers, WorkerType};

use crate::Config;

pub mod symlinks;
pub mod workers;

// Message ////////////////////////////////////////////////////////////////////

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    SymlinkCleanDir(usize, usize),
    NotifyEvent(NotifyEvent),
    Shutdown,
    Action(Action),
}

/// Used in `Message::NotifyEvent(NotifyEvent)`.
/// Provides needed additional info for the responder and its invoked symlinker actions.
#[derive(Clone, Debug)]
pub struct NotifyEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

// Dispatcher /////////////////////////////////////////////////////////////////

/// Used for action invocations and launching workers.
/// Spawns the responder thread when `new()` method is ran.
#[derive(Debug)]
pub struct Dispatcher {
    pub rx_handle: JoinHandle<anyhow::Result<()>>,
    pub tx: Sender<Message>,
    pub config: Arc<Config>,
}

impl Dispatcher {
    pub fn new(
        rx: Receiver<Message>,
        tx: Sender<Message>,
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            rx_handle: Self::start_responder(rx, Arc::clone(&config))?,
            tx,
            config,
        })
    }

    fn start_responder(
        rx: Receiver<Message>,
        config: Arc<Config>,
    ) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
        Ok(thread::spawn(move || -> anyhow::Result<()> {
            loop {
                let message = rx.recv().context("Responder waiting for Message")?;
                let maybe_signal =
                    Self::handle_message(&message, &config).context("handling message")?;
                if let Some(signal) = maybe_signal {
                    match signal {
                        Signal::ShutdownSignal => break Ok(()),
                    }
                }
            }
        }))
    }

    /// Responds to each Message variant appropriately.
    fn handle_message(message: &Message, config: &Arc<Config>) -> anyhow::Result<Option<Signal>> {
        match message {
            Message::SymlinkCleanDir(rule_idx, link_idx) => {
                symlink_clean_dir(config, *rule_idx, *link_idx)?
            }
            Message::NotifyEvent(event) => handle_notify_event(config, event)?,
            Message::Shutdown => return Ok(Some(Signal::ShutdownSignal)),
            Message::Action(action) => match action {
                Action::CleanAll => {
                    symlink_clean_all(config).context("cleaning all")?;
                }
                Action::MakeNecessaryDirs => {
                    make_necessary_dirs(config)?;
                }
                Action::SymlinkAll => {
                    symlink_create_all(config).context("maybe symlinking all")?;
                }
            },
        }
        Ok(None)
    }

    pub fn run(&self, action: Action) -> anyhow::Result<&Self> {
        self.tx
            .send(Message::Action(action))
            .context("sending message")?;

        Ok(self)
    }

    pub fn launch(&self, launch: WorkerType) -> anyhow::Result<&Self> {
        match launch {
            WorkerType::Cleaners => start_periodic_cleaners(&self.tx, &self.config)?,
            WorkerType::Watchers => {
                start_watchers(&self.tx, &self.config).context("starting watchers")?
            }
            WorkerType::Responder => {
                anyhow::bail!("cannot launch responder, since it should already be launched")
            }
        }

        Ok(self)
    }
}

/// Returned from `handle_message` for additional actions to take.
#[derive(Clone, Debug)]
enum Signal {
    ShutdownSignal,
}
