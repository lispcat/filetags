use std::{
    fs::{self},
    path::Path,
    sync::Arc,
};

use anyhow::Context;
use walkdir::WalkDir;

use crate::{
    delete_symlink, link_dir_indices, path_is_under_any_dirs, symlink_target,
    utils::path_matches_any_regex, Config, Rule,
};

// /// Shorthand for sending a query to the Receiver to `symlink_create_all`.
// pub fn query_symlink_clean_all(tx: &Sender<Message>) -> anyhow::Result<()> {
//     tx.send(Message::Action(Action::CleanAll))
//         .context("sending message")
// }

/// Runs `symlink_clean_dir` for every link_dir in config.
/// Ran from Receiver.
pub fn clean_all(config: &Arc<Config>) -> anyhow::Result<()> {
    for (rule_idx, link_idx) in link_dir_indices(config) {
        clean_dir(config, rule_idx, link_idx)?;
    }

    Ok(())
}

/// Recursively cleans symlinks at the specified link_dir.
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
        if metadata.file_type().is_symlink() && inappropriate_symlink(path, rule)? {
            delete_symlink(path, &metadata)?;
        }
    }

    Ok(())
}

/// Identifies whether the symlink at the path is invalid.
///
/// It checks the following:
/// - does the path match any of the regexes?
/// - is the symlink broken?
/// - is the symlink_target under any of the watch_dirs?
///
/// Note that this function does not check whether the file at the path is a symlink or not.
/// So do that validation beforehand.
fn inappropriate_symlink(path: &Path, rule: &Rule) -> anyhow::Result<bool> {
    // pattern doesnt match any regex
    if !path_matches_any_regex(path, &rule.regex).context("matching regexes")? {
        return Ok(true);
    }

    if let Some(symlink_target) = symlink_target(path).context("getting symlink target")? {
        // is symlink_target is not under any watch_dirs?
        if !path_is_under_any_dirs(&symlink_target, &rule.watch_dirs)? {
            return Ok(true);
        }
    } else {
        // symlink target unreachable, broken
        return Ok(true);
    }

    Ok(false)
}
