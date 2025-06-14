use std::{env::VarError, fs, path::PathBuf, sync::Arc};

use anyhow::Context;
use regex::Regex;
use serde::{de::Error, Deserialize, Deserializer};
use shellexpand::LookupError;
use smart_default::SmartDefault;

use crate::args::Args;

// Config /////////////////////////////////////////////////////////////////////

// Note: Deserialization impl is further down

#[derive(SmartDefault, Debug, Clone)]
pub struct Config {
    pub rules: Vec<Rule>,
}

impl Config {
    pub fn create(args: &Args) -> anyhow::Result<Arc<Self>> {
        let path: PathBuf = args.config_path.clone();
        let contents: String = fs::read_to_string(path).context("reading config file")?;
        let config: Self = serde_yml::from_str(&contents)?;
        Ok(Arc::new(config))
    }
}

// Rule ///////////////////////////////////////////////////////////////////////

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    #[default("rule")]
    pub name: String,

    #[default(vec![])]
    #[serde(deserialize_with = "expand_paths")]
    pub watch_dirs: Vec<PathBuf>,

    #[default(vec![])]
    #[serde(deserialize_with = "expand_paths")]
    pub link_dirs: Vec<PathBuf>,

    #[default(vec![])]
    #[serde(with = "serde_regex")]
    pub regex: Vec<Regex>,

    #[serde(rename = "settings")]
    pub raw_settings: Option<RawRuleSettings>,

    #[serde(skip)]
    pub settings: RuleSettings,
}

// RuleSettings ///////////////////////////////////////////////////////////////

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleSettings {
    #[default(true)]
    pub create_missing_directories: bool,

    #[default(vec![])]
    #[serde(with = "serde_regex")]
    pub exclude_pattern: Vec<Regex>,

    #[default(50)]
    pub max_depth: u32,

    #[default(false)]
    pub follow_symlinks: bool,

    #[default(Some(10))]
    pub clean_interval: Option<u32>,
}

// Config - Deserialization ///////////////////////////////////////////////////

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RawConfig {
    #[serde(rename = "default_settings")]
    pub default_rule_settings: RuleSettings,
    pub rules: Vec<Rule>,
}

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RawRuleSettings {
    pub create_missing_directories: Option<bool>,
    #[serde(deserialize_with = "serde_regex::deserialize")]
    pub exclude_pattern: Option<Vec<Regex>>,
    pub max_depth: Option<u32>,
    pub follow_symlinks: Option<bool>,
    pub clean_interval: Option<Option<u32>>,
}

macro_rules! unwrap_raw_setting_or_default {
    ($field:ident, $raw_rule_settings:ident, $raw_config:ident) => {{
        $raw_rule_settings
            .$field
            .unwrap_or($raw_config.default_rule_settings.$field.clone())
    }};
}

macro_rules! new_rule_settings_with_defaults {
    ( $raw_config:ident, $raw_rule_settings:ident, ($($field:ident),+) ) => {{
        RuleSettings {
            $(
                $field: unwrap_raw_setting_or_default!($field, $raw_rule_settings, $raw_config),
            )+
        }
    }};
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw_config = RawConfig::deserialize(deserializer)?;

        let updated_rules = raw_config
            .rules
            .iter()
            .map(|rule| -> Rule {
                let mut rule_new = rule.clone();
                rule_new.settings = {
                    let raw_rule_settings = rule.raw_settings.clone().unwrap_or_default();
                    new_rule_settings_with_defaults!(
                        raw_config,
                        raw_rule_settings,
                        (
                            create_missing_directories,
                            exclude_pattern,
                            max_depth,
                            follow_symlinks,
                            clean_interval
                        )
                    )
                };
                rule_new.raw_settings = None;
                rule_new
            })
            .collect::<Vec<Rule>>();

        Ok(Config {
            rules: updated_rules,
        })
    }
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

// fn custom_serializer_option_vec_regex<S>(
//     value: &Option<Vec<Regex>>,
//     serializer: S,
// ) -> Result<S::Ok, S::Error>
// where
//     S: Serializer,
// {
//     match value {
//         Some(regexes) => {
//             let strings: Vec<String> = regexes.iter().map(|r| r.as_str().to_string()).collect();
//             strings.serialize(serializer)
//         }
//         None => serializer.serialize_none(),
//     }
// }
