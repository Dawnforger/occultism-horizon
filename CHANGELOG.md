# Changelog

All notable changes to the Occultism port for Horizon. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the major version is 0, a minor
version adds features and a patch version fixes them. The version lives in `VERSION`;
`python3 tools/set_version.py X.Y.Z` writes it into every manifest.

## [Unreleased]

### Added
- Versioning: `VERSION`, `tools/set_version.py` and this changelog. The history before it was
  rebuilt as one tagged commit per stage.

## [0.1.3] - 2026-11-02

### Fixed
- A harvested altar drops itself. Its loot table was in 1.21's format (`{"condition": ...}`),
  which 26.3 reads as an unknown condition, so the pool never rolled.

### Changed
- All 15 rows of `TESTING.md` pass in game (agent-driven run on Horizon `0f0fa62`).
- Horizon's `/loot ... mine` bug, found along the way, is reported upstream as
  horizon-rust/horizon#7424 (reproduced with Horizon's template mod only).

## [0.1.2] - 2026-11-02

### Fixed
- The rank label ("Tier: ...") no longer draws over the "Inventory" label.
- Players can no longer put a spirit core in the input slot.
- A core that a hopper pushes into the input binds into an empty core slot, or else passes to
  the output for the hopper below; it no longer jams the altar.
- The core floating over the altar is lit by the air above it instead of being drawn black.

### Changed
- `TESTING.md` rows 8-10 describe what a player can actually do (outputs are filled by
  crushing, cores are refused in the input).

## [0.1.1] - 2026-11-02

### Changed
- `build.sh` uses the Horizon launcher and its portable Rust when they aren't on `PATH`, and
  keeps Cargo's downloads in `.work/`.
- The native glue compiles cleanly against Horizon `0f0fa62` and builds into the game with the
  launcher's `native-build` (launcher 0.2.37 or later: 0.2.26 rejects the `[api]` table).

## [0.1.0] - 2026-11-02

### Added
- First proof of concept, from the cloud session: the Demonic Altar with Occultism's crusher
  spirits bound into four spirit cores (Foliot, Djinni, Afrit, Marid), Occultism's timings,
  tier gates and ingot rule; 15 unit tests of the rules; the content mod generated from an
  Occultism checkout; native glue (not yet compiled); the in-game test plan.

[Unreleased]: v0.1.3...HEAD
[0.1.3]: v0.1.2...v0.1.3
[0.1.2]: v0.1.1...v0.1.2
[0.1.1]: v0.1.0...v0.1.1
[0.1.0]: v0.1.0
