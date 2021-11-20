# omakase: Declarative Package Manager
`omakase` is an declarative package manager that employs the power of modern Boolean satisfiability problem solvers.

## Build
```bash
cargo build --release
install -Dm755 target/release/oma /usr/local/bin/oma
```

## Concepts and configurations
Omakase accepts a config folder containing a series of files:
+ `config.toml`: main config file folder
+ `blueprint`: a list of desired packages. You should add all packages you intentionally use in this file
  - `blueprint.d/`: vendored blueprints
+ `ignorerules`: a list of package naming rules that omakase should not remove
+ `keys/`: stores PGP public keys for repositories

Here's a basic example of `config.toml`:
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

And here's an example of `blueprint`:
```
kernel-base
util-base
shadow
dpkg
vim
sudo
# Comment lines are allowed
# You can also specify the range of version you want
alacritty (>0.7, <=1.0)
```

Omakase is a declarative package manager: given a list of package requests, it will attempt to find the optimal dependency tree, and adjust the system according to this result. This includes installing missing packages, upgrading packages, **downgrade** packages if needed, and even **remove packages that are not needed**. This means that some package that is installed by omakase may be uninstalled later if it is not a dependency anymore. So, always add packages you use to the blueprint to ensure they are guaranteed to be installed.

If you install a package manually, you may want to add it to `ignorerules` so that omakase will not attempt to remove it because it is seemly not required by the blueprint. You can also write package names with variables. For example, you can add `linux-kernel-{KERNEL_VERSION}` to `ignorerules` so that the current running kernel won't be removed until you reboot to another kernel.

Currently supported variables:
+ `KERNEL_VERSION`: version of the current running kernel

## Operating omakase
Put these files at `/etc/omakase/` and run `oma execute`. It will download the latest package databases, read your blueprint, and try to find an optimal solution. If omakase can't fulfill the blueprint, it will try to tell you which of the packages are causing problems.

You can also use omakase more like a conventional package manager. Subcommands like `install`, `remove`, `refresh` and `upgrade` mimic behaviors of more conventional package managers, but under the hood, `install` and `remove` just manipulate the blueprint and try to execute the blueprint afterwards and `upgrade` just simply execute the blueprint after refreshing local database (since omakase will automatically pick latest version of packages when executing the blueprint).
