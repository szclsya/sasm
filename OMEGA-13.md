Omega-13
========

*"The truth, however, is that no one has a clue."* [¹](https://web.archive.org/web/20060218125408/https://blogs.msdn.com/michkap/archive/2005/10/16/481625.aspx)

— Michael Kaplan at the Windows Longhorn reset, "A Reset Does Not Mean Everything was Thrown Away."

Omakase needs a clearer vision, and a more realistic look at what could and
should be done. It's meant to be a evolutionary step from APT, not its
undertaker. Let's start small and from concrete issues.

APT
---

### Issues

- Indistinguishable program output, wall of gray text that does not have
  enough contrast or contextual layers.
    - Does not present clearly the risk and potential danger of user actions.
    - Needs a clearer confirmation and presentation on user requested changes.
- Too many fragmented commands, users mostly use `apt`, `apt-get`, `apt-cache`,
  and `apt-file`, leaving all the other commands outside of user awareness.
    - Why are they separate commands anyways?
- `/etc/apt/sources.list{,.d/*.list}` is counter intuitive, difficult for new
  users to know which parameter means which.
- `/etc/apt/apt.conf{,.d/*.conf}` options are obscure and difficult to
  understand, poor documentation.
- Less than ideal performance.
    - Lack of parallelism in resolution, download, and extract (`dpkg`).
    - Download processes require invocation of discrete commands, slowing down
      download (apt update, apt install, apt upgrade, apt full-upgrade,
      apt download, ...).
- Confusing package uninstallation/removal experience.
    - Which do I use, `purge`, `remove`, or `autoremove`?
- Buggy dpkg trigger handling when dependency loops exist.
- Ineffective in preventing dangerous operations, such as removing a large
  amount of packages or `Essential` packages.
    - `"Yes, do as I say!"` does not suffice.
- Too many Debian-specific features and requirements that are unhelpful in
  AOSC OS.
- Some outputs refer to Debian distribution features, which make no sense.
- Insufficient support for local repositories (e.g. `apt` will not run
  properly if local sources are being synchronized).

### Commendables

- Abstracts configuration format (even though flawed), such as sources.list.
- Separates different aspects of configuration (vendor, user, and state files).
- Offers possible solutions when dependencies could not be fulfilled (albeit
  very difficult to understand, what the fuck does "... but will not be
  installed" mean?)

Omakase
-------

### Issues

- TOML configuration is likely too complex and causes potential program
  behavioral issues, could do with abstraction.
- Does not separate between vendor, user, and state files (`user.blueprint`
  should really be a state file).
- Stateless design is not helpful (especially reflected in the blueprint and
  local installation), also that package manager is really a stateful
  application which manages system states.
- Overreaching behavior when handling local packages (local packages will be
  removed without much warning).
- Inconsistent state with dpkg status file.
- Unclear design intentions, should consider distribution-specific features, or
  at least allow them.
- Unreliable migration feature from APT.
- Lack of i18n/l10n support.
- No support for pinning package versions or installing specific versions.

### Commendables

- Defaults to an `apt autoremove`-like uninstallation behaviors.
- Offers confirmation interface before applying changes.
- Makes sure that unsafe operations are beyond reach.

Wishlist
--------

- Parallelism everywhere, from resolution, to download, to installation
  (SquashFS compressed package files in dpkg?).
- Integrate `apt-gen-list` and `atm` functionalities.
- A shim for `apt` commands.
- Compatibility with apt output, when `argv[0]` is `apt`.
- Record desktop environment/suite choices in user configuration, such as
  `Desktop = "gnome"` => `Desktop = "kde"`.
- Extra metadata defining groups of updates, such as "KDE Applications Update
  22.04" and "April 2022 Security Rollup."
- Explain all errors and potential issues, and point users to each
  corresponding documentation page.
- Server-client model, allowing remote administration (like `virt-manager`).
- Undo support based on extra state files.
