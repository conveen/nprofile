use std::{borrow::Cow, ops::Deref};

/// Default shell for running commands.
///
/// Only Unix and Widnows systems are supported. Uses:
///
/// * `/bin/bash` on Unix; and
/// * `C:\Windows\System32\WindowsPowershell\v1.0\powershell.exe` on Windows.
pub const DEFAULT_SHELL: &str = if cfg!(unix) {
    "/bin/bash"
} else if cfg!(windows) {
    r#"C:\Windows\System32\WindowsPowershell\v1.0\powershell.exe"#
} else {
    panic!("Cannot determine default shell for unsupported system");
};

/// Wrapper over [`std::process::Output`] with convenience methods for using output streams.
pub struct CommandResult(std::process::Output);

impl Deref for CommandResult {
    type Target = std::process::ExitStatus;

    fn deref(&self) -> &Self::Target {
        &self.0.status
    }
}

impl CommandResult {
    /// Decode stdout to [`String`].
    pub fn stderr(&self) -> crate::error::Result<Cow<'_, str>> {
        Ok(Cow::Borrowed(std::str::from_utf8(&self.0.stderr).map_err(crate::error::Error::from)?.trim()))
    }

    /// Decode stderr to [`String`].
    pub fn stdout(&self) -> crate::error::Result<Cow<'_, str>> {
        Ok(Cow::Borrowed(std::str::from_utf8(&self.0.stdout).map_err(crate::error::Error::from)?.trim()))
    }
}

/// Run a system command using the specified shell and capture stdin/stdout.
///
/// # Parameters
///
/// * `command`: The command to run.
/// * `shell`: The shell to run the command with (see: [`DEFAULT_SHELL`]).
///
/// # Errors
///
/// [`crate::error::Error`]: when the shell command cannot be executed.
pub fn run_command<S, I>(command: I, shell: Option<&str>) -> crate::error::Result<CommandResult>
where
    S: AsRef<std::ffi::OsStr>,
    I: IntoIterator<Item = S>,
{
    let shell = shell.unwrap_or(DEFAULT_SHELL);
    #[cfg(target_family = "unix")]
    let command_arg = "-c";
    #[cfg(target_family = "windows")]
    let command_arg = "-Command";

    Ok(CommandResult(
        std::process::Command::new(shell).arg(command_arg).args(command).output().map_err(crate::error::Error::from)?,
    ))
}

/// Run a function repeatedly until it returns `true`.
///
/// Can be used to e.g. wait for a NIC to be enabled before running another command.
///
/// # Parameters
///
/// * `predicate`: The function to run.
/// * `sleep_for`: The number of seconds to sleep in between invocations (defaults to 1).
///
/// # Errors
///
/// [`crate::error::Error`]: when the predicate function returns an error.
pub fn wait_for<F>(predicate: F, sleep_for: Option<u64>) -> crate::error::Result<()>
where
    F: Fn() -> crate::error::Result<bool>,
{
    let sleep_for = sleep_for.unwrap_or(1);
    while !predicate()? {
        std::thread::sleep(std::time::Duration::from_secs(sleep_for));
    }
    Ok(())
}
