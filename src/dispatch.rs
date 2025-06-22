use std::{sync::Arc, thread::JoinHandle};

use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};
use notify::Event;
use symlinks::Action;
use workers::{
    periodic_cleaner::start_symlink_cleaners, responder::start_responder, watcher::start_watchers,
    WorkerType,
};

use crate::{clone_vars, Config};

pub mod symlinks;
pub mod workers;

// Message ////////////////////////////////////////////////////////////////////

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    SymlinkCleanDir(usize, usize),
    NotifyEvent(WatchEvent),
    Shutdown,
    Action(Action),
}

/// Used in `Message::Watch(WatchEvent)`.
/// Provides needed additional info for the responder and its invoked symlinker actions.
#[derive(Clone, Debug)]
pub struct WatchEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}

// Dispatcher /////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Dispatcher {
    pub handle: JoinHandle<anyhow::Result<()>>,
    pub tx: Sender<Message>,
    pub config: Arc<Config>,
}

impl Dispatcher {
    pub fn new(
        rx: Receiver<Message>,
        tx: Sender<Message>,
        config: &Arc<Config>,
    ) -> anyhow::Result<Self> {
        clone_vars!((Arc :: config => config));
        Ok(Self {
            handle: start_responder(rx, &config)?,
            tx,
            config,
        })
    }

    pub fn run(&self, action: Action) -> anyhow::Result<()> {
        self.tx
            .send(Message::Action(action))
            .context("sending message")
    }

    pub fn launch(&self, launch: WorkerType) -> anyhow::Result<()> {
        match launch {
            WorkerType::SymlinkCleaners => start_symlink_cleaners(&self.tx, &self.config),
            WorkerType::Watchers => {
                start_watchers(&self.tx, &self.config).context("starting watchers")
            }
            WorkerType::Responder => {
                anyhow::bail!("cannot launch responder, since it should already be launched")
            }
        }
    }
}
