# Lime DLP

Lime DLP is a lightweight desktop GUI for downloading video and audio with [yt-dlp](https://github.com/yt-dlp/yt-dlp).

## Features

- Video and audio downloads
- Playlist support
- Configurable format, resolution, output folder, cookies, subtitles, and rate limits
- Embedded console output and download progress
- Automatic detection of yt-dlp when no custom binary is configured
- Windows runtime updates for yt-dlp, Deno, FFmpeg, and the yt-dlp EJS solver
- Updates are checked in the background and installed only after clicking the update button

## Download

Windows x64 release packages are published from the repository's GitHub Releases page.

The application can use the runtime bundle shipped with a release, a managed per-user runtime bundle, or a user-selected yt-dlp executable. Leave the yt-dlp path empty to use automatic detection.

## Building

Requirements:

- Rust stable toolchain
- Platform dependencies required by [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)

Build and check the application:

```text
cargo check -p lime-dlp
cargo build --release
```

For a Windows package with bundled runtime tools:

```text
cargo run -p xtask -- package-windows
```

The package is written to `packages/lime-dlp-windows-64.zip` during the build process. Release artifacts should be placed in the root `release/` directory locally and uploaded to GitHub Releases rather than committed to the repository.

## Runtime updates

On 64-bit Windows, Lime DLP checks the upstream releases for:

- yt-dlp
- Deno
- FFmpeg
- yt-dlp EJS

The check does not download files. The console provides a context-sensitive button: first **CHECK UPDATES**, then **UPDATE TOOLS** when a verified update is available. Downloads are staged, SHA-256 verified, and activated as a complete bundle. Existing downloads are not interrupted.

## Configuration

Configuration is stored at:

- Windows: `%APPDATA%\\lime-dlp\\config.toml`
- Linux/macOS: the platform-specific directory returned by the operating system configuration directory

A custom yt-dlp path is an explicit override. Clear it to return to automatic detection.

## License

Lime DLP is released into the public domain under the [Unlicense](LICENSE).
