use std::{fs, path::Path, sync::Arc};

use anyhow::Context;
use tracing::info;

use crate::{enter_span, Config, Rule};

macro_rules! watch_dirs_indices {
    ($config:tt) => {
        $config.rules.iter().flat_map(|rule| {
            rule.watch_dirs
                .iter()
                .map(move |watch_dir| (rule, watch_dir))
        })
    };
}

/// Initialize directories and catch errors early to prevent mild catastrophes.
pub fn maybe_create_dirs_all(config: &Arc<Config>) -> anyhow::Result<()> {
    let _span = enter_span!(DEBUG, "init_dirs");

    for (rule, watch_dir) in watch_dirs_indices!(config) {
        if !watch_dir.try_exists()? {
            handle_missing_dir(watch_dir, rule, config)?;
        }
    }

    Ok(())
}

fn handle_missing_dir(dir_path: &Path, rule: &Rule, _config: &Arc<Config>) -> anyhow::Result<()> {
    if rule.settings.create_missing_directories {
        info!(?dir_path, "creating directory and parents");
        fs::create_dir_all(dir_path).with_context(|| format!("creating dir: {:?}", dir_path))?;
    } else {
        anyhow::bail!("path does not exist: {:?}", dir_path);
    }
    Ok(())
}
