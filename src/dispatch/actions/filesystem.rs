use std::{fs, path::Path, sync::Arc};

use anyhow::Context;
use tracing::info;

use crate::{link_dir_indices_with_refs, span_enter, Config, Rule};

// /// Shorthand for sending a query to the Receiver to create necessary dirs.
// pub fn query_create_necessary_dirs(tx: &Sender<Message>) -> anyhow::Result<()> {
//     tx.send(Message::Action(Action::MakeNecessaryDirs))
//         .context("sending message")
// }

/// Ensure all link_dirs exist
pub fn make_necessary_dirs(config: &Arc<Config>) -> anyhow::Result<()> {
    span_enter!(DEBUG, "init_dirs");

    for (_, _, rule, link_dir) in link_dir_indices_with_refs(config) {
        if !link_dir.try_exists()? {
            handle_missing_dir(link_dir, rule, config)?;
        }
    }

    Ok(())
}

/// If a link_dir doesn't exist, create it.
/// If rule.settings.create_missing_directories is false, crash program.
fn handle_missing_dir(dir_path: &Path, rule: &Rule, _config: &Arc<Config>) -> anyhow::Result<()> {
    if rule.settings.create_missing_dirs {
        info!(?dir_path, "creating directory and parents");
        fs::create_dir_all(dir_path).with_context(|| format!("creating dir: {:?}", dir_path))?;
    } else {
        anyhow::bail!("path does not exist: {:?}", dir_path);
    }
    Ok(())
}
