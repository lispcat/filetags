use std::{env::VarError, fs, path::PathBuf};

use anyhow::Context;
use regex::Regex;
use serde::{de::Error, Deserialize, Deserializer};
use shellexpand::LookupError;
use smart_default::SmartDefault;

use crate::args::Args;

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub settings: Settings,
    pub rules: Vec<Rule>,
}

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    #[default("rule")]
    pub name: String,

    #[default(vec![])]
    #[serde(deserialize_with = "expand_paths")]
    pub watch: Vec<PathBuf>,

    #[default(vec![])]
    #[serde(deserialize_with = "expand_paths")]
    pub dest: Vec<PathBuf>,

    #[default(vec![])]
    #[serde(with = "serde_regex")]
    pub regex: Vec<Regex>,

    pub settings: Option<Settings>,
}

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    #[default(true)]
    pub create_missing_directories: bool,

    #[default(vec![])]
    #[serde(with = "serde_regex")]
    pub exclude_pattern: Vec<Regex>,

    #[default(50)]
    pub max_depth: u32,

    #[default(false)]
    pub follow_symlinks: bool,
}

impl Config {
    pub fn new(args: &Args) -> anyhow::Result<Self> {
        let path: PathBuf = args.config_path.clone();
        let contents: String = read_file(&path)?;
        let config: Self = serde_yml::from_str(&contents)?;
        Ok(config)
    }
}

/// read file and return contents as String
fn read_file(contents: &PathBuf) -> anyhow::Result<String> {
    fs::read_to_string(contents).with_context(|| "failed to read config file")
}

// Deserializer shell expansions //////////////////////////////////////////////

/// Trait Extension for PathBuf for shell expansions
trait PathBufExpand {
    fn shell_expand(&self) -> Result<PathBuf, LookupError<VarError>>;
}

impl PathBufExpand for PathBuf {
    fn shell_expand(&self) -> Result<PathBuf, LookupError<VarError>> {
        self.to_str()
            .map(|s| -> Result<PathBuf, _> {
                Ok(PathBuf::from(shellexpand::full(s)?.into_owned()))
            })
            .unwrap_or_else(|| Ok(self.clone()))
    }
}

/// a custom deserializer for Vec<PathBuf> to expand tildes and variables
fn expand_paths<'de, D>(deserializer: D) -> Result<Vec<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let paths = Vec::<PathBuf>::deserialize(deserializer)?;
    paths
        .into_iter()
        .map(|p| -> Result<PathBuf, _> { p.shell_expand().map_err(D::Error::custom) })
        .collect()
}
