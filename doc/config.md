Omakase requires a config folder of this structure:

```
CONFIG_ROOT (typically /etc/omakase)
|--- config.toml
|--- blueprint
|--- blueprint.d/
|    |--- vendor-1.blueprint
|    `--- vendor-2.blueprint
|--- ignorerules
`--- keys
     |--- repo1-key1.gpg
     |--- repo1-key2.asc
     `--- repo2-key.asc
```

# `config.toml`
This is the main configuration file. It uses `TOML` and have a series of mandatory fields. Here's an example:

```toml
arch = "amd64"
# Whether to purge package when it's no longer needed
# If set to true, config files of packages will also be removed when packages are removed
purge_on_remove = true

[repo.main]
url = "https://repo.aosc.io/debs"
distribution = "stable"
components = ["main"]
# GPG public key for this repository.
# Put the public keys in the `keys/` folder, and provide filenames of the key files here
keys = ["main.asc"]
```

# Blueprints
Blueprint are, as their name suggests, the blueprint for the system. They defines the packages users can use about the system, and omakase will ensure these packages are available. However, this also means that any package that is not included in the system blueprint is not guaranteed to be installed. For example, user might able to use a package installed as dependency, but if this package is no longer depended, it can be removed. Thus, user should always include packages they use in the blueprint files.

Blueprint files have a special syntax. Each line in a blueprint file represents a package request. Such line include two parts: package name and (optional) additional requirements. Here's a few examples:

```
# Example of a simple request
konsole
# Package request with version requirements
linux+kernel (>=3:5.14.0, <3:5.15.0)
mpv (=0.33.1)
```

You can specify additional requirements inside the pair of round brackets after package name. Multiple requirements are separated by `,`. Currently only version requirements (`>`, `>=`, `=`, `<`, `<=`) are supported; but there's plan to add more requirements, including install all recommended packages for a package request.

# Ignore Rules
Ignore rules defines what packages **shouldn't be removed** even if they are neither requested in the blueprints nor required as a dependency. Note that it doesn't prevent packages that match such rules from being installed.

Such rules can be used to prevent manually installed packages from being removed. It can also be used to prevent the currently running kernel from being removed.

Ignore rules files have a very simple syntax. Each line represents a package name. Package names may include variables surrounded by `{}`. These variables are supported for now:
+ `KERNEL_VERSION`: version of the current running kernel
