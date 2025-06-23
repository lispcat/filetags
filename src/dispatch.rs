use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};
use notify::Event;
use symlinks::{
    cleaning::{clean_all, clean_dir},
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
    pub worker_handles: Vec<JoinHandle<anyhow::Result<()>>>,
}

impl Dispatcher {
    /// Creates a new Dispatcher from a crossbeam channel.
    pub fn new(
        rx: Receiver<Message>,
        tx: Sender<Message>,
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            rx_handle: Self::start_rx(rx, Arc::clone(&config)).context("starting rx")?,
            tx,
            config,
            worker_handles: vec![],
        })
    }

    /// Starts the responder queue.
    /// For each Message it receives through rx, it handles it through `handle_message`.
    fn start_rx(
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

    /// A buildable method for invoking an `Action` using the dispatcher.
    pub fn run(self, action: Action) -> anyhow::Result<Self> {
        self.tx
            .send(Message::Action(action))
            .context("sending message")?;
        Ok(self)
    }

    /// Responds to each Message variant received. Invoked from `start_rx`.
    fn handle_message(message: &Message, config: &Arc<Config>) -> anyhow::Result<Option<Signal>> {
        match message {
            Message::Shutdown => return Ok(Some(Signal::ShutdownSignal)),
            Message::NotifyEvent(event) => {
                handle_notify_event(config, event).context("handling notify event")?
            }
            Message::Action(action) => match action {
                Action::CleanAll => {
                    clean_all(config).context("cleaning all")?;
                }
                Action::CleanDir(rule_idx, link_idx) => {
                    clean_dir(config, *rule_idx, *link_idx).context("cleaning dir")?
                }
                Action::MakeNecessaryDirs => {
                    make_necessary_dirs(config).context("making necessary dirs")?;
                }
                Action::SymlinkAll => {
                    symlink_create_all(config).context("maybe symlinking all")?;
                }
            },
        }
        Ok(None)
    }

    /// A buildable method for launching a `WorkerType`.
    /// Every worker thread that it launches will be appended to `self.worker_handles`.
    pub fn launch(mut self, launch: WorkerType) -> anyhow::Result<Self> {
        let mut new_handles = match launch {
            WorkerType::Cleaners => start_periodic_cleaners(&self.tx, &self.config)
                .context("starting periodic cleaners")?,
            WorkerType::Watchers => {
                start_watchers(&self.tx, &self.config).context("starting watchers")?
            }
        };
        // append the new worker handles to `self.worker_handles`
        self.worker_handles.append(&mut new_handles);

        Ok(self)
    }
}

/// Returned from `handle_message` for additional actions to take.
#[derive(Clone, Debug)]
enum Signal {
    ShutdownSignal,
}
