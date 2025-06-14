use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use anyhow::Context;
use crossbeam_channel::Sender;
use tracing::debug;
use walkdir::WalkDir;

use crate::{
    channels::actions::symlinking::handle_path, clone_vars, path_is_rec_subdir_of_any,
    symlink_target, utils::path_matches_any_regex, Config, Message,
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

        // skip this file if not a symlink
        if !metadata.file_type().is_symlink() {
            debug!("This file is not a symlink, skip: {:?}", path);

            continue;
        }

        // if file doesnt match any regex, it should't belong here... probably...
        if !path_matches_any_regex(path, &rule.regex).context("matching regexes")? {
            debug!(
                "Symlink doesn't match any regex, so deleting symlink i guess: {:?}",
                path
            );
            fs::remove_file(path)?;
            continue;
        }

        // if symlink is broken, delete!
        let symlink_target = match symlink_target(path).context("getting symlink target")? {
            Some(p) => p,
            None => {
                debug!("Symlink is broken, so deleting symlink: {:?}", path);
                fs::remove_file(path)?;
                continue;
            }
        };

        // does symlink target exist?
        if !symlink_target.exists() {
            debug!(
                "Symlink target does not exist!!! {:?}, deleting symlink: {:?}",
                symlink_target, path
            );
            fs::remove_file(path)?;
            continue;
        }

        // if symlink target is not a subdir of any watch dir, delete symlink
        if !path_is_rec_subdir_of_any(&symlink_target, &rule.watch_dirs)? {
            debug!(
                "Symlink target is not a subdir of any watch dirs, so deleting symlink: {:?}",
                symlink_target,
            );
            fs::remove_file(path)?;
            continue;
        } else {
            // debug!("OH WOW, symlink_target is a subdir of watch dirs!: {:?}, {:?}",);
        }

        debug!("Existing symlink looks good!: {:?}", path);
    }
    debug!("cleanup of link_dir complete!: {:?}", link_dir);

    Ok(())
}

pub fn clean_and_symlink_all(config: &Arc<Config>) -> anyhow::Result<()> {
    // - walk throgh every dir path recursively with WalkDir...
    for rule_idx in 0..config.rules.len() {
        for link_idx in 0..config.rules[rule_idx].link_dirs.len() {
            clean_dir(config, rule_idx, link_idx)?;
        }
        debug!("cleanup of link_dirs in rule complete!");
    }
    debug!("cleanup of all rules complete!");

    // TODO: do symlinks to all matching...
    for rule_idx in 0..config.rules.len() {
        for (watch_idx, watch) in config.rules[rule_idx].watch_dirs.iter().enumerate() {
            for direntry in WalkDir::new(watch) {
                handle_path(config, direntry.unwrap().path(), rule_idx, watch_idx)?;
            }
        }
    }

    Ok(())
}
