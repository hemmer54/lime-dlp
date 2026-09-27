use std::path::PathBuf;
use std::{fs, io};

use app::{DownloadType, Tab};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};

mod app;
pub mod command;
pub mod cookies;
mod error;
pub mod i18n;
pub mod media_options;
pub mod progress;
pub mod runtime_update;
mod sponsorblock;
pub mod update;

pub use app::YtGUI;

use sponsorblock::SponsorBlockOption;
use tracing::Level;
use tracing::metadata::LevelFilter;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::media_options::Options;
use crate::media_options::{AudioFormat, AudioQuality, VideoFormat, VideoResolution};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    TogglePlaylist(bool),
    ToggleThumbnail(bool),
    ToggleMetadata(bool),
    ToggleSubtitles(bool),
    ToggleConsole(bool),
    ToggleForceOverwrite(bool),
    SelectedSponsorBlockOption(SponsorBlockOption),
    SelectedVideoFormat(VideoFormat),
    SelectedResolution(VideoResolution),
    SelectedAudioFormat(AudioFormat),
    SelectedAudioQuality(AudioQuality),
    SelectDownloadFolder(DownloadType),
    SelectedDownloadFolder(DownloadType, Option<PathBuf>),
    DownloadFolderTextInput(DownloadType, String),
    SelectDownloadFolderTextInput(DownloadType),
    SelectTab(Tab),
    ProgressEvent(String),
    StartDownload(String),
    StopDownload,

    ToggleSaveWindowPosition(bool),
    SelectYtDlpBinPath,
    SelectedYtDlpBinPath(Option<PathBuf>),
    SelectYtDlpBitPathTextInput(String),
    SelectCookiesFile,
    SelectedCookiesFile(Option<PathBuf>),
    SelectCookiesFileTextInput(String),
    SelectedCookiesBrowser(cookies::Browser),
    SelectedCookiesKeyring(cookies::Keyring),
    InputCookiesProfile(String),
    InputCookiesContainer(String),
    UpdateCheck(Result<Option<update::Version>, update::Error>),
    RuntimeUpdateCheck(Result<Option<runtime_update::RuntimeVersions>, String>),
    CheckRuntimeUpdates,
    InstallRuntimeUpdate,
    RuntimeUpdateFinished(Result<runtime_update::RuntimeVersions, String>),
    OpenLink(String),
    ToggleAdvancedOptions,
    ClearConsole,
    InputCustomArgs(String),
    InputOutputTemplate(String),
    SelectedConcurrentFragments(u8),
    InputSubLangs(String),
    ToggleLiveFromStart(bool),
    InputRateLimit(String),
    OpenDownloadFolder(DownloadType),
    PasteUrl,
    ClearUrl,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct Flags {
    pub url: Option<String>,
    pub config: Config,
}

fn video_download_folder_default() -> PathBuf {
    dirs::video_dir().unwrap_or_else(|| shellexpand::tilde("~/Videos").to_string().into())
}

fn audio_download_folder_default() -> PathBuf {
    dirs::audio_dir().unwrap_or_else(|| shellexpand::tilde("~/Music").to_string().into())
}

fn bin_path_default() -> Option<PathBuf> {
    None
}

fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let opt = Option::<String>::deserialize(de)?;
    let opt = opt.as_deref();
    match opt {
        None | Some("") => Ok(None),
        Some(s) => T::deserialize(s.into_deserializer()).map(Some),
    }
}

fn output_template_default() -> String {
    "%(title)s.%(ext)s".to_string()
}

fn concurrent_fragments_default() -> u8 {
    4
}

