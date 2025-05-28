use std::{default, env::VarError, fs, path::PathBuf};

use anyhow::Context;
use regex::Regex;
use serde::{de::Error, Deserialize, Deserializer};
use shellexpand::LookupError;
use smart_default::SmartDefault;

use crate::args::Args;

/// The settings field specifies a set of defaults for all rules.
/// The rules field specifies a list of rules.
///
#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub settings: Settings,
    pub rules: Vec<Rule>,
}

impl Config {
    pub fn new(args: &Args) -> anyhow::Result<Self> {
        let path: PathBuf = args.config_path.clone();
        let contents: String =
            fs::read_to_string(path).with_context(|| "failed to read config file")?;
        let config: Self = serde_yml::from_str(&contents)?;
        Ok(config)
    }
}

/// Each Rule most notably has a `watch` and `dest` field, where `watch` is a list of directories
/// to look for filename cookies for, and `dest` is a list of directories to create symlinks to.
///
/// The `settings` field specifies overrides to the global default settings set in the
/// Config struct.
///
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

    pub settings: RuleSettings,
}

/// The `Settings` struct is used to define the defaults for settings for all Rules.
///
/// Overrides on a per-Rule basis can be done in each instance of `RuleSettings` in each Rule.
///
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

/// A copy of the `Settings` struct except that every field is wrapped in an `Option<T>` and
/// defaults to `None`.
///
/// This is to be used inside every Rule.
///
/// The reason for the differentiation between `Settings` and `RuleSettings` is so that when
/// `RuleSettings` has a field with no explicitly set default value, it searches the `Settings`
/// struct.
///
#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleSettings {
    pub create_missing_directories: Option<bool>,
    #[serde(with = "serde_regex")]
    pub exclude_pattern: Option<Vec<Regex>>,
    pub max_depth: Option<u32>,
    pub follow_symlinks: Option<bool>,
}

/// Given a `Rule` and a setting name, try to get the setting value. If `None`, get the default
/// from Config.
///
#[macro_export]
macro_rules! get_setting {
    ($config:tt, $rule:tt, $name:tt) => {{
        $rule.settings.$name.unwrap_or($config.settings.$name)
    }};
}

// Deserializer shell expansions //////////////////////////////////////////////

/// Trait Extension for PathBuf for shell expansions.
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

/// a custom deserializer for `Vec<PathBuf>` to expand tildes and variables.
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
