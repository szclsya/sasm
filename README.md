# apm: Experimental Package Manager
`apm` is an experimental package manager that employs the power of modern Boolean satisfiability problem solvers.

## Build
Install dependencies:
+ `openssl`: for HTTPS connections
+ `nettle`: for OpenPGP support

After that, just do:
```bash
cargo build --release
install -Dm755 target/release/apm /usr/local/bin/apm
```

## Try it out
apm accepts a config folder containing these files:
+ `apm.toml`: main config file folder
+ `wishlist`: a list of desired packages

Here's a basic example of `apm.toml`:
```toml
arch = "amd64"
# Whether to purge package when it's no longer needed
# If set to false, package will only be removed so that configs will remain
purge_on_remove = true

[repo.main]
url = "https://repo.aosc.io/debs"
distribution = "stable"
components = ["main"]
# GPG public key for this repository
certs = ["/path/to/publickey.asc"]
```

And here's an example of `wishlist`:
```
kernel-base
util-base
shadow
dpkg
vim
sudo
# You can specify the range of version you wish
alacritty (>0.7, <=1.0)
```

Put these files at `/etc/apm/` and run. apm will download dbs from the specified mirror and find a feasible package installation list, or spill out Unsolvable.

## Solver
apm utilizes [varisat](https://github.com/jix/varisat), a very fast, CDCL based SAT solver. Currently, solver is able to enroll all dependency rules (like package dependencies and breaks) in the db into the solver and try to find a feasible solution, and then try to optimize the result.

### Optimality
Although varisat can find a feasible solution, there's no guarantee that this is the best solution. For example, the solution may:

+ have redundant packages, and
+ have non-latest packages, although feasible solutions with latest packages exists.

One way to get around it (without re-implementing an efficient SAT solver, which is not easy), we can try to improve the result by providing some restrictions to the solver. We can force the solver to pick the latest package and find out if the result is better (that is, the new result won't downgrade other packages, or introduce new packages). We can also try to assume all versions of a particular package cannot be used, and if the problem is still solvable and no packages are downgraded, it means that this package is not mandatory.

### TODO: Error reporting
Although solver can tell us the requirements are infeasible, it cannot tell us what went wrong in a idiomatic way. It can only generate a proof, and that's not particularly human readable.
