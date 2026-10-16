# Rust port (`rust/`)

This directory is the in-progress **Rust rewrite** of YT-DLP Downloader. The
original C#/.NET 10 + WPF implementation still lives in `../src` and `../tests`
and stays in place (on the `refactor/rust-port` branch) until the port is
complete and verified.

> Target stack (decided): **Tauri v2 + web frontend**, porting **core first,
> then GUI**. See *Roadmap* below.

## Layout

```text
rust/
├─ Cargo.toml                 # workspace (members: crates/ytdlp-core)
├─ crates/
│  └─ ytdlp-core/             # Rust port of YtDlpDownloader.Core (net10.0 lib)
│     ├─ src/
│     │  ├─ lib.rs
│     │  ├─ cancellation.rs   # cloneable async CancellationToken
│     │  ├─ models/           # AppSettings, MediaInfo, ProxyRules, YtDlpFormat, ...
│     │  └─ services/         # YtDlpCli, ProcessRunner, MediaInfoParser, ...
│     └─ tests/               # integration tests mirroring the C# test project
└─ README.md
```

## Phase 1 — Core (this crate, done)

`crates/ytdlp-core` is a 1:1 port of `YtDlpDownloader.Core`:

| C# | Rust |
| --- | --- |
| `Models/AppSettings.cs` | `models/app_settings.rs` |
| `Models/MediaInfo.cs` | `models/media_info.rs` |
| `Models/PlaylistInfo.cs` | `models/playlist_info.rs` |
| `Models/ProxyRules.cs` | `models/proxy_rules.rs` |
| `Models/YtDlpFormat.cs` | `models/ytdlp_format.rs` |
| `Services/MediaInfoParser.cs` | `services/media_info_parser.rs` |
| `Services/ProcessRunner.cs` | `services/process_runner.rs` |
| `Services/SettingsService.cs` | `services/settings_service.rs` |
| `Services/YtDlpPathService.cs` | `services/ytdlp_path_service.rs` |
| `Services/FfmpegPathService.cs` | `services/ffmpeg_path_service.rs` |
| `Services/YtDlpCli.cs` | `services/ytdlp_cli.rs` |
| App `DownloadViewModel` naming logic | `models/output_template.rs` |

### Intentional differences from the C# code

- **No MVVM base classes.** `Core/Mvvm/{ObservableObject,RelayCommand,
  AsyncRelayCommand}` are dropped. The Tauri frontend owns UI state, so those
  WPF-era types have no meaningful Rust equivalent. The tests that covered them
  are correspondingly not ported.
- **Settings JSON is wire-compatible.** `AppSettings` uses PascalCase field
  names and string-valued enums, matching `JsonStringEnumConverter`, so a
  `settings.json` written by either implementation can be read by the other.
  Missing fields fall back to the same defaults; unknown fields are ignored.
  (Enum parsing is exact-case here, whereas `System.Text.Json` is
  case-insensitive by default.)
- **Errors are values, not exceptions.** `ParseError` keeps the
  `InvalidDataException` vs `JsonException` split the fallback logic relies on;
  `YtDlpError` and `ProcessError` replace thrown exceptions.
- **Cancellation is a `CancellationToken`** built on `tokio::sync::watch`
  (cloneable, thread-safe), replacing `System.Threading.CancellationToken`.
- **`PATH` lookup is injectable.** `PathEnvProvider` lets the path-service tests
  supply a `PATH` value instead of mutating process-wide environment state
  (which is not thread-safe and is `unsafe` in Rust edition 2024).
- **Process tree kill** uses `taskkill /T /F` on Windows to match
  `Process.Kill(entireProcessTree: true)` so a cancelled yt-dlp does not orphan
  ffmpeg.

### Test parity

The C# Core test project has 125 tests. The Rust integration suite mirrors them
by area: models, `YtDlpFormat`, `ProxyRules`, output-template naming,
`MediaInfoParser`, `SettingsService`, path services, `ProcessRunner`, and
`YtDlpCli`. The dropped MVVM tests (`ObservableObject`/`RelayCommand`/
`AsyncRelayCommand`) account for the difference.

`ProcessRunner` tests spawn real `cmd.exe`/`ping` processes and are therefore
Windows-only (`#![cfg(windows)]`).

## Building & testing

```powershell
cd rust
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Roadmap

- [x] **Phase 1 — Core**: models, services, tests.
- [ ] **Phase 2 — Tauri app**: `apps/desktop` (`src-tauri` crate + web frontend)
      wiring the core to a UI, plus commands for parse/download/queue/settings.
- [ ] **Phase 3 — Feature parity**: queue, live mode, proxy UI, theming,
      thumbnails, taskbar progress, notifications.
- [ ] **Phase 4 — Packaging**: single-file Windows release; retire the .NET
      sources once the Rust app is verified.

## License

Same as the repository: YT-DLP Downloader Non-Commercial Share-Alike License 1.0
(see `../LICENSE`). New dependencies must be license-compatible.