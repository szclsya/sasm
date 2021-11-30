# Installing packages
```bash
oma install PKG1 PKG2 ...
```

Note that in order to make sure the dependency tree is sound and up-to-date, omakase may upgrade existing packages when installing new packages.

# Removing packages
```bash
oma remove PKG1 PKG2 ...
```

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

Search what packages contain a certain file.
