use std::{fs, path::Path, sync::Arc};

use anyhow::Context;
use tracing::{debug, info};

use crate::{enter_span, Config, Rule};

/// Initialize directories and catch errors early to prevent mild catastrophes.
pub fn maybe_create_dirs_all(config: &Arc<Config>) -> anyhow::Result<()> {
    let _span = enter_span!(DEBUG, "init_dirs");

    // for each watch_dir...
    for rule in &config.rules {
        for watch_dir in &rule.watch_dirs {
            // check if watch_dir doesn't exist.
            if !watch_dir.try_exists()? {
                debug!(?watch_dir, "path to watch not found");

                handle_missing_dir(watch_dir, rule, config)?;
            }
        }
    }
    Ok(())
}

fn handle_missing_dir(dir_path: &Path, rule: &Rule, _config: &Arc<Config>) -> anyhow::Result<()> {
    if rule.settings.create_missing_directories {
        info!(?dir_path, "creating directory and parents");
        fs::create_dir_all(dir_path)
            .with_context(|| format!("failed to create dir: {:?}", dir_path))?;
    } else {
        anyhow::bail!("path does not exist: {:?}", dir_path);
    }
    Ok(())
}
