use clap::Parser;
use smart_default::SmartDefault;
use std::{env, path::PathBuf};

#[derive(Debug, Parser, SmartDefault)]
#[command(author, version, about)]
pub struct Args {
    #[arg(
        short,
        long = "config",
        default_value_os_t = default_config_path(),
    )]
    /// Path to config file
    pub config_path: PathBuf,
}

fn default_config_path() -> PathBuf {
    let config_dir = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|p| p.join(".config"))
                .expect("Cannot find HOME directory")
        });

    config_dir.join("filetags").join("config.yml")
}
