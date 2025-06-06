use std::{fs, os::unix::fs::symlink, path::Path};

use anyhow::Context;
use notify::{
    event::{ModifyKind, RenameMode},
    EventKind,
};

use crate::{
    match_event_kinds,
    utils::{calc_dest_link_from_src_orig, path_matches_any_regex},
    Config,
};

use super::WatchEvent;

/// Handle a `notify::Event` received from the crossbeam channel Receiver.
///
/// When a file creation or filename modification `notify::Event` is received,
/// run `handle_path` to check the filename and take action if needed.
pub fn handle_event_message(config: &Config, message: &WatchEvent) -> anyhow::Result<()> {
    match message.event.kind {
        match_event_kinds!() => {
            for check_path in &message.event.paths {
                let rule_idx = message.rule_idx;
                let watch_idx = message.watch_idx;
                handle_path(config, check_path, rule_idx, watch_idx)
                    .context("failed to handle path")?;
            }
        }
        _ => (),
    }
    Ok(())
}

/// Check if the filename of the path matches the specified Regex's, and take action if needed.
///
/// If it matches, create a symlink to the appropriate dest dir.
pub fn handle_path(
    config: &Config,
    src_path: &Path,
    rule_idx: usize,
    watch_idx: usize,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch[watch_idx];
    let regexes = &rule.regex;

    if path_matches_any_regex(src_path, regexes)? {
        eprintln!("Regex matches! {:?}", src_path);

        // For every dest_dir, check if the expected link_path has a symlink, and if not,
        // create one.
        for dest in &rule.dest {
            // ensure that the dest_dir exists
            anyhow::ensure!(
                dest.exists(),
                "Error: dest ({:?}) does not exist... was it deleted?",
                dest
            );

            // where the link_path should be
            let link_path = calc_dest_link_from_src_orig(src_path, watch, dest)?;

            if link_path.exists() {
                // file exists, so now check if it's a symlink and points to src_path
                ensure_is_symlink_and_expected_target(&link_path, src_path)?;
            } else {
                // file doesn't exist, so create a symlink to there
                symlink(src_path, link_path).context("failed to create symlink")?;
            }
        }
    }

    Ok(())
}

pub fn ensure_is_symlink_and_expected_target(
    link_path: &Path,
    src_path: &Path,
) -> anyhow::Result<()> {
    // something exists here, so ensure that the file at link_path is a symlink
    let is_symlink = fs::symlink_metadata(link_path)?.file_type().is_symlink();
    anyhow::ensure!(
        is_symlink,
        "Error: something already exists at link_path ({:?}) and it's not a symlink?!",
        link_path
    );

    // ensure the existing symlink points to the src_path
    let symlink_points_to_src = src_path == link_path;
    anyhow::ensure!(
        symlink_points_to_src,
        "Error: existing symlink at link_path ({:?}) doesn't point to src_path ({:?})",
        link_path,
        src_path
    );

    Ok(())
}
