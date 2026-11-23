# Code-Maps

Developer plugins for reading codebases. Currently one plugin:

## ua-viewer

A native Rust (egui) viewer for `knowledge-graph.json` produced by
[Understand-Anything](https://github.com/Egonex-AI/Understand-Anything).

- Layer overview first, then drill into a layer's nodes (large to small).
- Layout ported from the original dashboard's force simulation.
- Search, filters, tour, portals between layers, path finder, minimap, export (SVG, Markdown, JSON), code viewer.
- Drag cards with the left mouse button; positions are kept when the layout rebuilds.

### Install

**Claude Code**

```text
/plugin marketplace add theblacknull/Code-Maps
/plugin install ua-viewer@code-maps
```

Then, in a project that has `.ua/knowledge-graph.json`:

```text
/ua-viewer:view
```

**Codex**

The repository includes a Codex plugin manifest at `.codex-plugin/plugin.json` and the same skill under `skills/`. Add this repository as a plugin source in Codex.

### Which binary runs

`scripts/launch.sh` chooses the binary for your system:

| System | Release asset |
| --- | --- |
| Windows x64 | `ua-viewer-windows-x64.exe` |
| macOS Apple silicon | `ua-viewer-macos-arm64` |
| macOS Intel | `ua-viewer-macos-x64` |
| Linux x64 | `ua-viewer-linux-x64` |

It uses a local copy if present, otherwise downloads the asset from the matching GitHub Release, and as a last resort builds from source with `cargo build --release` (requires [Rust](https://rustup.rs)).

### Build from source

```bash
cargo build --release
./target/release/ua-viewer path/to/knowledge-graph.json
```

Default path: `.ua/knowledge-graph.json`.

### Release

Push a tag `v<version>` (matching `.claude-plugin/plugin.json`). The `release` workflow builds all four binaries and attaches them to the GitHub Release.

```bash
git tag v0.1.0
git push origin v0.1.0
```

### Licence

MIT
