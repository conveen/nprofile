use std::{borrow::Cow, collections::HashMap, ops::Deref};

use serde::Deserialize;

/// String containing a shell command.
///
/// Has convenience methods for sanitizing and injecting args into commands.
/// Should not be used outside of [`ProfileEnvironment`].
#[derive(Debug, Deserialize)]
pub struct CommandString(String);

impl Deref for CommandString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CommandString {
    /// Sanitize and inject args into the command string.
    pub fn prepare_with_args(
        &self,
        args: Option<&HashMap<&str, interpolator::Formattable<'_>>>,
    ) -> crate::error::Result<String> {
        let command = if let Some(args) = args {
            interpolator::format(self.0.as_str(), args).map_err(crate::error::Error::from)?
        } else {
            self.0.clone()
        };
        Ok(command)
    }
}

/// Environment-specific details to enable and disable a profile.
///
/// Profiles, like Wi-Fi or LAN networks, may need to be activated
/// using different commands in different environments, like Windows
/// vs. Linux vs. macOS. Environments (envs) provide the commands
/// necessary to enable and disable a profile.
#[derive(Debug, serde::Deserialize)]
pub struct ProfileEnvironment {
    /// Shell to run commands with.
    pub shell: Option<String>,
    /// Command arguments.
    /// Parameters injected into commands before they're run.
    pub parameters: Option<HashMap<String, String>>,
    /// Command to determine whether profile can be enabled.
    pub can_enable: CommandString,
    /// Command to determine whether profile is already enabled.
    pub is_enabled: Option<CommandString>,
    /// Command to enable profile.
    pub enable: CommandString,
    /// Command to disable profile.
    pub disable: CommandString,
}

/// Profile.
///
/// Profiles are metadata and instructions for configuring networking on a host.
/// Profiles can (and likely will always) have platform-specific instructions,
/// catpured in `envs`.
///
/// # Examples
///
/// Most laptops users connect to Wi-Fi at least once per day, likely more.
/// Connecting to Wi-Fi on the command line is very platform-specific.
/// A configuration for Linux (using nmcli) could look like the following:
///
/// ```toml
/// [[profiles]]
/// name = "wifi"
/// aliases = ["w"]
/// [profiles.envs.linux-nmcli.parameters]
/// device = "wifi"
/// ssid = "SomeSSID"
/// [profiles.envs.linux-nmcli]
/// can_enable = """
/// STATUS=$(nmcli device status | grep {device} | head -n 1 | awk '{{ print $3 }}')
/// ! test -z $STATUS || >&2 echo "Unable to locate device {device}""""
/// is_enabled = """
/// STATUS=$(nmcli device status | grep {device} | head -n 1 | awk '{{ print $3 }}')
/// test "connected" = $STATUS"""
/// enable = """
/// nmcli radio {device} on
/// ! test -z "{ssid}" && nmcli dev {device} connect {ssid}"""
/// disable = """
/// nmcli radio {device} off"""
/// ```
#[derive(Debug, serde::Deserialize)]
pub struct Profile {
    /// Name of the profile.
    pub name: String,
    /// Profile name aliases.
    pub aliases: Option<Vec<String>>,
    /// Profile dependencies.
    /// Refers to one or more profile names or aliases.
    pub dependencies: Option<Vec<String>>,
    /// Profile environments.
    pub envs: HashMap<String, ProfileEnvironment>,
}

