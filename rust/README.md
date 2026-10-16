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
├─ Cargo.toml                 # workspace (members: crates/ytdlp-core, apps/desktop/src-tauri)
├─ crates/
│  └─ ytdlp-core/             # Rust port of YtDlpDownloader.Core (net10.0 lib)
│     ├─ src/
│     │  ├─ lib.rs
│     │  ├─ cancellation.rs   # cloneable async CancellationToken
│     │  ├─ models/           # AppSettings, MediaInfo, ProxyRules, YtDlpFormat, ...
│     │  └─ services/         # YtDlpCli, ProcessRunner, MediaInfoParser, ...
│     └─ tests/               # integration tests mirroring the C# test project
├─ apps/
│  └─ desktop/                # Tauri v2 shell + Svelte 5 frontend
│     ├─ index.html, vite.config.ts, svelte.config.js, package.json
│     ├─ make-icons.mjs       # regenerates src-tauri/icons (no dependencies)
│     ├─ src/                 # Svelte app (App.svelte, views/, lib/)
│     └─ src-tauri/           # Rust command layer (Cargo crate `ytdlp-downloader-desktop`)
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

## Phase 2 — Tauri app (`apps/desktop`)

The desktop shell is **Tauri v2** with a **Svelte 5 + TypeScript + Vite**
frontend. The Rust side is a thin command layer over `ytdlp-core`; the frontend
owns all UI state (the role the WPF view-models played).

### Division of responsibility

- **Frontend (Svelte)** — URL/parse-result state, the in-page download queue,
  manual source selection, template chips, cookie/proxy pickers, theming, logs,
  progress, and the download orchestration loops (parse-each / download-each).
- **`src-tauri` (Rust)** — settings persistence, environment detection, cookie
  folder scanning, proxy status text, `--dump-single-json` parsing, building the
  yt-dlp `-f` expression, running downloads (streaming progress as events),
  cancellation, native file/folder pickers, notifications, taskbar progress and
  the native window theme.

### Command surface

| Command | Purpose |
| --- | --- |
| `get_settings` / `update_settings` | Load settings; shallow-merge a patch and persist |
| `ytdlp_status` / `detect_ytdlp` | Resolve yt-dlp (download page) / resolve + `--version` (settings page) |
| `ffmpeg_status` | ffmpeg on `PATH` + configured path state and status text |
| `default_download_dir` | `%USERPROFILE%\Downloads` fallback |
| `list_cookie_files` / `cookie_folder_status` | Cookie folder scan + hint/status text |
| `proxy_indicator` / `proxy_summary` | Proxy status strings for the download / settings page |
| `parse_media` / `get_media_info` | Parse a URL (playlist-aware) / a single video |
| `start_download` | Run one download, emitting `download:update` events |
| `cancel_job` | Cancel the parse-or-download currently in flight |
| `pick_file` / `pick_folder` | Native dialogs (driven from Rust, so no plugin permissions) |
| `open_url` / `reveal_path` | Open links / reveal a file in Explorer |
| `notify` | Windows toast on download completion (truncated to 250 chars) |
| `set_taskbar_progress` / `set_window_theme` | Taskbar button and native title-bar theme |

`DownloadRequestDto.kind`/`resolution` use the same PascalCase string enums as
`settings.json`; `manual` selection is forwarded verbatim so the format
expression is built in Rust with the ported `YtDlpFormat` / C# manual rules.

### Divergences from the WPF app (app layer)

- **Settings location.** The C# app passed `AppContext.BaseDirectory` (the exe
  folder); the Rust app uses the core service default,
  `%APPDATA%\YtDlpDownloader\settings.json`. The JSON shape is unchanged.
- **Dialogs, notifications and links are driven from Rust** (`tauri-plugin-*`
  Rust APIs), so the frontend needs no plugin capabilities — only `core:default`.
- **Thumbnails** are rendered straight from their URL in `<img>` tags (CSP is
  disabled in `tauri.conf.json`) instead of being downloaded and cached.
- **The native title bar replaces the WPF window chrome.** Light/dark is applied
  with `set_window_theme` (`System` follows the OS); the palette and the
  proportional down-scaling of the old `ScaleTransform` are reproduced in CSS.
- **The queue lives in the frontend**, and yt-dlp progress is streamed as
  `download:update` events (`line` / `percent` / `speed` / `eta` / `outputPath`).
- **Detecting yt-dlp runs on commit, not per keystroke.** The C# `YtDlpPath`
  setter re-ran `RefreshDetectionAsync()` on every change (spawning `yt-dlp
  --version` per keystroke); the Rust app persists the trimmed value as you type
  and re-detects when the field loses focus.

## Building & testing

```powershell
cd rust

# Core only (no frontend build needed):
cargo test -p ytdlp-downloader-core
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# Whole workspace (the Tauri crate embeds `apps/desktop/dist`, so build it first):
cd apps\desktop
npm install
npm run check          # svelte-check (types + a11y)
npm run build          # produces dist/
cd ..\..
cargo build            # builds core + the desktop crate
```

Run the app in development:

```powershell
cd rust\apps\desktop
npm run tauri dev
```

> `tauri-build` embeds `apps/desktop/dist` at compile time, so a bare
> `cargo build` at the workspace root fails until `npm run build` has produced
> it. `cargo test -p ytdlp-downloader-core` is unaffected.

> **crates.io mirror.** `rust/.cargo/config.toml` redirects crates.io to the USTC
> mirror because a direct connection to crates.io is roughly 20× slower on this
> machine (tens of seconds per index file). `Cargo.lock` still records the
> official source, so the file changes nothing about what is built — delete it if
> you can reach crates.io directly.

## Roadmap

- [x] **Phase 1 — Core**: models, services, tests.
- [x] **Phase 2 — Tauri app**: `apps/desktop` (`src-tauri` crate + Svelte
      frontend) wired to the core.
- [x] **Phase 3 — Feature parity**: queue, live mode, proxy UI, theming,
      thumbnails, taskbar progress, notifications.
- [ ] **Phase 4 — Packaging**: single-file Windows release; retire the .NET
      sources once the Rust app is verified.

## License

Same as the repository: YT-DLP Downloader Non-Commercial Share-Alike License 1.0
(see `../LICENSE`). New dependencies must be license-compatible.