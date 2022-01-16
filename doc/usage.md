# Installing packages
```bash
oma install PKG1 PKG2 ...
```

Possible arguments:
+ `--no-recommends` Do not install recommended packages

Note that in order to make sure the dependency tree is sound and up-to-date, omakase may upgrade existing packages when installing new packages.

# Removing packages
```bash
oma remove PKG1 PKG2 ...
```

Possible arguments:
+ `--remove-recommends` Remove recommended packages introduced by designated packages

This will remove designated packages alongside **all** their dependencies from the system.

Note that just like the previous case, you may see omakase upgrade (or even install) packages when using `remove` subcommand.

# Upgrading packages
```bash
oma upgrade
```

# Searching for packages
```bash
oma search QUERY
```

Query string accepts Regex syntax. Note that only package titles will be searched.

# Search packages that contain certain files
```bash
oma provide FILE
```

Possible arguments:
+ `--bin` Search binary files only. This should be significantly faster.

Search what packages contain a certain file.

# Pick a certain version for a package
```bash
oma pick PKGNAME
```

Tell Omakase to use a certain version of a package.

# Benchmarking mirrors and pick the best one
```bash
oma bench
```

Benchmark mirrors in MirrorLists (see [config documentation](doc/config.md)) and use the best one.

# Download a package from the repository
```bash
oma download PKGNAME
```

Download a package archive file from remote repositories.