impl Profile {
    /// Transform arguments for use in the profile commands.
    ///
    /// [`ProfileEnvironment::parameters`] defines the valid parameters for a profile environment.
    /// Only arguments for defined parameters are included, others are filtered out.
    /// Default parameter values are used if no arguments are provided.
    ///
    /// For example, given a profile with the following parameters:
    ///
    /// ```toml
    /// [profiles.envs.some-env.parameters]
    /// device = "wifi"
    /// ssid = "SomeSSID"
    /// ```
    ///
    /// * if no arguments are provided, then `device = "wifi"` and `ssid = "SomeSSID"`.
    /// * if `device` is set to `"radio2"`, then `device = "radio2"` and `ssid = "SomeSSID"`.
    ///
    /// Default parameter values can also be empty `""`.
    fn transform_args<'a: 'b, 'b>(
        &'a self,
        environment: &'a ProfileEnvironment,
        args: Option<&'a HashMap<String, String>>,
    ) -> Option<HashMap<&'b str, interpolator::Formattable<'b>>> {
        environment.parameters.as_ref().map(move |parameters| {
            parameters
                .iter()
                .map(|(k, v)| {
                    args.as_ref()
                        .map(|args| (k.as_str(), interpolator::Formattable::display(args.get(k.as_str()).unwrap_or(v))))
                        .unwrap_or_else(|| (k.as_str(), interpolator::Formattable::display(v)))
                })
                .collect()
        })
    }

    /// Get [`ProfileEnvironment`] by name.
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::InvalidEnvironment`]: If the environment is not defined for the profile.
    fn get_environment<S: AsRef<str>>(&self, environment_name: S) -> crate::error::Result<&ProfileEnvironment> {
        self.envs.get(environment_name.as_ref()).ok_or_else(|| crate::error::Error::InvalidEnvironment {
            environment: environment_name.as_ref().to_owned(),
            profile: self.name.to_owned(),
        })
    }

    /// Run the `can_enable` command using the given [`ProfileEnvironment`].
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::ProfileRequirementsNotMet`]: If the profile cannot be enabled for the environment.
    fn _can_enable(
        &self,
        environment: &ProfileEnvironment,
        args: Option<&HashMap<&str, interpolator::Formattable<'_>>>,
    ) -> crate::error::Result<()> {
        let command = environment.can_enable.prepare_with_args(args)?;
        log::debug!("Running command can_enable: {}", &command);
        let result = crate::process::run_command([command], environment.shell.as_deref())
            .map_err(|err| crate::error::Error::ProfileRequirementsNotMet { message: err.to_string() })?;
        if log::log_enabled!(log::Level::Debug) {
            if let Some(code) = result.code().as_ref() {
                log::debug!("Command exited with code {}", code);
            }
        }
        if !result.success() {
            Err(crate::error::Error::ProfileRequirementsNotMet {
                message: result
                    .stderr()
                    .unwrap_or_else(|err| Cow::Owned(format!("Failed to read command error output: {}", err)))
                    .into_owned(),
            })
        } else {
            Ok(())
        }
    }

    /// Run the `is_enabled` command using the given [`ProfileEnvironment`].
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::Io`]: If any IO errors occur when attempting to running the command.
    fn _is_enabled(
        &self,
        environment: &ProfileEnvironment,
        args: Option<&HashMap<&str, interpolator::Formattable<'_>>>,
    ) -> crate::error::Result<bool> {
        if let Some(is_enabled) = environment.is_enabled.as_ref() {
            let command = is_enabled.prepare_with_args(args)?;
            log::debug!("Running command is_enabled: {}", &command);
            let result = crate::process::run_command([command], environment.shell.as_deref())?;
            if log::log_enabled!(log::Level::Debug) {
                if let Some(code) = result.code().as_ref() {
                    log::debug!("Command exited with code {}", code);
                }
            }
            Ok(result.success())
        } else {
            Ok(false)
        }
    }

    /// Run the `enable` command using the given [`ProfileEnvironment`].
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::CommandFailure`]: If the command has a non-zero exit code.
    /// [`crate::error::Error::Io`]: If any IO errors occur when attempting to running the command.
    fn _enable(
        &self,
        environment: &ProfileEnvironment,
        args: Option<&HashMap<&str, interpolator::Formattable<'_>>>,
    ) -> crate::error::Result<()> {
        let command = environment.enable.prepare_with_args(args)?;
        log::debug!("Running command enable: {}", &command);
        let result = crate::process::run_command([command], environment.shell.as_deref())?;
        if log::log_enabled!(log::Level::Debug) {
            if let Some(code) = result.code().as_ref() {
                log::debug!("Command exited with code {}", code);
            }
        }
        if !result.success() {
            Err(crate::error::Error::CommandFailure {
                code: result.code().unwrap_or(-1),
                message: result
                    .stderr()
                    .unwrap_or_else(|err| Cow::Owned(format!("Failed to read command error output: {}", err)))
                    .into_owned(),
            })
        } else {
            Ok(())
        }
    }

    /// Run the `disable` command using the given [`ProfileEnvironment`].
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::CommandFailure`]: If the command has a non-zero exit code.
    /// [`crate::error::Error::Io`]: If any IO errors occur when attempting to running the command.
    fn _disable(
        &self,
        environment: &ProfileEnvironment,
        args: Option<&HashMap<&str, interpolator::Formattable<'_>>>,
    ) -> crate::error::Result<()> {
        let command = environment.disable.prepare_with_args(args)?;
        log::debug!("Running command disable: {}", &command);
        let result = crate::process::run_command([command], environment.shell.as_deref())?;
        if log::log_enabled!(log::Level::Debug) {
            if let Some(code) = result.code().as_ref() {
                log::debug!("Command exited with code {}", code);
            }
        }
        if !result.success() {
            Err(crate::error::Error::CommandFailure {
                code: result.code().unwrap_or(-1),
                message: result
                    .stderr()
                    .unwrap_or_else(|err| Cow::Owned(format!("Failed to read command error output: {}", err)))
                    .into_owned(),
            })
        } else {
            Ok(())
        }
    }

    /// Enable the profile using the given environment.
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::ProfileRequirementsNotMet`]: If the profile requirements are not met.
    /// [`crate::error::Error::CommandFailure`]: If any commands exit with a non-zero code.
    /// [`crate::error::Error::Io`]: If any IO errors occur when attempting to running the command.
    pub fn enable<S>(&self, environment_name: S, args: Option<&HashMap<String, String>>) -> crate::error::Result<()>
    where
        S: AsRef<str>,
    {
        let environment = self.get_environment(environment_name)?;
        let formattable_args = self.transform_args(environment, args);
        self._can_enable(environment, formattable_args.as_ref())?;
        if !self._is_enabled(environment, formattable_args.as_ref())? {
            self._enable(environment, formattable_args.as_ref())?;
        }

        Ok(())
    }

    /// Disable the profile using the given environment.
    ///
    /// # Errors
    ///
    /// [`crate::error::Error::CommandFailure`]: If any commands exit with a non-zero code.
    /// [`crate::error::Error::Io`]: If any IO errors occur when attempting to running the command.
    pub fn disable<S>(&self, environment_name: S, args: Option<&HashMap<String, String>>) -> crate::error::Result<()>
    where
        S: AsRef<str>,
    {
        let environment = self.get_environment(environment_name)?;
        let formattable_args = self.transform_args(environment, args);
        if self._is_enabled(environment, formattable_args.as_ref())? {
            self._disable(environment, formattable_args.as_ref())?;
        }

        Ok(())
    }
}

/// Collection of profiles.
#[derive(Debug, serde::Deserialize)]
pub struct ProfileConfig {
    /// The list of profiles defined in the config.
    pub profiles: Vec<Profile>,
}
