# jade-saves

Read-only native Rust Palworld save discovery and terminal reader. No Go, .NET, browser, JavaScript, C++ codec, subprocess, or game-save writing is used at runtime.

```rust
let scan = jade_saves::scan_game_saves(None)?;
// A native folder picker can supply Some(absolute_path) instead.
let result = jade_saves::read_terminal(&scan.saves[0])?;
```

Both operations must run on a worker thread. `GameWorld` is Clone + Serialize + Deserialize and can be persisted as the local world's game link; a later read revalidates its identity. `TerminalResult` contains all owners, so the UI must filter by its remembered owner, otherwise the unique `isLocal` owner, otherwise ask once. Records preserve original game IDs and names; translate species/trait keys only at the presentation boundary.

`read_terminal_files(&[PathBuf], &[PlayerFile])` and `scan_from_locations(selected, local_app_data, steam_libraries)` are deterministic test interfaces. A successful read verifies sizes/timestamps twice, including a newly enumerated complete file list. Missing containers, missing wanted individuals, malformed player records, ambiguous WGS blobs, wrong owner IDs, changing snapshots, and bad compression return an error; an error must leave the current inventory intact.

Public corpus tests (no user saves):

```powershell
cargo test -p jade-saves --tests -- --ignored --test-threads=1
```

The ignored integration tests require the repository's existing public research corpus (`work/terminal-import/parser/research`, `work/game-saves/discovery-ref`, `work/qa-v113/palhelm-modern.sav`, and independent `work/qa-v114/expected-players.json`). They deliberately fail if invoked without the fixtures. Set `JADE_SAVE_FIXTURES` and `JADE_EXPECTED_PLAYERS` to supply alternate paths for the terminal corpus.

`cargo run --release -p jade-saves --example read_fixture -- explicit-manifest.json` exposes the same reader for performance comparison. Manifest shape: `{ "level": ["absolute world .sav path"], "players": [{ "id": "32 hex owner id", "path": "absolute player .sav path" }] }`. The example never scans default directories.

Licenses and the vendored parser's compatibility patch are documented in `NOTICE.md`.
