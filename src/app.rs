use std::path::PathBuf;

use iced::widget::{
    column, container, pick_list, rich_text, row, scrollable, space, span, text, text_editor,
    text_input,
};
use iced::{Alignment, Color, Event, Length, Padding, Point, Subscription, window};
use iced_aw::Tabs;
use url::Url;

use crate::collapsible::collapsible;
use crate::cookies::{Browser, CookiesFromBrowser, Keyring};
use crate::error::DownloadError;
use crate::i18n::{dir_row, is_rtl};
use crate::media_options::{AudioFormat, Options, playlist_options};
use crate::sponsorblock::SponsorBlockOption;
use crate::theme::{
    LIME_HIGHLIGHT, button, console_container_style, pick_list_menu_style, pick_list_style,
    primary_button, tab_bar_style,
};
use crate::{Message, WindowPosition, YtGUI, choose_file, choose_folder};
use crate::{checkbox::checkbox, fl};
use notify_rust::Notification;

pub const FONT_SIZE: f32 = 18.;

pub const SPACING: f32 = 10.;

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

impl YtGUI {
    pub fn update(&mut self, event: Message) -> iced::Task<Message> {
        match event {
            Message::InputChanged(input) => {
                self.download_link = input.clone();
                self.download_link_content = iced::widget::text_editor::Content::with_text(&input);
                if !self.command.is_running() && self.download_message.is_some() {
                    self.download_message = None;
                    self.progress = None;
                }
            }
            Message::EditorAction(action) => {
                self.download_link_content.perform(action);
                self.download_link = self.download_link_content.text();
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
            Message::SelectDownloadFolder(download_type) => {
                if !self.is_file_dialog_open {
                    self.is_file_dialog_open = true;
                    let starting_dir = self.config.download_folder_for(download_type).clone();

                    return iced::Task::perform(choose_folder(starting_dir), move |folder| {
                        Message::SelectedDownloadFolder(download_type, folder)
                    });
                }
            }
            Message::SelectedDownloadFolder(download_type, folder) => {
                if let Some(path) = folder {
                    *self.config.download_folder_for_mut(download_type) = path;
                }
                self.is_file_dialog_open = false;
            }
            Message::DownloadFolderTextInput(download_type, folder_string) => {
                let path = PathBuf::from(&folder_string.to_string());
                *self.config.download_folder_for_mut(download_type) = path;
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
                    _ => {}
                }
                // Dismiss error or completed state when switching tabs if download is idle
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
                            log_error_to_file(&format!("[yt-dlp stderr] {}", err_trimmed));
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
                            log_error_to_file(&format!("[yt-dlp stdout] {}", trimmed));
                        }
                        self.log_message(trimmed);
                    }
                }
                self.handle_progress_event(&progress);
            }
            Message::ClearConsole => {
                self.log_output.clear();
            }
            Message::IcedEvent(event) => {
                if let Event::Window(window_event) = event {
                    match window_event {
                        window::Event::CloseRequested => {
                            self.command.kill();
                            self.config.window_position = Some(WindowPosition {
                                x: self.window_pos.x,
                                y: self.window_pos.y,
                            });
                            if let Err(e) = self.config.update_config_file() {
                                tracing::error!("Failed to update config file: {e}");
                            }
                            return window::latest().and_then(|id| window::close(id));
                        }
                        window::Event::Resized(size) => {
                            self.window_width = size.width;
                            self.window_height = size.height;
                        }
                        window::Event::Moved(pos) if self.config.save_window_position => {
                            self.window_pos = Point::new(pos.x, pos.y);
                        }
                        window::Event::Opened {
                            position: _,
                            size: _,
                        } => {
                            return iced::widget::operation::focus(
                                self.download_text_input_id.clone(),
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::StartDownload(link) => {
                self.log_output.clear();
                self.log_message("[INFO] Validating download settings...");
                let current_folder = self.config.download_folder_for(self.download_type).clone();
                let target_folder = PathBuf::from(
                    shellexpand::tilde(&current_folder.display().to_string()).to_string(),
                );
                *self.config.download_folder_for_mut(self.download_type) = target_folder.clone();

                if !target_folder.exists() {
                    self.progress = None;
                    let err = DownloadError::DownloadDir(target_folder.clone());
                    self.log_message(&format!(
                        "[ERROR] Download directory does not exist: {}",
                        target_folder.display()
                    ));
                    self.download_message = Some(Err(err));
                    return iced::Task::none();
                }

                let raw_links: Vec<&str> = link
                    .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();

                if raw_links.is_empty() {
                    self.progress = None;
                    let err = DownloadError::NoDownloadURL;
                    self.log_message("[ERROR] No download URL provided. Please enter a valid URL.");
                    self.download_message = Some(Err(err));
                    return iced::Task::none();
                }

                for (i, l) in raw_links.iter().enumerate() {
                    if Url::parse(l).is_err() {
                        self.progress = None;
                        let err = DownloadError::InvalidURL(i + 1);
                        self.log_message(&format!(
                            "[ERROR] Invalid URL at index {}: \"{}\"",
                            i + 1,
                            l
                        ));
                        self.download_message = Some(Err(err));
                        return iced::Task::none();
                    }
                }

                self.config
                    .update_config_file()
                    .expect("update config file");

                let mut args: Vec<&str> = Vec::new();

                for l in &raw_links {
                    args.push(l);
                }
                let links_num = raw_links.len();

                // Multi-threaded fragment downloading
                let concurrent_str = self.config.concurrent_fragments.to_string();
                args.push("-N");
                args.push(&concurrent_str);

                // Rate limiting
                if !self.config.rate_limit.trim().is_empty() {
                    args.push("--limit-rate");
                    args.push(self.config.rate_limit.trim());
                }

                // Live stream from start
                if self.config.live_from_start {
                    args.push("--live-from-start");
                }

                match self.download_type {
                    DownloadType::Video => {
                        let res_opt = self.config.options.video_resolution.options();
                        if !res_opt.is_empty() {
                            args.push("-S");
                            args.push(res_opt);
                        }

                        args.push("--remux-video");

                        args.push(self.config.options.video_format.options());

                        if self.get_thumbnail {
                            args.push("--embed-thumbnail")
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
                            args.push("--embed-thumbnail")
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

                args.append(&mut playlist_options.iter().map(|s| &**s).collect());

                match self.sponsorblock {
                    SponsorBlockOption::Disabled => {}
                    SponsorBlockOption::Remove => {
                        args.push("--sponsorblock-remove=default");
                    }
                    SponsorBlockOption::Mark => {
                        args.push("--sponsorblock-mark=default");
                    }
                }

                let custom_arg_tokens: Vec<&str> =
                    self.config.custom_args.split_whitespace().collect();
                for carg in custom_arg_tokens {
                    args.push(carg);
                }

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
                if let Some(Err(ref e)) = self.download_message {
                    self.log_message(&format!("[ERROR] Failed to start yt-dlp: {e}"));
                }
            }
            Message::StopDownload => {
                self.command.kill();
                self.log_message("[WARN] Download stopped by user.");
                let _ = self.progress.take();
                let _ = self.download_message.take();
            }
            Message::ToggleSaveWindowPosition(save_window_position) => {
                self.config.save_window_position = save_window_position;
            }
            Message::SelectYtDlpBinPath => {
                if !self.is_file_dialog_open {
                    self.is_file_dialog_open = true;

                    return iced::Task::perform(
                        choose_file(self.config.bin_path.clone().unwrap_or("~".into())),
                        Message::SelectedYtDlpBinPath,
                    );
                }
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
            Message::SelectCookiesFile => {
                if !self.is_file_dialog_open {
                    self.is_file_dialog_open = true;

                    return iced::Task::perform(
                        choose_file(self.config.cookies_file.clone().unwrap_or("~".into())),
                        Message::SelectedCookiesFile,
                    );
                }
            }
            Message::SelectedCookiesFile(file) => {
                if let Some(path) = file {
                    self.config.cookies_file = Some(path);
                }
                self.is_file_dialog_open = false;
            }
            Message::SelectCookiesFileTextInput(cookies_string) => {
                if cookies_string.is_empty() {
                    self.config.cookies_file = None;
                } else {
                    let path = PathBuf::from(cookies_string);

                    self.config.cookies_file = Some(path);
                }
            }
            Message::UpdateCheck(res) => match res {
                Ok(maybe_version) => {
                    self.new_version = maybe_version;
                }
                Err(e) => {
                    eprintln!("Failed to fetch update: {e:?}");
                }
            },
            Message::OpenLink(link) => {
                println!("opening: {link}");
                if let Err(e) = open::that(&link) {
                    eprintln!("failed to open link {link}: {e}");
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
                std::thread::spawn(move || {
                    if let Err(e) = open::that(&folder) {
                        tracing::error!("Failed to open download folder: {e}");
                    }
                });
            }
            Message::PasteUrl => {
                return iced::clipboard::read().map(|content| match content {
                    Some(s) => Message::InputChanged(s),
                    None => Message::Noop,
                });
            }
            Message::ClearUrl => {
                self.download_link = String::new();
                self.download_link_content = iced::widget::text_editor::Content::new();
            }
            Message::Noop => {}
        }

        iced::Task::none()
    }

    fn cookies_from_browser(&self) -> CookiesFromBrowser {
        CookiesFromBrowser::from_config(self.config.cookies_browser.as_deref().unwrap_or_default())
    }

    pub fn title(&self) -> String {
        if self.command.is_running() {
            if let Some(progress) = self.progress {
                format!("[{:.1}%] lime-dlp", progress)
            } else {
                String::from("[Downloading...] lime-dlp")
            }
        } else {
            String::from("lime-dlp")
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let advanced_options = || {
            let mut options = column![
                dir_row(vec![
                    text(format!("{}:", fl!("sponsorblock"))).into(),
                    pick_list(
                        vec![
                            SponsorBlockOption::Disabled,
                            SponsorBlockOption::Remove,
                            SponsorBlockOption::Mark,
                        ],
                        Some(self.sponsorblock),
                        Message::SelectedSponsorBlockOption
                    )
                    .style(pick_list_style)
                    .menu_style(pick_list_menu_style)
                    .into()
                ])
                .spacing(SPACING)
                .align_y(Alignment::Center),
                checkbox(self.get_thumbnail)
                    .label(fl!("embed-thumbnail"))
                    .on_toggle(Message::ToggleThumbnail),
                checkbox(self.embed_metadata)
                    .label("Embed Metadata & Chapters")
                    .on_toggle(Message::ToggleMetadata),
                checkbox(self.embed_subtitles)
                    .label("Embed Subtitles")
                    .on_toggle(Message::ToggleSubtitles),
                checkbox(self.force_overwrite)
                    .label(fl!("force-overwrite"))
                    .on_toggle(Message::ToggleForceOverwrite),
                checkbox(self.config.live_from_start)
                    .label("Live from Start (--live-from-start)")
                    .on_toggle(Message::ToggleLiveFromStart),
                checkbox(self.show_console)
                    .label("Show yt-dlp Console Window")
                    .on_toggle(Message::ToggleConsole),
            ];

            if is_rtl() {
                options = options.align_x(Alignment::End);
            }

            collapsible(
                fl!("advanced-options"),
                self.show_advanced_options,
                Message::ToggleAdvancedOptions,
                options.padding(12).spacing(SPACING),
            )
        };

        let download_path = |download_type: DownloadType| {
            let target_folder = self.config.download_folder_for(download_type);
            let mut download_path_input =
                text_input(&fl!("download_path"), &target_folder.display().to_string())
                    .on_input(move |s| Message::DownloadFolderTextInput(download_type, s))
                    .on_submit(Message::SelectDownloadFolderTextInput(download_type));

            if is_rtl() {
                download_path_input = download_path_input.align_x(Alignment::End);
            }

            dir_row(vec![
                download_path_input.into(),
                button(text(fl!("browse")))
                    .on_press(Message::SelectDownloadFolder(download_type))
                    .into(),
                button(text("Open Folder"))
                    .on_press(Message::OpenDownloadFolder(download_type))
                    .into(),
            ])
            .spacing(SPACING)
            .align_y(iced::Alignment::Center)
        };

        let video_tab = column![
            row![if let Some(download_message) = &self.download_message {
                self.show_download_progress(download_message)
            } else {
                column![
                    dir_row(vec![
                        Options::video_resolutions(self.config.options.video_resolution).into(),
                        space::horizontal().into(),
                        Options::video_formats(self.config.options.video_format).into()
                    ])
                    .padding(12),
                    advanced_options(),
                ]
                .width(Length::Fill)
            }],
            column![
                download_path(DownloadType::Video),
                row![if !self.command.is_running() {
                    primary_button(text(fl!("download")))
                        .on_press(Message::StartDownload(self.download_link.clone()))
                } else {
                    primary_button(text(fl!("download")))
                }]
            ]
            .width(Length::Fill)
            .align_x(iced::Alignment::Center)
            .spacing(20)
            .padding(Padding::ZERO.top(20).horizontal(20))
        ];

        let audio_tab = column![
            row![if let Some(download_message) = &self.download_message {
                self.show_download_progress(download_message)
            } else {
                column![
                    dir_row(vec![
                        Options::audio_qualities(self.config.options.audio_quality).into(),
                        space::horizontal().into(),
                        Options::audio_formats(self.config.options.audio_format).into(),
                    ])
                    .padding(12),
                    advanced_options(),
                ]
            }],
            column![
                download_path(DownloadType::Audio),
                row![if !self.command.is_running() {
                    primary_button(text(fl!("download")))
                        .on_press(Message::StartDownload(self.download_link.clone()))
                } else {
                    primary_button(text(fl!("download")))
                }]
            ]
            .width(Length::Fill)
            .align_x(iced::Alignment::Center)
            .spacing(20)
            .padding(Padding::ZERO.top(20).horizontal(20))
        ];

        let cookies = self.cookies_from_browser();

        let mut cookies_from_browser = vec![
            dir_row(vec![
                text(format!("{}:", fl!("cookies_from_browser"))).into(),
                pick_list(
                    Browser::ALL,
                    Some(cookies.browser),
                    Message::SelectedCookiesBrowser,
                )
                .style(pick_list_style)
                .menu_style(pick_list_menu_style)
                .into(),
            ])
            .spacing(SPACING)
            .align_y(Alignment::Center)
            .into(),
        ];

        if cookies.browser.supports_keyring() {
            cookies_from_browser.push(
                dir_row(vec![
                    text(format!("{}:", fl!("cookies_keyring"))).into(),
                    pick_list(
                        Keyring::ALL,
                        Some(cookies.keyring),
                        Message::SelectedCookiesKeyring,
                    )
                    .style(pick_list_style)
                    .menu_style(pick_list_menu_style)
                    .into(),
                ])
                .spacing(SPACING)
                .align_y(Alignment::Center)
                .into(),
            );
        }

        if cookies.browser != Browser::Disabled {
            cookies_from_browser.push(
                dir_row(vec![
                    text(format!("{}:", fl!("cookies_profile"))).into(),
                    text_input(&fl!("cookies_profile_placeholder"), &cookies.profile)
                        .on_input(Message::InputCookiesProfile)
                        .into(),
                ])
                .spacing(SPACING)
                .align_y(Alignment::Center)
                .into(),
            );
        }

        if cookies.browser.supports_container() {
            cookies_from_browser.push(
                dir_row(vec![
                    text(format!("{}:", fl!("cookies_container"))).into(),
                    text_input(&fl!("cookies_container_placeholder"), &cookies.container)
                        .on_input(Message::InputCookiesContainer)
                        .into(),
                ])
                .spacing(SPACING)
                .align_y(Alignment::Center)
                .into(),
            );
        }

        let cookies_from_browser = dir_row(cookies_from_browser).spacing(20);

        let mut settings_tab = column![
            row![
                checkbox(self.config.save_window_position)
                    .label(fl!("save_window_position"))
                    .on_toggle(Message::ToggleSaveWindowPosition)
            ],
            dir_row(vec![
                text(format!("{}:", fl!("ytdlp_path"))).into(),
                text_input(
                    &fl!("ytdlp_path_leave_empty"),
                    &self
                        .config
                        .bin_path
                        .clone()
                        .unwrap_or("".into())
                        .to_string_lossy()
                )
                .on_input(Message::SelectYtDlpBitPathTextInput)
                .into(),
                button(text(fl!("browse")))
                    .on_press(Message::SelectYtDlpBinPath)
                    .into(),
            ])
            .spacing(SPACING)
            .align_y(iced::Alignment::Center),
            dir_row(vec![
                text(format!("{}:", fl!("cookies_file"))).into(),
                text_input(
                    "",
                    &self
                        .config
                        .cookies_file
                        .clone()
                        .unwrap_or("".into())
                        .to_string_lossy()
                )
                .on_input(Message::SelectCookiesFileTextInput)
                .into(),
                button(text(fl!("browse")))
                    .on_press(Message::SelectCookiesFile)
                    .into(),
            ])
            .spacing(SPACING)
            .align_y(Alignment::Center),
            cookies_from_browser,
            dir_row(vec![
                text("Concurrent Fragments (-N):").into(),
                pick_list(
                    vec![1u8, 2, 4, 8, 16],
                    Some(self.config.concurrent_fragments),
                    Message::SelectedConcurrentFragments,
                )
                .style(pick_list_style)
                .menu_style(pick_list_menu_style)
                .into(),
                space::horizontal().into(),
                text("Rate Limit:").into(),
                text_input(
                    "e.g. 5M, 500K (unlimited if empty)",
                    &self.config.rate_limit
                )
                .on_input(Message::InputRateLimit)
                .into(),
            ])
            .spacing(SPACING)
            .align_y(Alignment::Center),
            dir_row(vec![
                text("Output Template:").into(),
                text_input("%(title)s.%(ext)s", &self.config.output_template)
                    .on_input(Message::InputOutputTemplate)
                    .into(),
                space::horizontal().into(),
                text("Subtitle Langs:").into(),
                text_input("all, en.*, ja", &self.config.sub_langs)
                    .on_input(Message::InputSubLangs)
                    .into(),
            ])
            .spacing(SPACING)
            .align_y(Alignment::Center),
            dir_row(vec![
                text("Custom yt-dlp Args:").into(),
                text_input("e.g. --geo-bypass --no-mtime", &self.config.custom_args)
                    .on_input(Message::InputCustomArgs)
                    .into(),
            ])
            .spacing(SPACING)
            .align_y(Alignment::Center),
        ]
        .width(Length::Fill)
        .spacing(20)
        .padding(Padding::ZERO.top(20).horizontal(20));

        if is_rtl() {
            settings_tab = settings_tab.align_x(Alignment::End);
        }

        let tabs = if is_rtl() {
            Tabs::new(Message::SelectTab)
                .push(
                    Tab::Settings,
                    iced_aw::TabLabel::Text(fl!("settings")),
                    scrollable(settings_tab),
                )
                .push(Tab::Audio, iced_aw::TabLabel::Text(fl!("audio")), audio_tab)
                .push(Tab::Video, iced_aw::TabLabel::Text(fl!("video")), video_tab)
        } else {
            Tabs::new(Message::SelectTab)
                .push(Tab::Video, iced_aw::TabLabel::Text(fl!("video")), video_tab)
                .push(Tab::Audio, iced_aw::TabLabel::Text(fl!("audio")), audio_tab)
                .push(
                    Tab::Settings,
                    iced_aw::TabLabel::Text(fl!("settings")),
                    settings_tab,
                )
        }
        .set_active_tab(&self.active_tab)
        .height(Length::Shrink)
        .width(Length::FillPortion(1))
        .tab_bar_width(Length::FillPortion(1))
        .tab_bar_style(tab_bar_style);

        let download_link_input = text_editor(&self.download_link_content)
            .placeholder("Download link(s) - One per row...")
            .on_action(Message::EditorAction);

        let url_controls = column![
            button(text("Paste"))
                .on_press(Message::PasteUrl)
                .width(Length::Fixed(90.0)),
            button(text("Clear"))
                .on_press(Message::ClearUrl)
                .width(Length::Fixed(90.0)),
            checkbox(self.is_playlist)
                .label(fl!("playlist"))
                .on_toggle(Message::TogglePlaylist),
        ]
        .spacing(7);

        let mut main_column = column![
            dir_row(vec![
                container(download_link_input)
                    .height(110)
                    .width(Length::Fill)
                    .into(),
                url_controls.into(),
            ])
            .spacing(10)
            .align_y(iced::Alignment::Center),
            tabs,
        ];

        if self.show_console {
            let status_dot = text("●").size(10).color(if self.command.is_running() {
                Color::from_rgb8(0x67, 0xDA, 0x0D)
            } else {
                Color::from_rgb8(0x80, 0x83, 0x8A)
            });

            let status_label = text(if self.command.is_running() {
                "CONSOLE (ACTIVE)"
            } else {
                "CONSOLE (IDLE)"
            })
            .size(11)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .color(Color::from_rgb8(0x9E, 0xA2, 0xAC));

            let clear_button = iced::widget::button(
                text("Clear")
                    .size(11)
                    .color(Color::from_rgb8(0x9E, 0xA2, 0xAC)),
            )
            .on_press(Message::ClearConsole)
            .padding([2, 8])
            .style(|_theme: &iced::Theme, status| {
                let mut s = iced::widget::button::Style::default();
                s.background = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        Color::from_rgba8(0x40, 0x40, 0x40, 0.6),
                    )),
                    _ => Some(iced::Background::Color(Color::TRANSPARENT)),
                };
                s.border = match status {
                    iced::widget::button::Status::Hovered => iced::Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: LIME_HIGHLIGHT,
                    },
                    _ => iced::Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: Color::from_rgba8(0x80, 0x83, 0x8A, 0.3),
                    },
                };
                s.shadow = match status {
                    iced::widget::button::Status::Hovered => iced::Shadow {
                        color: Color::from_rgba8(0xBF, 0xFF, 0x00, 0.50),
                        offset: iced::Vector::new(0.0, 0.0),
                        blur_radius: 6.0,
                    },
                    _ => iced::Shadow::default(),
                };
                s
            });

            let header = row![
                row![status_dot, status_label]
                    .spacing(6)
                    .align_y(Alignment::Center),
                space::horizontal(),
                clear_button,
            ]
            .padding([0, 4])
            .align_y(Alignment::Center);

            let log_text = if self.log_output.is_empty() {
                text("[Console ready. Output and status will appear here...]")
                    .size(12)
                    .font(iced::Font::MONOSPACE)
                    .color(Color::from_rgba8(0x80, 0x83, 0x8A, 0.6))
            } else {
                text(&self.log_output)
                    .size(12)
                    .font(iced::Font::MONOSPACE)
                    .color(Color::from_rgb8(0x67, 0xDA, 0x0D))
            };

            let console_box = container(
                column![
                    header,
                    container(
                        scrollable(log_text)
                            .height(Length::Fixed(125.0))
                            .width(Length::Fill)
                    )
                    .padding(8)
                    .style(|_theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(Color::from_rgba8(
                            0x18, 0x1A, 0x1D, 0.95,
                        ))),
                        border: iced::Border {
                            color: Color::from_rgba8(0x80, 0x83, 0x8A, 0.20),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..iced::widget::container::Style::default()
                    })
                ]
                .spacing(6),
            )
            .padding(10)
            .style(console_container_style);

            main_column = main_column.push(console_box);
        }

        main_column = main_column.push(self.new_version.as_ref().map(|new_version| {
            row![
                column![
                    rich_text![
                        "New version available: ",
                        span(new_version.to_string())
                            .color(Color::from_rgb8(94, 169, 231))
                            .underline(true)
                            .link("https://github.com/BKSalman/ytdlp-gui/releases/latest"),
                    ]
                    .on_link_click(Message::OpenLink)
                ]
                .align_x(iced::Alignment::End),
            ]
        }));

        let content: iced::Element<Message> = main_column
            .width(Length::Fill)
            .align_x(iced::Alignment::Center)
            .spacing(20)
            .padding(Padding::ZERO.top(20).horizontal(20))
            .into();

        #[cfg(feature = "explain")]
        let content = content.explain(Color::from_rgb(0., 1.0, 0.));

        container(content)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::event::listen().map(Message::IcedEvent)
    }

    pub fn log_message(&mut self, msg: &str) {
        if !self.log_output.is_empty() {
            self.log_output.push('\n');
        }
        self.log_output.push_str(msg);

        let msg_trimmed = msg.trim();
        let msg_lower = msg_trimmed.to_lowercase();
        if msg_trimmed.starts_with("[ERROR]")
            || msg_trimmed.starts_with("[WARN]")
            || msg_lower.contains("error")
            || msg_lower.contains("fail")
        {
            log_error_to_file(msg_trimmed);
        }
    }

    pub fn end_download(&mut self, download_message: Option<Result<String, DownloadError>>) {
        self.command.kill();
        self.progress = None;
        match &download_message {
            Some(Ok(msg)) => {
                self.log_message(&format!("[SUCCESS] {}", msg));
                let _ = Notification::new().summary(msg).show();
            }
            Some(Err(e)) => {
                self.log_message(&format!("[ERROR] {}", e));
                let _ = Notification::new().summary(&e.to_string()).show();
            }
            None => {}
        }
        self.download_message = download_message;
        self.log_download();
    }
}

pub fn log_error_to_file(message: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Ok(mut file) = OpenOptions::new()
        .append(true)
        .create(true)
        .open("lime-dlp.log")
    {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{}] {}", timestamp, message);
    }
}
