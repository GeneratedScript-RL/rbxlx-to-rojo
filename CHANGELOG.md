# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0] - 2026-06-14
### Added
- Added `workspace.json` and `startergui.json` hierarchy/property exports.
- Added tests for binary place conversion and typed hierarchy properties.

### Changed
- Updated XML and binary parsing to the vendored Roblox release-700 reflection
  database with support for 2024-2026 property formats.
- Made Roblox file extension matching case-insensitive.
- Sanitized Roblox instance names that are invalid as Windows paths.
- Removed object referents and internal Studio bookkeeping properties from
  hierarchy JSON exports.
- Replaced instance reference IDs with Roblox-style object paths.
- Culled default-valued properties, null references, UniqueIds, empty property
  maps, and empty child lists from hierarchy JSON.
- Removed properties from Workspace and Camera hierarchy nodes.

## [1.0.1] - 2021-04-11
### Fixed
- Fixed newer builds not being usable.

## [1.0.0] - 2021-01-06
### Added
- Added support for .rbxl and .rbxm, and not just .rbxlx.

### Changed
- Changed file reading mechanism to be one that should be more optimized, increasing read times. You can further increase read times by switching to binary (.rbxl, .rbxm) files instead of using .rbxlx.
