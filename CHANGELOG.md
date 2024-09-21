# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Removed

## [0.2.0] - 2024-09-21

### Feature

- Added support for listening for TLS connections and connecting to TLS upstreams.
  See the output of -h for new flags.

## [0.1.3] - 2024-02-08

### Fixed

- Avoid crash when using docker container due to missing CA certificates

## [0.1.2] - 2023-10-17

### Added

- Added `--replay-only` flag to ensure only replays and no recordings are made. Useful in CI.

## [0.1.1] - 2023-10-16

### Added

- Initial release
