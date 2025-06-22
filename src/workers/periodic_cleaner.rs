use std::{
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use anyhow::Context;
use crossbeam_channel::Sender;

use crate::{clone_vars, sum_all_rules, with_barrier, Config, Message};

/// Start symlink cleaners.
pub fn start_symlink_cleaners(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    with_barrier!(sum_all_rules(config), |barrier| {
        start_symlink_cleaner_for_each_rule(tx, config, barrier)
    });

    Ok(())
}

fn start_symlink_cleaner_for_each_rule(
    tx: &Sender<Message>,
    config: &Arc<Config>,
    barrier: &Arc<Barrier>,
) -> anyhow::Result<()> {
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        if let Some(clean_interval) = rule.settings.clean_interval {
            clone_vars!(tx, barrier, (Arc :: config => arc_config));
            thread::spawn(move || -> anyhow::Result<()> {
                barrier.wait();
                symlink_cleaner_worker(arc_config, rule_idx, tx, clean_interval)
            });
        }
    }
    Ok(())
}

fn symlink_cleaner_worker(
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
