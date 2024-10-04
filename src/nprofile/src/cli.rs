use std::collections::HashMap;

use libnprofile::profile::{Profile, ProfileConfig};

/// Default profile environment.
const DEFAULT_ENVIRONMENT: &str = if cfg!(target_os = "linux") {
    "linux"
} else if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    panic!("Cannot determine default environment for system");
};

/// Supported profile actions.
#[derive(Clone, Debug, Default, clap::ValueEnum)]
pub(crate) enum ProfileAction {
    /// Disable the profile.
    #[value(alias = "d")]
    Disable,
    /// Enable the profile.
    #[default]
    #[value(aliases = &["u", "e"])]
    Enable,
    /// Reset the profile (disable then re-enable).
    #[value(alias = "r")]
    Reset,
}

impl From<&ProfileAction> for String {
    fn from(value: &ProfileAction) -> Self {
        match value {
            ProfileAction::Disable => "disable".to_string(),
            ProfileAction::Enable => "enable".to_string(),
            ProfileAction::Reset => "reset".to_string(),
        }
    }
}

/// Internal profile actions.
///
/// User-facing profile actions are sequences of one or more core actions.
#[derive(Debug)]
enum CoreProfileAction {
    /// Disable the profile.
    Disable,
    /// Enable the profile.
    Enable,
}

impl std::fmt::Display for ProfileAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

fn parse_key_value_pair<K, V>(arg: &str) -> Result<(K, V), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    K: std::str::FromStr,
    K::Err: std::error::Error + Send + Sync + 'static,
    V: std::str::FromStr,
    V::Err: std::error::Error + Send + Sync + 'static,
{
    let delim = arg.find('=').ok_or_else(|| "Invalid format, expected `key=value` but no `\"` found".to_string())?;
    Ok((arg[..delim].parse()?, arg[delim + 1..].parse()?))
}

fn parse_key_value_pairs<K, V>(arg: &str) -> Result<HashMap<K, V>, Box<dyn std::error::Error + Send + Sync + 'static>>
where
    K: std::str::FromStr + Eq + core::hash::Hash,
    K::Err: std::error::Error + Send + Sync + 'static,
    V: std::str::FromStr,
    V::Err: std::error::Error + Send + Sync + 'static,
{
    arg.split(',')
        .map(parse_key_value_pair::<K, V>)
        .collect::<Result<HashMap<K, V>, Box<dyn std::error::Error + Send + Sync + 'static>>>()
}

#[derive(Debug, clap::Parser)]
pub(crate) struct Cli {
    /// Path to the profile config file.
    #[arg(required = true, short, long, env = "CONFIG_PATH", value_hint = clap::ValueHint::FilePath)]
    pub config_path: std::path::PathBuf,
    /// Enable debug logging
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub debug: bool,
    /// Name of the environment to use for managing the profile
    #[arg(short, long, env = "ENVIRONMENT_NAME", default_value = DEFAULT_ENVIRONMENT)]
    pub environment_name: String,
    /// Name of the profile
    pub profile_name: String,
    /// Profile action
    #[arg(default_value_t)]
    pub action: ProfileAction,
    /// Profile-specific args formatted as comma-separated key-value pairs (e.g. ssid=MyWiFi,device=radio1)
    #[arg(value_parser = parse_key_value_pairs::<String, String>)]
    pub profile_args: Option<HashMap<String, String>>,
}

impl Cli {
    fn validate_args(&self) {
        if !self.config_path.is_file() {
            log::error!("Config path is either not a file or does not exist");
            std::process::exit(1);
        }
    }

    fn read_config_from_file(&self) -> ProfileConfig {
        let config = std::fs::read_to_string(&self.config_path).unwrap_or_else(|err| {
            log::error!("Failed to read config file: {}", err.to_string());
            std::process::exit(1);
        });
        let config: ProfileConfig = toml::from_str(config.as_str()).unwrap_or_else(|err| {
            log::error!("Failed to read config file: {}", err.to_string());
            std::process::exit(1);
        });
        if let Err(err) = config.is_valid() {
            log::error!("{}", err.to_string());
            std::process::exit(1);
        }
        log::debug!(
            "Loaded profile config from {:?}, contains {:02} profiles",
            self.config_path,
            config.profiles.len(),
        );
        config
    }

