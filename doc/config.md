Omakase requires a config folder of this structure:

```
CONFIG_ROOT (typically /etc/omakase)
|--- config.toml
|--- user.blueprint
|--- blueprint.d/
|    |--- vendor-1.blueprint
|    `--- vendor-2.blueprint
`--- keys
     |--- repo1-key1.gpg
     |--- repo1-key2.asc
     `--- repo2-key.asc
```

# `config.toml`
This is the main configuration file. It uses `TOML` and have a series of mandatory fields. Here's an example:

```toml
arch = "amd64"

# Repository configuration sections are denoted by `[repo.REPO_NAME]`. REPO_NAME can be arbitary.
[repo.main]
# Omakase support loading mirrors from a mirrorlist
# The mirrorlist path must be an absolute path
source = { mirrorlist = "/usr/share/distro-repository-data/mirrors.toml", preferred = "origin" }
# Or, use a simple URL
#source = "https://repo.aosc.io"
distribution = "stable"
components = ["main"]
# GPG public key for this repository.
# Put the public keys in the `keys/` folder, and provide filenames of the key files here
keys = ["main.asc"]
# Tags are used by external programs to identify repositories. Omakase doesn't use them.
tags = ["topic-template"]
```

## The MirrorList file format
A MirrorList file defines a series of possible mirrors. Such file should use `TOML` file format. Here's an example:

```toml
# This field specifies which field should be used by default
default = "origin"

# And a list of mirrors
[origin]
description = "AOSC main repository"
url = "https://repo.aosc.io/"

[magicmirror]
description = "Mirror built by magic"
url = "https://magicmirror.bruh/"
```

## The Omanomicon: `unsafe` section
Some dangerous flags of Omakase can be enabled in the `unsafe` section. This section is optional and the default config will not contain this section, but if you are sure you want to enable these features, you can manually add this section and enable the flags you want.

```toml
[unsafe]
# When Omakase thinks it should remove a package, purge the package's config files too
purge_on_remove = true
# Allow dpkg to skip fsync on files. Only use on systems with battery backup.
unsafe_io = true
# Allow remove essential packages.
# If not implicitly set to true, Omakase will refuse any action that involves removing essential packages
allow_remove_essential = true
```

# Blueprints
Blueprint are, as their name suggests, the blueprint for the system. They defines the packages users can use about the system, and omakase will ensure these packages are available. However, this also means that any package that is not included in the system blueprint is not guaranteed to be installed. For example, user might able to use a package installed as dependency, but if this package is no longer depended, it can be removed. Thus, user should always include packages they use in the blueprint files.

There are two types of blueprint: _user blueprint_ and _vendor blueprint_. There is only one user blueprint at `CONFIG_ROOT/user.blueprint`, but there may be many vendor blueprints at `CONFIG_ROOT/blueprint.d/*.blueprint`. When using the CLI, Omakase will only modify user blueprint. So, if you wants to remove a package inside vendor blueprint, Omakase will not allow so. You will have to manually remove the line in the corresponding vendor blueprint.

Blueprint files have a special syntax. Each line in a blueprint file represents a package request. Such line include two parts: package name and (optional) additional requirements. Here's a few examples:

```
# Example of a simple request
konsole
# Package name that includes variables
linux-kernel-{KERNEL_VERSION}
# Package request with version requirements
linux+kernel (>=3:5.14.0, <<3:5.15.0)
mpv (=0.33.1)
# Package that are installed from local debs
some_pkg (local)
# Package that are installed because they are recommended by other packages
fcitx5-base
fcitx5 (added_by = fcitx5-base)
fcitx5-qt (added_by = fcitx5-base)
```

There may be variables in package names. These can be used to dynamically request packages based on system state. Currently, these variables are supported:
+ `KERNEL_VERSION`: version of the current running kernel, can be used to prevent current kernel from being removed.

You can specify additional attributes inside the pair of round brackets after package name. Multiple arguments are separated by `,`. Currently these attributes are supported:
+ Version requirements (`>>`, `>=`, `=`, `<<`, `<=`): Indicate what range of version should be installed. Multiple requirements are allowed as far as they are not contradictory (for example, `>=2, <=1` will not be accepted).
  - `>>` means strictly larger and `<<` means strictly smaller
  - Note that this only accepts full deb version, which includes epoch, upstream version and package revision.
+ `local`: Install this package from local package repository. This will be added automatically if you use `install --local` to install a local deb.
+ `added_by = PKGNAME`: This package is introduced by another package rather than direct user request. Recommended packages will contain this attribute to show which package recommends them. When removing packages with `--remove-recommends` argument, all packages that have this attribute and is pointing to the package to remove will also be removed.