fn sub_langs_default() -> String {
    "all".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    #[serde(
        deserialize_with = "empty_string_as_none",
        default = "bin_path_default"
    )]
    bin_path: Option<PathBuf>,
    #[serde(default = "video_download_folder_default")]
    pub video_download_folder: PathBuf,
    #[serde(default = "audio_download_folder_default")]
    pub audio_download_folder: PathBuf,
    #[serde(deserialize_with = "empty_string_as_none")]
    cookies_file: Option<PathBuf>,
    #[serde(deserialize_with = "empty_string_as_none")]
    cookies_browser: Option<String>,
    pub save_window_position: bool,
    pub window_position: Option<WindowPosition>,
    pub window_size: Option<WindowSize>,
    options: Options,
    #[serde(default = "output_template_default")]
    pub output_template: String,
    #[serde(default = "concurrent_fragments_default")]
    pub concurrent_fragments: u8,
    #[serde(default)]
    pub custom_args: String,
    #[serde(default = "sub_langs_default")]
    pub sub_langs: String,
    #[serde(default)]
    pub live_from_start: bool,
    #[serde(default)]
    pub rate_limit: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bin_path: bin_path_default(),
            video_download_folder: video_download_folder_default(),
            audio_download_folder: audio_download_folder_default(),
            cookies_file: Default::default(),
            cookies_browser: Default::default(),
            save_window_position: Default::default(),
            window_position: Default::default(),
            window_size: Default::default(),
            options: Default::default(),
            output_template: output_template_default(),
            concurrent_fragments: concurrent_fragments_default(),
            custom_args: Default::default(),
            sub_langs: sub_langs_default(),
            live_from_start: Default::default(),
            rate_limit: Default::default(),
        }
    }
}

impl Config {
    pub fn migrate_legacy_bin_path(&mut self) {
        let (Some(configured), Some(bundled)) =
            (self.bin_path.as_ref(), command::bundled_bin_path())
        else {
            return;
        };

        let configured = configured
            .canonicalize()
            .unwrap_or_else(|_| configured.clone());
        let bundled = bundled.canonicalize().unwrap_or(bundled);
        if configured == bundled {
            self.bin_path = None;
        }
    }

    pub fn download_folder_for(&self, download_type: DownloadType) -> &PathBuf {
        match download_type {
            DownloadType::Video => &self.video_download_folder,
            DownloadType::Audio => &self.audio_download_folder,
        }
    }

    pub fn download_folder_for_mut(&mut self, download_type: DownloadType) -> &mut PathBuf {
        match download_type {
            DownloadType::Video => &mut self.video_download_folder,
            DownloadType::Audio => &mut self.audio_download_folder,
        }
    }
}

impl Config {
    fn update_config_file(&mut self) -> io::Result<()> {
        let current_config = toml::to_string(self).expect("config to string");
        let config_file = dirs::config_dir()
            .expect("config directory")
            .join("lime-dlp/config.toml");
        fs::write(config_file, &current_config)?;
        tracing::info!("Updated config file to {}", current_config);
        Ok(())
    }
}

pub fn logging() {
    if let Err(_e) = std::env::var("LIME_LOG") {
        tracing::info!("no log level specified, defaulting to debug level for lime_dlp crate only");
        unsafe { std::env::set_var("LIME_LOG", "none,lime_dlp=debug") };
    }

    let logs_dir = dirs::cache_dir()
        .expect("cache dir should exist")
        .join("lime-dlp/logs");

    let _ = std::fs::create_dir_all(&logs_dir);

    let debug_file = rolling::minutely(&logs_dir, "debug");
    let warn_file = rolling::daily(&logs_dir, "warnings");

    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_env_var("LIME_LOG")
                .with_default_directive(LevelFilter::ERROR.into())
                .from_env_lossy(),
        )
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(debug_file.with_max_level(Level::DEBUG))
                .with_ansi(false),
        )
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(warn_file.with_max_level(tracing::Level::WARN))
                .with_ansi(false),
        )
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(std::io::stdout.with_max_level(Level::DEBUG)),
        )
        .init();
}

#[macro_export]
macro_rules! git_hash {
    () => {
        match option_env!("GIT_HASH") {
            Some(hash) => hash.to_string(),
            None => {
                let output = std::process::Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .unwrap();
                String::from_utf8(output.stdout).unwrap()
            }
        }
    };
}
