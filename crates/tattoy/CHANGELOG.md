# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/tattoy-org/tattoy/compare/tattoy-v0.1.6...tattoy-v0.1.7) - 2025-07-21

### Added

- support cursor colour for animated cursor
- preserve text attrs like underline, reverse, etc
- also get fg and bg colours with OSC parser
- config for adjusting the cursor scale
- animated cursors

### Fixed

- ensure shader assets exist
- *(animated_cursor)* various cosmetic improvements

### Other

- refactor e2e test and add shader tests
- optionally hash GPU render
- move glsl files to own directory
- seperate shader tattoy from GPU code

## [0.1.6](https://github.com/tattoy-org/tattoy/compare/tattoy-v0.1.5...tattoy-v0.1.6) - 2025-07-06

### Other

- update Cargo.lock dependencies

## [0.1.4](https://github.com/tattoy-org/tattoy/compare/tattoy-v0.1.3...tattoy-v0.1.4) - 2025-07-02

### Other

- update to shadow-terminal v0.2.0
- migrated Shadow Terminal into its own repo
- *(deps)* update Wezterm dependencies
