use std::{
    fs::{self},
    path::Path,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use anyhow::Context;
use crossbeam_channel::Sender;
use tracing::debug;
use walkdir::WalkDir;

use crate::{
    clone_vars, delete_symlink, link_dir_indices, path_is_rec_subdir_of_any, symlink_target,
    utils::path_matches_any_regex, Config, Message, Rule,
};

pub fn start_cleaners(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    // set up barrier with total sum of TODO
    let barrier = Arc::new(Barrier::new(config.rules.iter().len() + 1));

    // start an async cleaner for each rule
    start_cleaner_for_each_rule(tx, config, &barrier)?;

    // pause execution until all cleaners started
    barrier.wait();
    debug!("Cleaner Barrier passed!");

    Ok(())
}

fn start_cleaner_for_each_rule(
    tx: &Sender<Message>,
    config: &Arc<Config>,
    barrier: &Arc<Barrier>,
) -> anyhow::Result<()> {
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        if let Some(clean_interval) = rule.settings.clean_interval {
            clone_vars!((Arc :: config => arc_config), tx, barrier);
            thread::spawn(move || -> anyhow::Result<()> {
                start_cleaner(arc_config, rule_idx, tx, barrier, clean_interval)
            });
        }
    }
    Ok(())
}

fn start_cleaner(
    config: Arc<Config>,
    rule_idx: usize,
    tx: Sender<Message>,
    barrier: Arc<Barrier>,
    clean_interval: u32,
) -> anyhow::Result<()> {
    barrier.wait();
    loop {
        let rule = &config.rules[rule_idx];
        for link_idx in 0..rule.link_dirs.len() {
            tx.send(Message::CleanDir(rule_idx, link_idx))
                .context("sending message CleanDir")?;
        }

        thread::sleep(Duration::from_secs(clean_interval.into()));
    }
}

pub fn clean_dir(config: &Arc<Config>, rule_idx: usize, link_idx: usize) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let link_dir = &rule.link_dirs[link_idx];

    for entry in WalkDir::new(link_dir) {
        let entry = entry?;
        let path = entry.path();

        // get file metadata
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("performing metadata call on path: {:?}", path))?;

        // if is symlink, check if valid. if not, delete
        if metadata.file_type().is_symlink() && invalid_symlink(path, rule)? {
            delete_symlink(path, &metadata)?;
        }
    }

    Ok(())
}

pub fn invalid_symlink(path: &Path, rule: &Rule) -> anyhow::Result<bool> {
    // pattern doesnt match any regex
    if !path_matches_any_regex(path, &rule.regex).context("matching regexes")? {
        return Ok(true);
    }

    if let Some(symlink_target) = symlink_target(path).context("getting symlink target")? {
        // symlink_target is not a subdir of any watch_dir
        if !path_is_rec_subdir_of_any(&symlink_target, &rule.watch_dirs)? {
            return Ok(true);
        }
    } else {
        // symlink target unreachable, broken
        return Ok(true);
    }

    Ok(false)
}

// clean_dir for every link_dir in config.
pub fn clean_all(config: &Arc<Config>) -> anyhow::Result<()> {
    for (rule_idx, link_idx) in link_dir_indices(config) {
        clean_dir(config, rule_idx, link_idx)?;
    }

    Ok(())
}