    fn transform_to_profile_map(config: &ProfileConfig) -> HashMap<&str, &Profile> {
        let mut profile_map = HashMap::<&str, &Profile>::with_capacity(config.profiles.len());
        for profile in config.profiles.iter() {
            profile_map.insert(&profile.name, profile);
            if let Some(aliases) = profile.aliases.as_ref() {
                for alias in aliases.iter() {
                    profile_map.insert(alias, profile);
                }
            }
        }
        profile_map
    }

    fn get_profiles_to_action<'a: 'b, 'b>(
        &'a self,
        profile_map: &'b HashMap<&'b str, &'b Profile>,
        config: &'b ProfileConfig,
    ) -> Vec<(&'b Profile, Option<&'b str>)> {
        if let Some(profile) = profile_map.get(self.profile_name.as_str()) {
            let mut profiles = Vec::with_capacity(profile.dependencies.as_ref().map(Vec::len).unwrap_or(0) + 1);
            if let Some(dependencies) = profile.dependencies.as_ref() {
                for dependency in dependencies {
                    if let Some(dependency_profile) = profile_map.get(dependency.name.as_str()) {
                        profiles.push((*dependency_profile, dependency.env_name.as_deref()));
                    } else {
                        log::error!("Invalid dependency profile name {}", dependency.name);
                        std::process::exit(1);
                    }
                }
            }
            if !profile.is_composition_profile() {
                profiles.push((profile, None));
            }
            profiles
        } else {
            log::error!(
                "invalid profile name {}, possible values are: {}",
                self.profile_name,
                config.profiles.iter().map(|profile| profile.name.as_str()).collect::<Vec<_>>().join(", "),
            );
            std::process::exit(1);
        }
    }

    fn run_profile_action(&self, profile: &Profile, environment_name: Option<&str>, action: CoreProfileAction) {
        let environment_name = environment_name.unwrap_or(self.environment_name.as_str());
        match action {
            CoreProfileAction::Enable => {
                log::info!("Enabling profile {} using environment {}", profile.name, self.environment_name);
                if let Err(err) = profile.enable(environment_name, self.profile_args.as_ref()) {
                    log::error!("Failed to enable profile: {}", err);
                    std::process::exit(1);
                }
                log::info!("Enabled profile {}", profile.name);
            },
            CoreProfileAction::Disable => {
                log::info!("Disabling profile {} using environment {}", profile.name, self.environment_name);
                if let Err(err) = profile.disable(environment_name, self.profile_args.as_ref()) {
                    log::error!("Failed to disable profile: {}", err);
                    std::process::exit(1);
                }
                log::info!("Disabled profile {}", profile.name);
            },
        }
    }

    pub fn run(self) {
        crate::logging::configure_logging(self.debug);

        self.validate_args();
        let config = self.read_config_from_file();
        let profile_map = Self::transform_to_profile_map(&config);
        let profiles = self.get_profiles_to_action(&profile_map, &config);

        match self.action {
            ProfileAction::Disable => {
                for (profile, environment_name) in profiles.into_iter().rev() {
                    self.run_profile_action(profile, environment_name, CoreProfileAction::Disable);
                }
            },
            ProfileAction::Enable => {
                for (profile, environment_name) in profiles {
                    self.run_profile_action(profile, environment_name, CoreProfileAction::Enable);
                }
            },
            ProfileAction::Reset => {
                for (profile, environment_name) in profiles.iter().rev() {
                    self.run_profile_action(*profile, *environment_name, CoreProfileAction::Disable);
                }
                for (profile, environment_name) in profiles {
                    self.run_profile_action(profile, environment_name, CoreProfileAction::Enable);
                }
            },
        }
    }
}
