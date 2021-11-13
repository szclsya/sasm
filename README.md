# omakase: Experimental Package Manager
`omakase` is an experimental package manager that employs the power of modern Boolean satisfiability problem solvers.

## Build
```bash
cargo build --release
install -Dm755 target/release/oma /usr/local/bin/oma
```

## Try it out
Omakase accepts a config folder containing these files:
+ `config.toml`: main config file folder
+ `blueprint`: a list of desired packages. You should add all packages you intentionally use in this file

Here's a basic example of `config.toml`:
```toml
arch = "amd64"
# Whether to purge package when it's no longer needed
# If set to false, package will only be removed so that configs will remain
purge_on_remove = true

[repo.main]
url = "https://repo.aosc.io/debs"
distribution = "stable"
components = ["main"]
# GPG public key for this repository. Should be filenames relative to $CONFIG_ROOT/keys
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

Put these files at `/etc/omakase/` and run `oma execute`. `Omakase` will download dbs from the specified mirror and find a feasible package installation list, or spill out Unsolvable.

You can also use omakase more like a conventional package manager. Subcommands like `install`, `remove`, `refresh` and `upgrade` mimic behaviors of more conventional package managers, but under the hood, `install` and `remove` just manipulate the blueprint and try to execute the blueprint afterwards and `upgrade` just simply execute the blueprint after refreshing local database (since omakase will automatically pick latest version of packages when executing the blueprint).
