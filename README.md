# nprofile: Cross-Platform Network Profile Manager

## Overview

`nprofile` is a command line tool for managing network profiles.
If you're like me you have platform-specific shell scripts for managing Wi-Fi, ethernet, VPN, and other connections
on personal and work computers, and have probably copied them across various systems over the years.
With `nprofile` you can consolidate all those shell scripts int one config file, then share that file across all your systems and use one common command line interface.
You could also have separate files per-system, one for personal and work, etc.
When paired with a tools for syncing configs across systems (like Git!), `nprofile` becomes a powerful tool for the terminal-inclined.

## Installation

### Build from Source

Clone the repo from GitHub and use Cargo to build from source:

```bash
git clone git@github.com:conveen/nprofile.git
cd nprofile
cargo build # use the `release` or `release-fat-lto` profiles for optimized builds
cargo run -- --help
```

## Getting Started

`nprofile` reads config files formatted as [toml](https://toml.io/). To get started, create a file named `nprofile.toml`, insert the following profile template for managing Wi-Fi, and fill in the placeholders with commands for your system:

```toml
[[profiles]]
name = "wifi"
aliases = ["w"]
[profiles.envs.<linux|macos|windows>]
can_enable = """
# <commands to determine if profile can be enabled>
# A non-zero exit code means the profile cannot be enabled
# For example, check if some CLI tools are installed/in $PATH:
# which <some_tool> || ( echo "Unable to locate the some_tool executable" && exit 1 )"""
is_enabled = """
# <command to determine if the profile is already enabled>
# A non-zero exit code means the profile is not enabled
# Use this to support idempotency for stateful profiles. Skip if idempotency is not a concern. For example:
# STATUS=$(some_network_tool | grep status | tr -d ' ')
# ! test -z $STATUS"""
enable = """
# <commands to enable the profile>
# A non-zero exit code means the profile was not enabled successfully"""
disable = """
# <commands to enable the profile>
# A non-zero exit code means the profile was not disabled successfully"""
```

Use the following command to enable the profile (assuming `nprofile.toml` is in the cwd):

```bash
/path/to/nprofile --debug -c nprofile.toml -e <environment_name> wifi enable
```

where the `-e` argument matches the environment name (`profiles.envs.<environment_name>`).
To disable the profile, run the same command but with "disable" instead of "enable":

```bash
/path/to/nprofile --debug -c nprofile.toml -e <environment_name> wifi disable
```

For more complex functionality, like dependencies and parameters, see the [Concepts](#concepts) section below.

## Concepts

* **Profile**: Network connection that can be enabled and disabled. This could be Wi-Fi, ethernet (statically or dynamically configured), VPN, SSH tunnel, etc.
Profiles have a unique name, zero or more unique aliases, zero or more dependencies, and zero or more environments.
    * A profile with one or more dependencies and no environments is called a **Composition Profile**. A profile must define at least one environment or dependency.
* **Profile Alias**: Alternative (short) name for a profile.
For example, aliases for the `wifi` profile could be `w` and `wf`.
These are used to reference a profile when enabling or disabling it from the command line.
* **Profile Dependency**: Other profile that must be enabled/disabled for a profile to work.
Dependencies provide a way to compose individual profiles for specific scenarios, like enabling Wi-Fi and a work VPN to access company resources.
* **Profile Environment**: Platform and tool-specific commands for managing a profile.
The standard environments are `linux`, `macos`, and `windows`, but for example an environment that uses NetworkManager CLI on Linux could be called `linux-nmcli`.
Environments have a unique name (for the profile), an optional shell to run the profile commands, zero or more parameters, and the commands to run.
* **Profile Environment Parameters**: Parameters are injected into commands and can be used to modify behavior.
For example, if a command to enable Wi-Fi on Linux requires the device name, then the environment can define a `device` parameter,
and the user-provided argument will be injected into the command before it is run (e.g. for `device = lo` then  `ifconfig | grep -A2 {device}` becomes `ifconfig | grep -A2 lo`).
* **Profile Commands**: See [Getting Started](#getting-started) for the commands and their purpose.
To use parameter arguments in a command, use the syntax `{paramter_name}` and the argument will be injected before running the command.
For example, the command `nmcli device status | grep {device}` when `device` is set to `wifi` will become `nmcli device status | grep wifi`.
To use literal brackets in a command use double brackets (e.g. `awk {{ print $2 }}`).


## Config File Specification

```toml
[[profiles]]
name = "<profile_a>"
# Optional
aliases = ["<alias1>", "<alias2>"]
# Optional
# dependencies = ["<profile1>"]
# Optional
[profiles.env.<env_name>.parameters]
param1 = "<default_value>"
paramN = "<default_value>"
[profiles.env.<env_name>]
# Optional - defaults to platform-specific default shell
shell = "<shell_path>"
can_enable = """
<command>"""
# Optional, if not provided will always be false
is_enabled = """
<command>"""
enable = """
<command>"""
disable = """
<command>"""

# This is a composition profile
[[profiles]]
name = "<profile_name>"
dependencies = ["<profile_a>"]
```

## Contributing

Contribution are welcome! Before submitting a PR, please ensure the code compiles, that you've written some description of what the PR is meant to accomplish,
and that you've updated the in-code documentation appropriately. PR reviews may be infrequent, but I'll do my best.
