use std::{
    fmt::Debug,
    fs,
    path::Path,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use anyhow::Context;
use crossbeam_channel::Sender;
use walkdir::WalkDir;

use crate::{
    channels::symlinking::handle_path,
    get_setting,
    utils::{is_symlink_valid, path_is_rec_subdir_of_any, path_matches_any_regex},
    Config, Rule,
};

use super::Message;

pub fn start_cleaners(tx: &Sender<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    // set up barrier with total sum of TODO
    let barrier = Arc::new(Barrier::new(config.rules.iter().len() + 1));

    // start an async cleaner for each rule
    start_cleaner_for_each_rule(tx, config, &barrier)?;

    // pause execution until all cleaners started
    barrier.wait();
    eprintln!("Cleaner Barrier passed!");

    Ok(())
}

fn start_cleaner_for_each_rule(
    tx: &Sender<Message>,
    config: &Arc<Config>,
    barrier: &Arc<Barrier>,
) -> anyhow::Result<()> {
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        if let Some(clean_interval) = get_setting!(config, rule, clean_interval) {
            let config_arc = Arc::clone(config);
            let tx_clone: Sender<Message> = tx.clone();
            let barrier_clone = barrier.clone();
            thread::spawn(move || -> anyhow::Result<()> {
                start_cleaner(
                    config_arc,
                    rule_idx,
                    tx_clone,
                    barrier_clone,
                    clean_interval,
                )
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
        for (watch_idx, _watch) in rule.watch.iter().enumerate() {
            // TODO: instead of directly cleaning, send an event message instead, to prevent race conditions!!!
            // clean_dir(watch, &config.rules[rule_idx])?;
            tx.send(Message::CleanDir(rule_idx, watch_idx))
                .context("failed to send message for clean dir")?;
        }

        thread::sleep(Duration::from_secs(clean_interval.into()));
    }
}

pub fn clean_dir(config: &Arc<Config>, rule_idx: usize, dest_idx: usize) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let dir = &rule.dest[dest_idx];

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();

        // get file metadata
        let metadata = fs::symlink_metadata(path).with_context(|| {
            format!(
                "could not perform metadata call on path or path does not exist: {:?}",
                path
            )
        })?;

        // skip this file if not a symlink
        if !metadata.file_type().is_symlink() {
            eprintln!("This file is not a symlink, skip: {:?}", path);

            continue;
        }

        // if file doesnt match any regex, it should't belong here... probably...
        if !path_matches_any_regex(path, &rule.regex).context("failed to match regexes")? {
            eprintln!(
                "Symlink doesn't match any regex, so deleting symlink i guess: {:?}",
                path
            );
            fs::remove_file(path)?;

            continue;
        }

        // if symlink is broken, delete!
        if !is_symlink_valid(path).context("failed to check if valid symlink")? {
            eprintln!("Symlink is broken, so deleting symlink: {:?}", path);
            fs::remove_file(path)?;

            continue;
        }

        // if symlink is not a subdir of any watch dir, delete
        if !path_is_rec_subdir_of_any(path, &rule.watch)? {
            eprintln!(
                "Symlink is not a subdir of any watch dirs, so deleting symlink: {:?}",
                path
            );
            fs::remove_file(path)?;

            continue;
        }

        eprintln!("Existing symlink looks good!: {:?}", path);
    }
    eprintln!("cleanup of dest_dir complete!: {:?}", dir);

    Ok(())
}

pub fn clean_all_dest(config: &Arc<Config>) -> anyhow::Result<()> {
    // - walk throgh every dir path recursively with WalkDir...
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        for (dest_idx, _dest) in rule.dest.iter().enumerate() {
            clean_dir(config, rule_idx, dest_idx)?;
        }
        eprintln!("cleanup of dest_dirs in rule complete!: {}", rule.name);
    }
    eprintln!("cleanup of all rules complete!");

    // TODO: do symlinks to all matching...
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        for (watch_idx, watch) in rule.watch.iter().enumerate() {
            for direntry in WalkDir::new(watch) {
                handle_path(config, direntry.unwrap().path(), rule_idx, watch_idx)?;
            }
        }
    }

    Ok(())
}
