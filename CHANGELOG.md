# Changelog

All notable changes to this project will be documented in this file.

## [pwnhub-bot-v0.2.0] - 2025-03-23

### Bug Fixes

- Clippy lints
- Remove -o option from git-cliff

### Features

- Add hackban command

### Miscellaneous Tasks

- Update non breaking dependencies
- Update sqlx
- Update other non breaking dependencies

### Refactor

- Run fmt

## [pwnhub-bot-v0.1.0] - 2022-09-06

### Bug Fixes

- Remove unused import

### Documentation

- Penalty string generation

### Features

- Prevent sending of default stickers
- Filter invites based on partial input
- Timeout on sending of default stickers
- Notify user about penalty
- Graceful shutdown
- Allow unused penalty variants
- Ci caching

### Miscellaneous Tasks

- Bump iana-time-zone from 0.1.44 to 0.1.47

### Refactor

- Fmt
- Run nightly formatter

## [pwnhub-bot-v0.0.4] - 2022-08-16

### Bug Fixes

- Run cargo sqlx prepare

### CI/CD

- Enable verify option in cargo-release

## [pwnhub-bot-v0.0.3] - 2022-08-16

### Bug Fixes

- Correctly filter member parameter and invite duplicates

## [pwnhub-bot-v0.0.2] - 2022-08-16

### Bug Fixes

- Run cargo sqlx prepare

### Features

- Combine invite hints from local cahe and database and base hints on the member parameter

### Miscellaneous Tasks

- Update a code commit

## [pwnhub-bot-v0.0.1] - 2022-08-14

### Bug Fixes

- Enable sqlx offline feature
- Set `SQLX_OFFLINE` env
- Allow RUSTSEC-2020-0071
- Name option without underscore
- Cargo-fmt
- Run cargo sqlx prepare
- Use placeholder correctly

### CI/CD

- Add deny.toml
- Remove cargo-audit job since it is already covered by cargo-deny
- First draft for gh release action
- Add input field for major version number
- Add git-cliff config file
- Add release.toml
- Dont publish to crates.io
- Add config files for cliff
- Configure cargo-release with git-cliff
- Add action that runs cargo-release
- Fix typo in package name
- Correctly set --verbose flag
- Fetch all tags and git history in checkout and pass --no-confirm flag to cargo-release
- Set git user and email
- Sign commits
- Specify environment
- Disable signed push
- Use baptiste0928/cargo-install@v1 instead of actions-rs/install@v0.1
- Add --sign option to cargo-release
- Add docker workflow
- Add docker workflow
- Correctly set image name
- Setup automatic docker push action

### Features

- Add table for invited members
- Add rustfmt config
- Implement internal invite store
- Implement invite tracking
- Implement invite list command
- Allow listing invites from other members
- Implement invite revoke command

### Miscellaneous Tasks

- Update chrono to 0.4.20
- Add metadata for bot

### Refactor

- Use single slash command to list invites of self or other members
- Allow members to list the invites of another member if this other member is themselve and add some docs
- Use symlink to link to migrations in parent folder, so cargo publish doesnt complain

