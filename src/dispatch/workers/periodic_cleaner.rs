use std::{future::Future, sync::Arc, time::Duration};

use anyhow::Context;
use tokio::task::JoinHandle;

use crate::{actions::Action, clone_vars, Config, Message, Sender};

/// Create and start symlink cleaners.
pub fn start_periodic_cleaners(
    tx: &Sender<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<Vec<JoinHandle<anyhow::Result<()>>>> {
    Ok(create_cleaner_closures(tx, config)?
        .into_iter()
        .map(|future| tokio::spawn(future))
        .collect::<Vec<_>>())
}

/// Create and return a Vec of closures of periodic cleaners.
fn create_cleaner_closures(
    tx: &Sender<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<Vec<impl Future<Output = anyhow::Result<()>>>> {
    Ok(config
        .rules
        .iter()
        .enumerate()
        .filter_map(|(rule_idx, rule)| {
            if let Some(clean_interval) = rule.settings.clean_interval {
                clone_vars!(rule, tx);
                Some(async move {
                    loop {
                        tokio::time::sleep(Duration::from_secs(clean_interval.into())).await;
                        for link_idx in 0..rule.link_dirs.len() {
                            tx.send(Message::Action(Action::CleanDir(rule_idx, link_idx)))
                                .context("sending message CleanDir")?;
                        }
                    }
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>())
}

// /// The periodic cleaner process. Meant to be run as a new thread.
// async fn periodic_cleaner_process(
//     rule_idx: usize,
//     tx: Sender<Message>,
//     clean_interval: u32,
//     config: Arc<Config>,
// ) -> anyhow::Result<()> {
//     let rule = &config.rules[rule_idx];
//     loop {
//         tokio::time::sleep(Duration::from_secs(clean_interval.into())).await;
//         for link_idx in 0..rule.link_dirs.len() {
//             tx.send(Message::Action(Action::CleanDir(rule_idx, link_idx)))
//                 .context("sending message CleanDir")?;
//         }
//     }
// }
