use std::{env::VarError, fs, path::PathBuf};

use anyhow::Context;
use regex::Regex;
use serde::{de::Error, Deserialize, Deserializer};
use shellexpand::LookupError;
use smart_default::SmartDefault;

use crate::args::Args;

#[derive(SmartDefault, Debug, Clone)]
pub struct Config {
    pub rules: Vec<Rule>,
}

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigRaw {
    #[serde(rename = "default_settings")]
    pub default_rule_settings: RuleSettings,
    pub rules: Vec<Rule>,
}

macro_rules! unwrap_raw_setting_or_default {
    ($field:ident, $settings_raw:ident, $config_raw:ident) => {{
        $settings_raw
            .$field
            .clone()
            .unwrap_or($config_raw.default_rule_settings.$field.clone())
    }};
}

macro_rules! new_rule_settings_with_defaults {
    ( $config_raw:ident, $settings_raw:ident, ($($field:ident),+) ) => {{
        RuleSettings {
            $(
                $field: unwrap_raw_setting_or_default!($field, $settings_raw, $config_raw),
            )+
        }
    }};
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut config_raw = ConfigRaw::deserialize(deserializer)?;

        let updated_rules = config_raw
            .rules
            .iter_mut()
            .map(|rule| -> Rule {
                let settings_raw = &rule.raw_settings;
                rule.settings = new_rule_settings_with_defaults!(
                    config_raw,
                    settings_raw,
                    (
                        create_missing_directories,
                        exclude_pattern,
                        max_depth,
                        follow_symlinks,
                        clean_interval
                    )
                );
                rule.clone()
            })
            .collect::<Vec<Rule>>();

        let updated_config = Config {
            rules: updated_rules,
        };

        Ok(updated_config)
    }
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
    pub raw_settings: RuleSettingsRaw,

    #[serde(skip)]
    pub settings: RuleSettings,
}

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

#[derive(SmartDefault, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleSettingsRaw {
    pub create_missing_directories: Option<bool>,
    #[serde(deserialize_with = "serde_regex::deserialize")]
    pub exclude_pattern: Option<Vec<Regex>>,
    pub max_depth: Option<u32>,
    pub follow_symlinks: Option<bool>,
    pub clean_interval: Option<Option<u32>>,
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

/// Given a `Rule` and a setting name, try to get the setting value. If `None`, get the default
/// from Config.
// #[macro_export]
// macro_rules! get_setting {
//     ($config:tt, $rule:tt, $name:tt) => {{
//         $rule.settings.$name.unwrap_or($config.settings.$name)
//     }};
// }

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
