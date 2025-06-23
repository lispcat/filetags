use std::{sync::Arc, thread, time::Duration};

use anyhow::Context;
use crossbeam_channel::Sender;

use crate::{clone_vars, Config, Message};

/// Start symlink cleaners.
pub fn start_periodic_cleaners(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    let _cleaner_handles = collect_cleaner_closures(tx, config)?
        .into_iter()
        .map(thread::spawn)
        .collect::<Vec<_>>();

    Ok(())
}

fn collect_cleaner_closures(
    tx: &Sender<Message>,
    config: &Arc<Config>,
) -> anyhow::Result<Vec<impl FnOnce() -> anyhow::Result<()> + Send + 'static>> {
    Ok(config
        .rules
        .iter()
        .enumerate()
        .filter_map(|(rule_idx, rule)| {
            if let Some(clean_interval) = rule.settings.clean_interval {
                clone_vars!(tx, (config: Arc));
                Some(move || -> anyhow::Result<()> {
                    periodic_cleaner_process(config, rule_idx, tx, clean_interval)
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>())
}

fn periodic_cleaner_process(
    config: Arc<Config>,
    rule_idx: usize,
    tx: Sender<Message>,
    clean_interval: u32,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    loop {
        thread::sleep(Duration::from_secs(clean_interval.into()));
        for link_idx in 0..rule.link_dirs.len() {
            tx.send(Message::SymlinkCleanDir(rule_idx, link_idx))
                .context("sending message CleanDir")?;
        }
    }
}

// fn start_symlink_cleaner_for_each_rule(
//     tx: &Sender<Message>,
//     config: &Arc<Config>,
//     barrier: &Arc<Barrier>,
// ) -> anyhow::Result<()> {
//     for (rule_idx, rule) in config.rules.iter().enumerate() {
//         if let Some(clean_interval) = rule.settings.clean_interval {
//             clone_vars!(tx, barrier, (config: Arc));
//             thread::spawn(move || -> anyhow::Result<()> {
//                 barrier.wait();
//                 symlink_cleaner_worker(config, rule_idx, tx, clean_interval)
//             });
//         }
//     }
//     Ok(())
// }
