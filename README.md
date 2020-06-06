# vgmms

`vgtk`-based SMS+MMS client

![vgmms screenshot](https://user-images.githubusercontent.com/65555601/83953195-c4fb0000-a82d-11ea-845b-0fba2ded883a.png)

## motivation

The other messaging stacks for linux either have no plans to support MMS or do not handle media attachments correctly, and rely on outdated and unmaintained infrastructure (namely the telepathy framework). `vgmms` exists to do only SMS+MMS and intends to have feature parity with messaging clients for Android and iOS.

## status

- sending/receiving MMS and SMS works
	- both group chats and media attachments work
- logs are persisted to disk (in `$XDG_DATA_HOME/vgmms/vgmms.db`)
- lots of work to do still (see below)
- contributions welcome!

## installation

1. install ofono and MMSd--you may need to use patched versions of these!
	- [patched ofono](https://git.sr.ht/~anteater/ofono) fixes dual-stack IPv6 connectivity (needed for MMS at least with T-Mobile)
	- [patched MMSd](https://git.sr.ht/~anteater/mmsd) fixes MMS parsing. whether you need this depends on your network's MMS implementation (again, at least T-Mobile seems to need this)
	- if you're feeling brave, or too lazy to, install them from upstream ([ofono](https://git.kernel.org/pub/scm/network/ofono/ofono.git), [MMSd](https://git.kernel.org/pub/scm/network/ofono/mmsd.git/)) or your package manager and please [report if SMS and MMS work](https://todo.sr.ht/~anteater/mms-stack-bugs)!
2. make sure you have a Rust compilation toolchain, e.g. `pacman -S rust` or `curl https://sh.rustup.rs -sSf | sh`
3. download the source: `git clone https://git.sr.ht/~anteater/vgmms`
4. `cd vgmms`
4. `cargo build --release`

## running

1. make sure `ofonod` and `mmsd` are both running. be able to watch their logs for error messages.
2. run `vgmms`
3. if you have trouble (or don't), [please submit a bug (or success) report](https://todo.sr.ht/~anteater/mms-stack-bugs)!

## known bugs

- lots, since things are still in-development
- see the [bug tracker](https://todo.sr.ht/~anteater/mms-stack-bugs)
