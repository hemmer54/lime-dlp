use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;

use chrono::Local;
use gpui_kit::component::{
    self, InteractiveElementExt,
    button::ButtonVariants,
    input::{Input, InputState},
    menu::PopupMenuItem,
    scroll::ScrollableElement,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use notify_rust::Notification;
use url::Url;

use crate::cookies::{Browser, CookiesFromBrowser, Keyring};
use crate::error::DownloadError;
use crate::media_options::{
    AudioFormat, AudioQuality, VideoFormat, VideoResolution, playlist_options,
};
use crate::runtime_update;
use crate::sponsorblock::SponsorBlockOption;
use crate::update;
use crate::{Config, Flags, Message, WindowPosition, WindowSize, fl};

const LIME: u32 = 0xb8ff3c;
const PANEL: u32 = 0x101a12b8;
const SURFACE: u32 = 0x17251bb8;
const TEXT: u32 = 0xecffe1;
const MUTED: u32 = 0xa6bd9c;
const FALLOUT: &str = "JH_Fallout";

type InputEntity = Entity<InputState>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DownloadType {
    Video,
    Audio,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Tab {
    Video,
    Audio,
    Settings,
}

#[derive(Clone)]
struct FormInputs {
    download_url: InputEntity,
    video_folder: InputEntity,
    audio_folder: InputEntity,
    bin_path: InputEntity,
    cookies_file: InputEntity,
    cookies_profile: InputEntity,
    cookies_container: InputEntity,
    output_template: InputEntity,
    custom_args: InputEntity,
    sub_langs: InputEntity,
    rate_limit: InputEntity,
}

#[derive(Clone, Copy)]
enum ButtonKind {
    Primary,
    Secondary,
    Danger,
    Info,
}

#[derive(Clone, Copy)]
enum FilePickerKind {
    YtDlp,
    Cookies,
}

pub struct YtGUI {
    pub(crate) download_link: String,
    pub(crate) is_playlist: bool,
    pub(crate) get_thumbnail: bool,
    pub(crate) embed_metadata: bool,
    pub(crate) embed_subtitles: bool,
    pub(crate) show_console: bool,
    pub(crate) force_overwrite: bool,
    pub(crate) sponsorblock: SponsorBlockOption,
    pub(crate) config: Config,
    pub(crate) active_tab: Tab,
    pub(crate) download_type: DownloadType,
    pub(crate) playlist_progress: Option<String>,
    pub(crate) download_message: Option<Result<String, DownloadError>>,
    is_file_dialog_open: bool,
    form: Option<FormInputs>,
    pub(crate) sender: tokio::sync::mpsc::UnboundedSender<Message>,
    pub(crate) command: crate::command::Command,
    pub(crate) progress: Option<f32>,
    window_height: f32,
    window_width: f32,
    pub(crate) new_version: Option<update::Version>,
    pub(crate) runtime_update: Option<runtime_update::RuntimeVersions>,
    pub(crate) runtime_update_in_progress: bool,
    pub(crate) show_advanced_options: bool,
    pub(crate) log_output: String,
}

impl YtGUI {
    pub fn new(
        flags: Flags,
        sender: tokio::sync::mpsc::UnboundedSender<Message>,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<Message>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        tracing::info!("config loaded: {flags:#?}");

        cx.on_app_quit(|app, _cx| {
            app.command.kill();
            if let Err(error) = app.config.update_config_file() {
                tracing::error!("Failed to update config file on exit: {error}");
            }
            async {}
        })
        .detach();

        cx.observe_window_bounds(window, |app, window, _cx| {
            let bounds = window.bounds();
            app.window_width = f32::from(bounds.size.width);
            app.window_height = f32::from(bounds.size.height);
            app.config.window_size = Some(WindowSize {
                width: app.window_width,
                height: app.window_height,
            });
            if app.config.save_window_position {
                app.config.window_position = Some(WindowPosition {
                    x: f32::from(bounds.origin.x),
                    y: f32::from(bounds.origin.y),
                });
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            while let Some(message) = receiver.recv().await {
                let updated = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.update_state(message);
                        cx.notify();
                    })
                    .is_ok()
                });
                if !updated {
                    break;
                }
            }
        })
        .detach();

        let initial_link = flags.url.clone().unwrap_or_default();
        Self {
            download_link: initial_link,
            is_playlist: false,
            get_thumbnail: false,
            embed_metadata: false,
            embed_subtitles: false,
            show_console: true,
            force_overwrite: false,
            sponsorblock: SponsorBlockOption::default(),
            config: flags.config,
            active_tab: Tab::Video,
            download_type: DownloadType::Video,
            playlist_progress: None,
            download_message: None,
            is_file_dialog_open: false,
            form: None,
            sender,
            command: crate::command::Command::default(),
            progress: None,
            window_height: 0.0,
            window_width: 0.0,
            new_version: None,
            runtime_update: None,
            runtime_update_in_progress: false,
            show_advanced_options: false,
            log_output: String::new(),
        }
    }

    fn ensure_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() {
            return;
        }
        let cookies = self.cookies_from_browser();
        self.form = Some(FormInputs {
            download_url: input(
                window,
                cx,
                "Download URL(s), separated by spaces or commas",
                &self.download_link,
            ),
            video_folder: input(
                window,
                cx,
                "Video download folder",
                &self.config.video_download_folder.display().to_string(),
            ),
            audio_folder: input(
                window,
                cx,
                "Audio download folder",
                &self.config.audio_download_folder.display().to_string(),
            ),
            bin_path: input(
                window,
                cx,
                "yt-dlp binary path (blank uses automatic detection)",
                &self
                    .config
                    .bin_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            cookies_file: input(
                window,
                cx,
                "Cookies file (optional)",
                &self
                    .config
                    .cookies_file
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            cookies_profile: input(window, cx, "Browser profile", &cookies.profile),
            cookies_container: input(window, cx, "Firefox container", &cookies.container),
            output_template: input(
                window,
                cx,
                "%(title)s.%(ext)s",
                &self.config.output_template,
            ),
            custom_args: input(
                window,
                cx,
                "e.g. --geo-bypass --no-mtime",
                &self.config.custom_args,
            ),
            sub_langs: input(window, cx, "all, en.*, ja", &self.config.sub_langs),
            rate_limit: input(
                window,
                cx,
                "e.g. 5M, 500K (unlimited if empty)",
                &self.config.rate_limit,
            ),
        });
    }

    fn sync_form(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.form.as_ref() else {
            return;
        };
        let download_url = form.download_url.read(cx).value().to_string();
        let video_folder = form.video_folder.read(cx).value().to_string();
        let audio_folder = form.audio_folder.read(cx).value().to_string();
        let bin_path = form.bin_path.read(cx).value().to_string();
        let cookies_file = form.cookies_file.read(cx).value().to_string();
        let cookies_profile = form.cookies_profile.read(cx).value().to_string();
        let cookies_container = form.cookies_container.read(cx).value().to_string();
        let output_template = form.output_template.read(cx).value().to_string();
        let custom_args = form.custom_args.read(cx).value().to_string();
        let sub_langs = form.sub_langs.read(cx).value().to_string();
        let rate_limit = form.rate_limit.read(cx).value().to_string();

        if download_url != self.download_link {
            self.update_state(Message::InputChanged(download_url));
        }
        self.config.video_download_folder = PathBuf::from(video_folder);
        self.config.audio_download_folder = PathBuf::from(audio_folder);
        self.config.bin_path = (!bin_path.trim().is_empty()).then(|| PathBuf::from(bin_path));
        self.config.cookies_file =
            (!cookies_file.trim().is_empty()).then(|| PathBuf::from(cookies_file));
        self.config.output_template = output_template;
        self.config.custom_args = custom_args;
        self.config.sub_langs = sub_langs;
        self.config.rate_limit = rate_limit;

        let mut cookies = self.cookies_from_browser();
        cookies.profile = cookies_profile;
        cookies.container = cookies_container;
        self.config.cookies_browser = cookies.to_arg();
    }

    fn handle_message(&mut self, message: Message, window: &mut Window, cx: &mut Context<Self>) {
        match message {
            Message::SelectDownloadFolder(download_type) => {
                self.pick_download_folder(download_type, window, cx);
            }
            Message::SelectYtDlpBinPath => self.pick_file(FilePickerKind::YtDlp, window, cx),
            Message::SelectCookiesFile => self.pick_file(FilePickerKind::Cookies, window, cx),
            Message::PasteUrl => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    if let Some(form) = &self.form {
                        form.download_url.update(cx, |state, cx| {
                            state.set_value(text.clone(), window, cx);
                        });
                    }
                    self.update_state(Message::InputChanged(text));
                }
            }
            Message::ClearUrl => {
                if let Some(form) = &self.form {
                    form.download_url.update(cx, |state, cx| {
                        state.set_value(String::new(), window, cx);
                    });
                }
                self.update_state(Message::ClearUrl);
            }
            Message::StartDownload(_) => {
                self.sync_form(cx);
                self.update_state(Message::StartDownload(self.download_link.clone()));
            }
            message => self.update_state(message),
        }
        cx.notify();
    }

    fn pick_download_folder(
        &mut self,
        download_type: DownloadType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_file_dialog_open {
            return;
        }
        self.sync_form(cx);
        self.is_file_dialog_open = true;
        let starting_dir = self.config.download_folder_for(download_type).clone();
        let entity = cx.entity();
        window
            .spawn(cx, async move |cx| {
                let folder = choose_folder(starting_dir).await;
                let _ = cx.update_window_entity(&entity, |app, window, cx| {
                    app.update_state(Message::SelectedDownloadFolder(
                        download_type,
                        folder.clone(),
                    ));
                    if let (Some(path), Some(form)) = (folder, app.form.as_ref()) {
                        let input = match download_type {
                            DownloadType::Video => form.video_folder.clone(),
                            DownloadType::Audio => form.audio_folder.clone(),
                        };
                        input.update(cx, |state, cx| {
                            state.set_value(path.to_string_lossy().into_owned(), window, cx);
                        });
                    }
                    cx.notify();
                });
            })
            .detach();
    }

    fn pick_file(&mut self, kind: FilePickerKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_file_dialog_open {
            return;
        }
        self.sync_form(cx);
        self.is_file_dialog_open = true;
        let starting_dir = match kind {
            FilePickerKind::YtDlp => self.config.bin_path.clone().unwrap_or_else(|| "~".into()),
            FilePickerKind::Cookies => self
                .config
                .cookies_file
                .clone()
                .unwrap_or_else(|| "~".into()),
        };
        let entity = cx.entity();
        window
            .spawn(cx, async move |cx| {
                let file = choose_file(starting_dir).await;
                let _ = cx.update_window_entity(&entity, |app, window, cx| {
                    match kind {
                        FilePickerKind::YtDlp => {
                            app.update_state(Message::SelectedYtDlpBinPath(file.clone()));
                            if let (Some(path), Some(form)) = (file.as_ref(), app.form.as_ref()) {
                                form.bin_path.update(cx, |state, cx| {
                                    state.set_value(
                                        path.to_string_lossy().into_owned(),
                                        window,
                                        cx,
                                    );
                                });
                            }
                        }
                        FilePickerKind::Cookies => {
                            app.update_state(Message::SelectedCookiesFile(file.clone()));
                            if let (Some(path), Some(form)) = (file.as_ref(), app.form.as_ref()) {
                                form.cookies_file.update(cx, |state, cx| {
                                    state.set_value(
                                        path.to_string_lossy().into_owned(),
                                        window,
                                        cx,
                                    );
                                });
                            }
                        }
                    }
                    if file.is_none() {
                        app.is_file_dialog_open = false;
                    }
                    cx.notify();
                });
            })
            .detach();
    }

    pub fn update_state(&mut self, event: Message) {
        match event {
            Message::InputChanged(input) => {
                self.download_link = input;
                if !self.command.is_running() && self.download_message.is_some() {
                    self.download_message = None;
                    self.progress = None;
                }
            }
            Message::SelectedResolution(resolution) => {
                self.config.options.video_resolution = resolution;
            }
            Message::TogglePlaylist(is_playlist) => {
                self.is_playlist = is_playlist;
            }
            Message::ToggleThumbnail(get_thumbnail) => {
                self.get_thumbnail = get_thumbnail;
            }
            Message::ToggleMetadata(embed_metadata) => {
                self.embed_metadata = embed_metadata;
            }
            Message::ToggleSubtitles(embed_subtitles) => {
                self.embed_subtitles = embed_subtitles;
            }
            Message::ToggleConsole(show_console) => {
                self.show_console = show_console;
            }
            Message::ToggleForceOverwrite(force_overwrite) => {
                self.force_overwrite = force_overwrite;
            }
            Message::SelectedSponsorBlockOption(sponsorblock) => {
                self.sponsorblock = sponsorblock;
            }
            Message::SelectedVideoFormat(format) => {
                self.config.options.video_format = format;
            }
            Message::SelectDownloadFolder(_)
            | Message::SelectYtDlpBinPath
            | Message::SelectCookiesFile => {}
            Message::SelectedDownloadFolder(download_type, folder) => {
                if let Some(path) = folder {
                    *self.config.download_folder_for_mut(download_type) = path;
                }
                self.is_file_dialog_open = false;
            }
            Message::DownloadFolderTextInput(download_type, folder_string) => {
                *self.config.download_folder_for_mut(download_type) = PathBuf::from(folder_string);
            }
            Message::SelectDownloadFolderTextInput(download_type) => {
                let current_folder = self
                    .config
                    .download_folder_for(download_type)
                    .display()
                    .to_string();
                let path = PathBuf::from(shellexpand::tilde(&current_folder).to_string());
                *self.config.download_folder_for_mut(download_type) = path;
            }
            Message::SelectTab(selected_tab) => {
                self.active_tab = selected_tab;
                match self.active_tab {
                    Tab::Video => self.download_type = DownloadType::Video,
                    Tab::Audio => self.download_type = DownloadType::Audio,
                    Tab::Settings => {}
                }
                if !self.command.is_running() {
                    self.download_message = None;
                    self.progress = None;
                }
            }
            Message::SelectedAudioFormat(format) => {
                self.config.options.audio_format = format;
            }
            Message::SelectedAudioQuality(quality) => {
                self.config.options.audio_quality = quality;
            }
            Message::InputCustomArgs(custom_args) => {
                self.config.custom_args = custom_args;
            }
            Message::InputOutputTemplate(output_template) => {
                self.config.output_template = output_template;
            }
            Message::SelectedConcurrentFragments(concurrent_fragments) => {
                self.config.concurrent_fragments = concurrent_fragments;
            }
            Message::InputSubLangs(sub_langs) => {
                self.config.sub_langs = sub_langs;
            }
            Message::ToggleLiveFromStart(live_from_start) => {
                self.config.live_from_start = live_from_start;
            }
            Message::InputRateLimit(rate_limit) => {
                self.config.rate_limit = rate_limit;
            }
            Message::ProgressEvent(progress) => {
                let trimmed = progress.trim();
                if !trimmed.starts_with("__") && !trimmed.is_empty() {
                    if let Some(err) = trimmed.strip_prefix("stderr:") {
                        let err_trimmed = err.trim();
                        if !err_trimmed.is_empty() {
                            log_error_to_file(&format!("[yt-dlp stderr] {err_trimmed}"));
                        }
                        if err_trimmed.starts_with("ERROR:") {
                            self.log_message(&format!(
                                "[ERROR] {}",
                                err_trimmed
                                    .strip_prefix("ERROR:")
                                    .unwrap_or(err_trimmed)
                                    .trim()
                            ));
                        } else if err_trimmed.starts_with("WARNING:") {
                            self.log_message(&format!(
                                "[WARN] {}",
                                err_trimmed
                                    .strip_prefix("WARNING:")
                                    .unwrap_or(err_trimmed)
                                    .trim()
                            ));
                        } else if !err_trimmed.is_empty() {
                            self.log_message(err_trimmed);
                        }
                    } else {
                        let lower_trimmed = trimmed.to_lowercase();
                        if lower_trimmed.contains("error")
                            || lower_trimmed.contains("fail")
                            || lower_trimmed.contains("warning")
                        {
                            log_error_to_file(&format!("[yt-dlp stdout] {trimmed}"));
                        }
                        self.log_message(trimmed);
                    }
                }
                self.handle_progress_event(&progress);
            }
            Message::ClearConsole => self.log_output.clear(),
            Message::StartDownload(link) => self.start_download(link),
            Message::StopDownload => {
                self.command.kill();
                self.log_message("[WARN] Download stopped by user.");
                self.progress = None;
                self.download_message = None;
            }
            Message::ToggleSaveWindowPosition(save_window_position) => {
                self.config.save_window_position = save_window_position;
            }
            Message::SelectedYtDlpBinPath(file) => {
                if let Some(path) = file {
                    self.config.bin_path = Some(path);
                }
                self.is_file_dialog_open = false;
            }
            Message::SelectYtDlpBitPathTextInput(file_string) => {
                let path = PathBuf::from(file_string);
                self.config.bin_path = Some(path);
            }
            Message::SelectedCookiesFile(file) => {
                if let Some(path) = file {
                    self.config.cookies_file = Some(path);
                }
                self.is_file_dialog_open = false;
            }
            Message::SelectCookiesFileTextInput(cookies_string) => {
                self.config.cookies_file = if cookies_string.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(cookies_string))
                };
            }
            Message::UpdateCheck(result) => match result {
                Ok(maybe_version) => self.new_version = maybe_version,
                Err(error) => tracing::debug!("Failed to fetch update: {error:?}"),
            },
            Message::RuntimeUpdateCheck(result) => match result {
                Ok(maybe_version) => {
                    if let Some(versions) = &maybe_version {
                        self.log_message(&format!(
                            "[INFO] Runtime update available: {versions}. Open CONSOLE to update."
                        ));
                    } else {
                        self.log_message("[INFO] Runtime tools are up to date.");
                    }
                    self.runtime_update = maybe_version;
                }
                Err(error) => {
                    self.log_message(&format!("[ERROR] Runtime update check failed: {error}"))
                }
            },
            Message::CheckRuntimeUpdates => {
                let sender = self.sender.clone();
                std::thread::spawn(move || {
                    let result = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())
                        .and_then(|runtime| {
                            runtime
                                .block_on(crate::runtime_update::check_for_update())
                                .map_err(|error| error.to_string())
                        });
                    let _ = sender.send(Message::RuntimeUpdateCheck(result));
                });
            }
            Message::InstallRuntimeUpdate => {
                if self.runtime_update_in_progress || self.command.is_running() {
                    return;
                }
                self.runtime_update_in_progress = true;
                let sender = self.sender.clone();
                std::thread::spawn(move || {
                    let result = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())
                        .and_then(|runtime| {
                            runtime
                                .block_on(crate::runtime_update::install_latest())
                                .map_err(|error| error.to_string())
                        });
                    let _ = sender.send(Message::RuntimeUpdateFinished(result));
                });
            }
            Message::RuntimeUpdateFinished(result) => {
                self.runtime_update_in_progress = false;
                match result {
                    Ok(versions) => {
                        self.runtime_update = None;
                        self.log_message(&format!("[INFO] Runtime tools updated: {versions}"));
                    }
                    Err(error) => {
                        self.log_message(&format!("[ERROR] Runtime update failed: {error}"));
                    }
                }
            }
            Message::OpenLink(link) => {
                if let Err(error) = open::that(&link) {
                    tracing::error!("failed to open link {link}: {error}");
                }
            }
            Message::ToggleAdvancedOptions => {
                self.show_advanced_options = !self.show_advanced_options;
            }
            Message::SelectedCookiesBrowser(browser) => {
                let mut cookies = self.cookies_from_browser();
                cookies.browser = browser;
                self.config.cookies_browser = cookies.to_arg();
            }
            Message::SelectedCookiesKeyring(keyring) => {
                let mut cookies = self.cookies_from_browser();
                cookies.keyring = keyring;
                self.config.cookies_browser = cookies.to_arg();
            }
            Message::InputCookiesProfile(profile) => {
                let mut cookies = self.cookies_from_browser();
                cookies.profile = profile;
                self.config.cookies_browser = cookies.to_arg();
            }
            Message::InputCookiesContainer(container) => {
                let mut cookies = self.cookies_from_browser();
                cookies.container = container;
                self.config.cookies_browser = cookies.to_arg();
            }
            Message::OpenDownloadFolder(download_type) => {
                let folder = self.config.download_folder_for(download_type).clone();
                thread::spawn(move || {
                    if let Err(error) = open::that(&folder) {
                        tracing::error!("Failed to open download folder: {error}");
                    }
                });
            }
            Message::PasteUrl => {}
            Message::ClearUrl => self.download_link.clear(),
        }
    }

    fn start_download(&mut self, link: String) {
        self.log_output.clear();
        self.log_message("[INFO] Validating download settings...");
        let current_folder = self.config.download_folder_for(self.download_type).clone();
        let target_folder =
            PathBuf::from(shellexpand::tilde(&current_folder.display().to_string()).to_string());
        *self.config.download_folder_for_mut(self.download_type) = target_folder.clone();

        if !target_folder.exists() {
            self.progress = None;
            let error = DownloadError::DownloadDir(target_folder.clone());
            self.log_message(&format!(
                "[ERROR] Download directory does not exist: {}",
                target_folder.display()
            ));
            self.download_message = Some(Err(error));
            return;
        }

        let raw_links: Vec<&str> = link
            .split(|character: char| {
                character.is_whitespace() || character == ',' || character == ';'
            })
            .map(str::trim)
            .filter(|link| !link.is_empty())
            .collect();

        if raw_links.is_empty() {
            self.progress = None;
            self.log_message("[ERROR] No download URL provided. Please enter a valid URL.");
            self.download_message = Some(Err(DownloadError::NoDownloadURL));
            return;
        }

        for (index, link) in raw_links.iter().enumerate() {
            if Url::parse(link).is_err() {
                self.progress = None;
                self.log_message(&format!(
                    "[ERROR] Invalid URL at index {}: \"{}\"",
                    index + 1,
                    link
                ));
                self.download_message = Some(Err(DownloadError::InvalidURL(index + 1)));
                return;
            }
        }

        self.config
            .update_config_file()
            .expect("update config file");

        let mut args: Vec<&str> = raw_links.to_vec();
        let links_num = raw_links.len();
        let concurrent_str = self.config.concurrent_fragments.to_string();
        args.push("-N");
        args.push(&concurrent_str);

        if !self.config.rate_limit.trim().is_empty() {
            args.push("--limit-rate");
            args.push(self.config.rate_limit.trim());
        }
        if self.config.live_from_start {
            args.push("--live-from-start");
        }

        match self.download_type {
            DownloadType::Video => {
                let resolution = self.config.options.video_resolution.options();
                if !resolution.is_empty() {
                    args.push("-S");
                    args.push(resolution);
                }
                args.push("--remux-video");
                args.push(self.config.options.video_format.options());
                if self.get_thumbnail {
                    args.push("--embed-thumbnail");
                }
            }
            DownloadType::Audio => {
                args.push("-x");
                if self.config.options.audio_format != AudioFormat::Original {
                    args.push("--audio-format");
                    args.push(self.config.options.audio_format.options());
                    args.push("--audio-quality");
                    args.push(self.config.options.audio_quality.options());
                }
                if self.get_thumbnail {
                    args.push("--embed-thumbnail");
                }
            }
        }

        if self.embed_metadata {
            args.push("--embed-metadata");
            args.push("--embed-chapters");
        }
        if self.embed_subtitles {
            args.push("--embed-subs");
            args.push("--write-auto-subs");
            if !self.config.sub_langs.trim().is_empty() {
                args.push("--sub-langs");
                args.push(self.config.sub_langs.trim());
            }
        }
        if let Some(cookies_file) = &self.config.cookies_file {
            args.push("--cookies");
            args.push(cookies_file.to_str().unwrap());
        }
        if let Some(cookies_browser) = &self.config.cookies_browser {
            args.push("--cookies-from-browser");
            args.push(cookies_browser);
        }
        if self.force_overwrite {
            args.push("--force-overwrites");
        }

        let playlist_options = playlist_options(
            self.is_playlist,
            target_folder,
            Some(&self.config.output_template),
        );
        args.extend(playlist_options.iter().map(String::as_str));

        match self.sponsorblock {
            SponsorBlockOption::Disabled => {}
            SponsorBlockOption::Remove => args.push("--sponsorblock-remove=default"),
            SponsorBlockOption::Mark => args.push("--sponsorblock-mark=default"),
        }
        args.extend(self.config.custom_args.split_whitespace());

        tracing::debug!("{args:#?}");
        self.download_message = self.command.start(
            args,
            self.config.bin_path.clone(),
            self.sender.clone(),
            links_num,
            self.show_console,
        );
        self.log_message(&format!(
            "[INFO] Starting yt-dlp download ({} link(s))...",
            links_num
        ));
        if let Some(Err(error)) = &self.download_message {
            self.log_message(&format!("[ERROR] Failed to start yt-dlp: {error}"));
        }
    }

    fn cookies_from_browser(&self) -> CookiesFromBrowser {
        CookiesFromBrowser::from_config(self.config.cookies_browser.as_deref().unwrap_or_default())
    }

    pub fn title(&self) -> String {
        if self.command.is_running() {
            if let Some(progress) = self.progress {
                format!("[{progress:.1}%] lime-dlp")
            } else {
                String::from("[Downloading...] lime-dlp")
            }
        } else {
            String::from("lime-dlp")
        }
    }

    fn log_download(&self) {
        let downloads_log_path = dirs::cache_dir()
            .expect("cache directory")
            .join("lime-dlp/downloads.log");
        if let Some(parent) = downloads_log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&downloads_log_path)
        {
            if let Err(error) = writeln!(
                file,
                "{}::{}::{}::{}",
                Local::now(),
                self.download_link,
                match self.download_type {
                    DownloadType::Video => format!(
                        "{:?}:{:?}",
                        self.config.options.video_resolution, self.config.options.video_format
                    ),
                    DownloadType::Audio => format!(
                        "{:?}:{:?}",
                        self.config.options.audio_quality, self.config.options.audio_format
                    ),
                },
                self.config
                    .download_folder_for(self.download_type)
                    .to_string_lossy()
            ) {
                tracing::error!("failed to log download: {error}");
            }
        }
    }

    pub fn log_message(&mut self, message: &str) {
        if !self.log_output.is_empty() {
            self.log_output.push('\n');
        }
        self.log_output.push_str(message);

        let trimmed = message.trim();
        let lower = trimmed.to_lowercase();
        if trimmed.starts_with("[ERROR]")
            || trimmed.starts_with("[WARN]")
            || lower.contains("error")
            || lower.contains("fail")
        {
            log_error_to_file(trimmed);
        }
    }

    pub fn end_download(&mut self, download_message: Option<Result<String, DownloadError>>) {
        self.command.kill();
        self.progress = None;
        match &download_message {
            Some(Ok(message)) => {
                self.log_message(&format!("[SUCCESS] {message}"));
                let _ = Notification::new().summary(message).show();
            }
            Some(Err(error)) => {
                self.log_message(&format!("[ERROR] {error}"));
                let _ = Notification::new().summary(&error.to_string()).show();
            }
            None => {}
        }
        self.download_message = download_message;
        self.log_download();
    }

    fn video_panel(&self, form: &FormInputs, cx: &Context<Self>) -> AnyElement {
        let content = if self.download_message.is_some() {
            self.download_status(cx)
        } else {
            let mut options = div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .p_3()
                .bg(rgba(0x13201899))
                .rounded_md()
                .child(select_dropdown(
                    "Resolution",
                    self.config.options.video_resolution,
                    &VIDEO_RESOLUTIONS,
                    set_video_resolution,
                    cx,
                ))
                .child(select_dropdown(
                    "Format",
                    self.config.options.video_format,
                    &VIDEO_FORMATS,
                    set_video_format,
                    cx,
                ));
            if self.show_advanced_options {
                options = options.child(self.advanced_options(cx));
            }
            options.into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(content)
            .child(self.download_path(DownloadType::Video, form, cx))
            .child(self.download_controls(cx))
            .into_any_element()
    }

    fn audio_panel(&self, form: &FormInputs, cx: &Context<Self>) -> AnyElement {
        let content = if self.download_message.is_some() {
            self.download_status(cx)
        } else {
            let mut options = div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .p_3()
                .bg(rgba(0x13201899))
                .rounded_md()
                .child(select_dropdown(
                    "Quality",
                    self.config.options.audio_quality,
                    &AUDIO_QUALITIES,
                    set_audio_quality,
                    cx,
                ))
                .child(select_dropdown(
                    "Format",
                    self.config.options.audio_format,
                    &AUDIO_FORMATS,
                    set_audio_format,
                    cx,
                ));
            if self.show_advanced_options {
                options = options.child(self.advanced_options(cx));
            }
            options.into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(content)
            .child(self.download_path(DownloadType::Audio, form, cx))
            .child(self.download_controls(cx))
            .into_any_element()
    }

    fn advanced_options(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgba(0x101a12b8))
            .border_1()
            .border_color(rgb(0x38552f))
            .rounded_md()
            .child(section_title("ADVANCED DOWNLOAD OPTIONS"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(toggle_button(
                        "THUMBNAIL",
                        self.get_thumbnail,
                        Message::ToggleThumbnail(!self.get_thumbnail),
                        cx,
                    ))
                    .child(toggle_button(
                        "METADATA + CHAPTERS",
                        self.embed_metadata,
                        Message::ToggleMetadata(!self.embed_metadata),
                        cx,
                    ))
                    .child(toggle_button(
                        "SUBTITLES",
                        self.embed_subtitles,
                        Message::ToggleSubtitles(!self.embed_subtitles),
                        cx,
                    ))
                    .child(toggle_button(
                        "FORCE OVERWRITE",
                        self.force_overwrite,
                        Message::ToggleForceOverwrite(!self.force_overwrite),
                        cx,
                    ))
                    .child(toggle_button(
                        "LIVE FROM START",
                        self.config.live_from_start,
                        Message::ToggleLiveFromStart(!self.config.live_from_start),
                        cx,
                    ))
                    .child(toggle_button(
                        "CONSOLE",
                        self.show_console,
                        Message::ToggleConsole(!self.show_console),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_end()
                    .gap_2()
                    .child(select_dropdown(
                        "SponsorBlock",
                        self.sponsorblock,
                        &SPONSORBLOCK_OPTIONS,
                        set_sponsorblock,
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn download_path(
        &self,
        download_type: DownloadType,
        form: &FormInputs,
        cx: &Context<Self>,
    ) -> AnyElement {
        let input = match download_type {
            DownloadType::Video => &form.video_folder,
            DownloadType::Audio => &form.audio_folder,
        };
        div()
            .flex()
            .items_end()
            .gap_2()
            .child(field("Download folder", input))
            .child(action_button(
                format!("browse-folder-{download_type:?}"),
                "BROWSE",
                ButtonKind::Secondary,
                Message::SelectDownloadFolder(download_type),
                cx,
            ))
            .child(action_button(
                format!("open-folder-{download_type:?}"),
                "OPEN FOLDER",
                ButtonKind::Secondary,
                Message::OpenDownloadFolder(download_type),
                cx,
            ))
            .into_any_element()
    }

    fn download_controls(&self, cx: &Context<Self>) -> AnyElement {
        let (label, message, kind) = if self.command.is_running() {
            (
                "STOP DOWNLOAD".to_string(),
                Message::StopDownload,
                ButtonKind::Danger,
            )
        } else {
            (
                fl!("download"),
                Message::StartDownload(self.download_link.clone()),
                ButtonKind::Primary,
            )
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .child(match self.active_tab {
                        Tab::Video => "VIDEO OUTPUT",
                        Tab::Audio => "AUDIO OUTPUT",
                        Tab::Settings => "DOWNLOAD SETTINGS",
                    })
                    .font_family(FALLOUT)
                    .text_xs()
                    .text_color(rgb(MUTED)),
            )
            .child(action_button("download-control", label, kind, message, cx))
            .into_any_element()
    }

    fn download_status(&self, cx: &Context<Self>) -> AnyElement {
        let (message, color) = match &self.download_message {
            Some(Ok(message)) => (message.to_string(), LIME),
            Some(Err(error)) => (error.to_string(), 0xff7b8b),
            None => (String::new(), MUTED),
        };
        let progress = self
            .progress
            .filter(|progress| progress.is_finite())
            .unwrap_or_default()
            .clamp(0.0, 100.0);
        let mut status = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgba(0x13201899))
            .border_1()
            .border_color(rgb(color))
            .rounded_md()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().flex_1().child(message).text_color(rgb(color)))
                    .child(
                        div()
                            .child(self.playlist_progress.clone().unwrap_or_default())
                            .text_sm()
                            .text_color(rgb(MUTED)),
                    )
                    .child(action_button(
                        "clear-download-status",
                        if self.command.is_running() {
                            "STOP"
                        } else {
                            "DISMISS"
                        },
                        ButtonKind::Danger,
                        Message::StopDownload,
                        cx,
                    )),
            );
        if self.progress.is_some() {
            status = status.child(
                div().h_2().w_full().bg(rgb(0x253b28)).rounded_sm().child(
                    div()
                        .h_full()
                        .w(relative(progress / 100.0))
                        .bg(rgb(LIME))
                        .rounded_sm(),
                ),
            );
        }
        status.into_any_element()
    }

    fn settings_panel(&self, form: &FormInputs, cx: &Context<Self>) -> AnyElement {
        let cookies = self.cookies_from_browser();
        let mut paths = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(section_title("DOWNLOAD PATHS AND TOOLS"))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .child(field("yt-dlp binary", &form.bin_path))
                    .child(action_button(
                        "browse-ytdlp",
                        "BROWSE",
                        ButtonKind::Secondary,
                        Message::SelectYtDlpBinPath,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .child(field("Cookies file", &form.cookies_file))
                    .child(action_button(
                        "browse-cookies",
                        "BROWSE",
                        ButtonKind::Secondary,
                        Message::SelectCookiesFile,
                        cx,
                    )),
            );

        let cookie_options = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(section_title("BROWSER COOKIES"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_end()
                    .gap_2()
                    .child(select_dropdown(
                        "Browser",
                        cookies.browser,
                        &Browser::ALL,
                        set_cookies_browser,
                        cx,
                    ))
                    .when(cookies.browser.supports_keyring(), |this| {
                        this.child(select_dropdown(
                            "Keyring",
                            cookies.keyring,
                            &Keyring::ALL,
                            set_cookies_keyring,
                            cx,
                        ))
                    }),
            );
        let cookie_options = if cookies.browser == Browser::Disabled {
            cookie_options
        } else {
            let cookie_options =
                cookie_options.child(field("Browser profile", &form.cookies_profile));
            if cookies.browser.supports_container() {
                cookie_options.child(field("Firefox container", &form.cookies_container))
            } else {
                cookie_options
            }
        };

        let options = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(section_title("OUTPUT AND PERFORMANCE"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_end()
                    .gap_2()
                    .child(select_dropdown(
                        "Concurrent fragments",
                        self.config.concurrent_fragments,
                        &CONCURRENT_FRAGMENTS,
                        set_concurrent_fragments,
                        cx,
                    ))
                    .child(field("Rate limit", &form.rate_limit)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_end()
                    .gap_2()
                    .child(field("Output template", &form.output_template))
                    .child(field("Subtitle languages", &form.sub_langs)),
            )
            .child(field("Custom yt-dlp arguments", &form.custom_args));

        paths = paths.child(cookie_options).child(options);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .bg(rgba(0x13201899))
            .border_1()
            .border_color(rgb(0x38552f))
            .rounded_md()
            .child(section_title("SETTINGS"))
            .child(toggle_button(
                "SAVE WINDOW POSITION",
                self.config.save_window_position,
                Message::ToggleSaveWindowPosition(!self.config.save_window_position),
                cx,
            ))
            .child(paths)
            .into_any_element()
    }

    fn console(&self, cx: &Context<Self>) -> AnyElement {
        let lines = if self.log_output.is_empty() {
            vec!["[Console ready. Output and status will appear here...]".to_string()]
        } else {
            self.log_output.lines().map(str::to_string).collect()
        };
        let color = if self.command.is_running() {
            LIME
        } else {
            MUTED
        };
        let copy_text = self.log_output.clone();
        div()
            .flex()
            .flex_col()
            .mx_2()
            .mb_2()
            .p_2()
            .w_full()
            .h_48()
            .bg(rgba(0x08100be6))
            .border_1()
            .border_color(rgb(0x38552f))
            .rounded_md()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .child(if self.command.is_running() {
                                "CONSOLE (ACTIVE)"
                            } else {
                                "CONSOLE (IDLE)"
                            })
                            .font_family(FALLOUT)
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(color)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(clipboard_button("COPY", copy_text, cx))
                            .child(action_button(
                                "runtime-update",
                                if self.runtime_update_in_progress {
                                    "UPDATING..."
                                } else if self.runtime_update.is_some() {
                                    "UPDATE TOOLS"
                                } else {
                                    "CHECK UPDATES"
                                },
                                ButtonKind::Info,
                                if self.runtime_update.is_some() {
                                    Message::InstallRuntimeUpdate
                                } else {
                                    Message::CheckRuntimeUpdates
                                },
                                cx,
                            ))
                            .child(action_button(
                                "clear-console",
                                "CLEAR",
                                ButtonKind::Secondary,
                                Message::ClearConsole,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .children(lines.into_iter().map(|line| div().child(line))),
            )
            .into_any_element()
    }

    fn render_tab(&self, form: &FormInputs, cx: &Context<Self>) -> AnyElement {
        match self.active_tab {
            Tab::Video => self.video_panel(form, cx),
            Tab::Audio => self.audio_panel(form, cx),
            Tab::Settings => self.settings_panel(form, cx),
        }
    }
}

impl Render for YtGUI {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_form(window, cx);
        self.sync_form(cx);
        let form = self.form.as_ref().expect("form initialized").clone();
        let active = self.active_tab;
        let url_input = component::input::Input::new(&form.download_url)
            .flex_1()
            .min_w_0()
            .h_9();

        let tabs = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .child(action_button(
                "tab-video",
                fl!("video"),
                if active == Tab::Video {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                Message::SelectTab(Tab::Video),
                cx,
            ))
            .child(action_button(
                "tab-audio",
                fl!("audio"),
                if active == Tab::Audio {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                Message::SelectTab(Tab::Audio),
                cx,
            ))
            .child(action_button(
                "tab-settings",
                fl!("settings"),
                if active == Tab::Settings {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                Message::SelectTab(Tab::Settings),
                cx,
            ));

        let download_view = self.render_tab(&form, cx);
        let body = div()
            .id("lime-dlp-content")
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .flex_col()
            .gap_3()
            .p_3()
            .child(download_view);

        let mut root = div()
            .id("lime-dlp-root")
            .flex()
            .flex_col()
            .size_full()
            .bg(rgba(PANEL))
            .font_family("Fixedsys Excelsior")
            .text_color(rgb(TEXT))
            .border_1()
            .border_color(rgb(LIME))
            .child(title_bar(self.title()))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .child(url_input)
                    .child(action_button(
                        "paste-url",
                        "PASTE",
                        ButtonKind::Secondary,
                        Message::PasteUrl,
                        cx,
                    ))
                    .child(action_button(
                        "clear-url",
                        "CLEAR",
                        ButtonKind::Secondary,
                        Message::ClearUrl,
                        cx,
                    ))
                    .child(toggle_button(
                        "PLAYLIST",
                        self.is_playlist,
                        Message::TogglePlaylist(!self.is_playlist),
                        cx,
                    ))
                    .child(action_button(
                        "toggle-advanced",
                        if self.show_advanced_options {
                            "LESS"
                        } else {
                            "ADVANCED"
                        },
                        ButtonKind::Secondary,
                        Message::ToggleAdvancedOptions,
                        cx,
                    ))
                    .child(toggle_button(
                        "CONSOLE",
                        self.show_console,
                        Message::ToggleConsole(!self.show_console),
                        cx,
                    )),
            )
            .child(tabs)
            .child(body);

        if self.show_console {
            root = root.child(self.console(cx));
        }

        if let Some(version) = &self.new_version {
            root = root.child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .px_3()
                    .pb_2()
                    .child(
                        div()
                            .child(format!("New version available: {version}"))
                            .text_sm()
                            .text_color(rgb(MUTED)),
                    )
                    .child(action_button(
                        "open-release",
                        "VIEW RELEASE",
                        ButtonKind::Info,
                        Message::OpenLink(
                            "https://github.com/BKSalman/ytdlp-gui/releases/latest".into(),
                        ),
                        cx,
                    )),
            );
        }
        root
    }
}

fn input(
    window: &mut Window,
    cx: &mut Context<YtGUI>,
    placeholder: &'static str,
    value: &str,
) -> InputEntity {
    let value = value.to_string();
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn title_bar(title: String) -> impl IntoElement {
    div()
        .id("lime-dlp-titlebar")
        .font_family(FALLOUT)
        .flex()
        .items_center()
        .h(px(34.0))
        .w_full()
        .bg(rgba(SURFACE))
        .border_b_1()
        .border_color(rgb(LIME))
        .child(
            div()
                .flex()
                .items_center()
                .id("lime-dlp-title-drag")
                .flex_1()
                .h_full()
                .px_4()
                .on_double_click(|_, window, _| window.zoom_window())
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .child(title)
                        .font_family("Fixedsys Excelsior")
                        .text_size(px(18.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(LIME)),
                ),
        )
        .child(window_control("MIN"))
        .child(window_control("MAX"))
        .child(window_control("X"))
}

fn window_control(label: &'static str) -> impl IntoElement {
    div()
        .id(format!("lime-dlp-window-{label}"))
        .font_family("Fixedsys Excelsior")
        .flex()
        .items_center()
        .justify_center()
        .w(px(44.0))
        .h(px(34.0))
        .bg(rgba(0x203a26ff))
        .border_1()
        .border_color(rgb(LIME))
        .text_sm()
        .text_color(rgb(TEXT))
        .hover(|style| style.bg(rgba(0x38552fff)).text_color(rgb(LIME)))
        .child(match label {
            "MIN" => "-",
            "MAX" => "[ ]",
            "X" => "X",
            _ => "?",
        })
        .on_click(move |_, window, _| match label {
            "MIN" => window.minimize_window(),
            "MAX" => window.zoom_window(),
            "X" => window.remove_window(),
            _ => {}
        })
}

fn action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    message: Message,
    cx: &Context<YtGUI>,
) -> impl IntoElement {
    let mut button = component::button::Button::new(id)
        .font_family("Fixedsys Excelsior")
        .label(label)
        .compact()
        .text_xs()
        .text_size(px(13.0))
        .outline();
    button = match kind {
        ButtonKind::Primary => button.success(),
        ButtonKind::Secondary => button.secondary(),
        ButtonKind::Danger => button.danger(),
        ButtonKind::Info => button.info(),
    };
    button.on_click(cx.listener(move |app, _, window, cx| {
        app.handle_message(message.clone(), window, cx);
    }))
}

fn toggle_button(
    label: &'static str,
    active: bool,
    message: Message,
    cx: &Context<YtGUI>,
) -> impl IntoElement {
    action_button(
        format!("toggle-{label}"),
        if active {
            format!("✓ {label}")
        } else {
            label.to_string()
        },
        if active {
            ButtonKind::Primary
        } else {
            ButtonKind::Secondary
        },
        message,
        cx,
    )
}

fn clipboard_button(
    label: &'static str,
    contents: String,
    cx: &Context<YtGUI>,
) -> impl IntoElement {
    component::button::Button::new(format!("copy-{label}"))
        .font_family("Fixedsys Excelsior")
        .label(label)
        .compact()
        .text_xs()
        .outline()
        .secondary()
        .on_click(cx.listener(move |_app, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(contents.clone()));
        }))
}

fn section_title(label: &'static str) -> impl IntoElement {
    div()
        .child(label)
        .font_family(FALLOUT)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(LIME))
}

fn field(label: &'static str, input: &InputEntity) -> AnyElement {
    div()
        .flex_1()
        .flex_col()
        .gap_1()
        .child(
            div()
                .child(label)
                .font_family(FALLOUT)
                .text_size(px(13.0))
                .text_color(rgb(MUTED)),
        )
        .child(
            Input::new(input)
                .h_8()
                .bg(rgba(0x101a12e6))
                .border_1()
                .border_color(rgb(0x38552f))
                .rounded_md()
                .px_2(),
        )
        .into_any_element()
}

fn select_dropdown<T: Copy + core::fmt::Display + 'static>(
    label: &'static str,
    current: T,
    choices: &'static [T],
    update: fn(&mut YtGUI, T),
    cx: &Context<YtGUI>,
) -> AnyElement {
    let weak_entity = cx.weak_entity();
    let mut dropdown = component::button::DropdownButton::new(format!("select-{label}"))
        .button(
            component::button::Button::new(format!("select-{label}-value"))
                .label(current.to_string())
                .compact()
                .outline()
                .secondary(),
        )
        .outline();
    dropdown = dropdown.dropdown_menu(move |menu, _window, _cx| {
        choices.iter().fold(menu, |menu, choice| {
            let weak_entity = weak_entity.clone();
            let value = *choice;
            menu.item(
                PopupMenuItem::new(value.to_string()).on_click(move |_event, _window, cx| {
                    let _ = weak_entity.update(cx, |app, cx| {
                        update(app, value);
                        cx.notify();
                    });
                }),
            )
        })
    });
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .child(label)
                .font_family(FALLOUT)
                .text_size(px(13.0))
                .text_color(rgb(MUTED)),
        )
        .child(dropdown)
        .into_any_element()
}

fn choose_folder(
    starting_dir: impl AsRef<Path>,
) -> impl Future<Output = Option<PathBuf>> + Send + 'static {
    let starting_dir = starting_dir.as_ref().to_path_buf();
    async move {
        rfd::AsyncFileDialog::new()
            .set_directory(starting_dir)
            .pick_folder()
            .await
            .map(|folder| folder.path().to_path_buf())
    }
}

fn choose_file(
    starting_dir: impl AsRef<Path>,
) -> impl Future<Output = Option<PathBuf>> + Send + 'static {
    let starting_dir = starting_dir.as_ref().to_path_buf();
    async move {
        rfd::AsyncFileDialog::new()
            .set_directory(starting_dir)
            .pick_file()
            .await
            .map(|file| file.path().to_path_buf())
    }
}

pub fn log_error_to_file(message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .append(true)
        .create(true)
        .open("lime-dlp.log")
    {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

const VIDEO_RESOLUTIONS: [VideoResolution; 6] = [
    VideoResolution::Source,
    VideoResolution::FourK,
    VideoResolution::TwoK,
    VideoResolution::FullHD,
    VideoResolution::Hd,
    VideoResolution::Sd,
];
const VIDEO_FORMATS: [VideoFormat; 6] = [
    VideoFormat::Mp4,
    VideoFormat::Mkv,
    VideoFormat::Webm,
    VideoFormat::Avi,
    VideoFormat::Mov,
    VideoFormat::Flv,
];
const AUDIO_QUALITIES: [AudioQuality; 7] = [
    AudioQuality::Best,
    AudioQuality::Ultra,
    AudioQuality::VeryHigh,
    AudioQuality::High,
    AudioQuality::Good,
    AudioQuality::Medium,
    AudioQuality::Low,
];
const AUDIO_FORMATS: [AudioFormat; 7] = [
    AudioFormat::Mp3,
    AudioFormat::Original,
    AudioFormat::M4a,
    AudioFormat::Opus,
    AudioFormat::Flac,
    AudioFormat::Wav,
    AudioFormat::Vorbis,
];
const SPONSORBLOCK_OPTIONS: [SponsorBlockOption; 3] = [
    SponsorBlockOption::Disabled,
    SponsorBlockOption::Remove,
    SponsorBlockOption::Mark,
];
const CONCURRENT_FRAGMENTS: [u8; 5] = [1, 2, 4, 8, 16];

fn set_video_resolution(app: &mut YtGUI, value: VideoResolution) {
    app.config.options.video_resolution = value;
}
fn set_video_format(app: &mut YtGUI, value: VideoFormat) {
    app.config.options.video_format = value;
}
fn set_audio_quality(app: &mut YtGUI, value: AudioQuality) {
    app.config.options.audio_quality = value;
}
fn set_audio_format(app: &mut YtGUI, value: AudioFormat) {
    app.config.options.audio_format = value;
}
fn set_sponsorblock(app: &mut YtGUI, value: SponsorBlockOption) {
    app.sponsorblock = value;
}
fn set_concurrent_fragments(app: &mut YtGUI, value: u8) {
    app.config.concurrent_fragments = value;
}
fn set_cookies_browser(app: &mut YtGUI, value: Browser) {
    let mut cookies = app.cookies_from_browser();
    cookies.browser = value;
    app.config.cookies_browser = cookies.to_arg();
}
fn set_cookies_keyring(app: &mut YtGUI, value: Keyring) {
    let mut cookies = app.cookies_from_browser();
    cookies.keyring = value;
    app.config.cookies_browser = cookies.to_arg();
}
