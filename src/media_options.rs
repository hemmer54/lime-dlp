use std::path::PathBuf;

use iced::widget::{pick_list, text};
use serde::{Deserialize, Serialize};

use crate::{
    Message,
    app::{FONT_SIZE, SPACING},
    fl,
    i18n::dir_row,
    theme::{pick_list_menu_style, pick_list_style},
};

#[derive(Deserialize, Serialize, Debug, Default, Copy, Clone)]
pub struct Options {
    pub video_resolution: VideoResolution,
    pub video_format: VideoFormat,
    pub audio_quality: AudioQuality,
    pub audio_format: AudioFormat,
}

#[derive(Deserialize, Serialize, Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum VideoResolution {
    #[default]
    Source,
    FourK,
    TwoK,
    FullHD,
    Hd,
    Sd,
}

impl core::fmt::Display for VideoResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoResolution::Source => write!(f, "Source (Best)"),
            VideoResolution::FourK => write!(f, "4K"),
            VideoResolution::TwoK => write!(f, "1440p"),
            VideoResolution::FullHD => write!(f, "1080p"),
            VideoResolution::Hd => write!(f, "720p"),
            VideoResolution::Sd => write!(f, "480p"),
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum VideoFormat {
    #[default]
    Mp4,
    Mkv,
    Webm,
    Avi,
    Mov,
    Flv,
}

impl core::fmt::Display for VideoFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoFormat::Mp4 => write!(f, "MP4"),
            VideoFormat::Mkv => write!(f, "MKV"),
            VideoFormat::Webm => write!(f, "WEBM"),
            VideoFormat::Avi => write!(f, "AVI"),
            VideoFormat::Mov => write!(f, "MOV"),
            VideoFormat::Flv => write!(f, "FLV"),
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum AudioQuality {
    Best,
    Ultra,
    VeryHigh,
    High,
    #[default]
    Good,
    Medium,
    Low,
}

impl core::fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioQuality::Best => f.write_str(&fl!("quality_best")),
            AudioQuality::Ultra => write!(f, "Ultra (320kbps)"),
            AudioQuality::VeryHigh => write!(f, "Very High (256kbps)"),
            AudioQuality::High => write!(f, "High (192kbps)"),
            AudioQuality::Good => f.write_str(&fl!("quality_good")),
            AudioQuality::Medium => f.write_str(&fl!("quality_medium")),
            AudioQuality::Low => f.write_str(&fl!("quality_low")),
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum AudioFormat {
    #[default]
    Mp3,
    Original,
    M4a,
    Opus,
    Flac,
    Wav,
    Vorbis,
}

impl core::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioFormat::Mp3 => write!(f, "MP3"),
            AudioFormat::Original => write!(f, "Original (Lossless Copy)"),
            AudioFormat::M4a => write!(f, "M4A"),
            AudioFormat::Opus => write!(f, "OPUS"),
            AudioFormat::Flac => write!(f, "FLAC"),
            AudioFormat::Wav => write!(f, "WAV"),
            AudioFormat::Vorbis => write!(f, "VORBIS"),
        }
    }
}

impl VideoResolution {
    pub fn options(&self) -> &str {
        match self {
            VideoResolution::Source => "",
            VideoResolution::FourK => "res:2160",
            VideoResolution::TwoK => "res:1440",
            VideoResolution::FullHD => "res:1080",
            VideoResolution::Hd => "res:720",
            VideoResolution::Sd => "res:480",
        }
    }
}

impl VideoFormat {
    pub fn options(&self) -> &str {
        match self {
            VideoFormat::Mp4 => "mp4",
            VideoFormat::Mkv => "mkv",
            VideoFormat::Webm => "webm",
            VideoFormat::Avi => "avi",
            VideoFormat::Mov => "mov",
            VideoFormat::Flv => "flv",
        }
    }
}

impl AudioFormat {
    pub fn options(&self) -> &str {
        match self {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Original => "best",
            AudioFormat::M4a => "m4a",
            AudioFormat::Opus => "opus",
            AudioFormat::Flac => "flac",
            AudioFormat::Wav => "wav",
            AudioFormat::Vorbis => "vorbis",
        }
    }
}

impl AudioQuality {
    pub fn options(&self) -> &str {
        match self {
            AudioQuality::Best => "0",
            AudioQuality::Ultra => "320K",
            AudioQuality::VeryHigh => "256K",
            AudioQuality::High => "192K",
            AudioQuality::Good => "2",
            AudioQuality::Medium => "4",
            AudioQuality::Low => "6",
        }
    }
}

impl Options {
    pub fn video_resolutions(resolution: VideoResolution) -> iced::widget::Row<'static, Message> {
        dir_row(vec![
            text(format!("{}: ", fl!("resolution")))
                .size(FONT_SIZE)
                .into(),
            pick_list(
                vec![
                    VideoResolution::Source,
                    VideoResolution::FourK,
                    VideoResolution::TwoK,
                    VideoResolution::FullHD,
                    VideoResolution::Hd,
                    VideoResolution::Sd,
                ],
                Some(resolution),
                Message::SelectedResolution,
            )
            .text_size(FONT_SIZE)
            .menu_style(pick_list_menu_style)
            .style(pick_list_style)
            .into(),
        ])
        .spacing(SPACING)
        .width(iced::Length::Shrink)
        .align_y(iced::Alignment::Center)
    }

    pub fn video_formats(format: VideoFormat) -> iced::widget::Row<'static, Message> {
        dir_row(vec![
            text(format!("{}: ", fl!("preferred_format")))
                .size(FONT_SIZE)
                .into(),
            pick_list(
                vec![
                    VideoFormat::Mp4,
                    VideoFormat::Mkv,
                    VideoFormat::Webm,
                    VideoFormat::Avi,
                    VideoFormat::Mov,
                    VideoFormat::Flv,
                ],
                Some(format),
                Message::SelectedVideoFormat,
            )
            .text_size(FONT_SIZE)
            .menu_style(pick_list_menu_style)
            .style(pick_list_style)
            .into(),
        ])
        .width(iced::Length::Shrink)
        .spacing(SPACING)
        .align_y(iced::Alignment::Center)
    }

    pub fn audio_formats(format: AudioFormat) -> iced::widget::Row<'static, Message> {
        dir_row(vec![
            text(format!("{}: ", fl!("preferred_format")))
                .size(FONT_SIZE)
                .into(),
            pick_list(
                vec![
                    AudioFormat::Mp3,
                    AudioFormat::Original,
                    AudioFormat::M4a,
                    AudioFormat::Opus,
                    AudioFormat::Flac,
                    AudioFormat::Wav,
                    AudioFormat::Vorbis,
                ],
                Some(format),
                Message::SelectedAudioFormat,
            )
            .text_size(FONT_SIZE)
            .style(pick_list_style)
            .menu_style(pick_list_menu_style)
            .into(),
        ])
        .width(iced::Length::Shrink)
        .spacing(SPACING)
        .align_y(iced::Alignment::Center)
    }

    pub fn audio_qualities(quality: AudioQuality) -> iced::widget::Row<'static, Message> {
        dir_row(vec![
            text(format!("{}: ", fl!("quality"))).size(FONT_SIZE).into(),
            pick_list(
                vec![
                    AudioQuality::Best,
                    AudioQuality::Ultra,
                    AudioQuality::VeryHigh,
                    AudioQuality::High,
                    AudioQuality::Good,
                    AudioQuality::Medium,
                    AudioQuality::Low,
                ],
                Some(quality),
                Message::SelectedAudioQuality,
            )
            .text_size(FONT_SIZE)
            .style(pick_list_style)
            .menu_style(pick_list_menu_style)
            .into(),
        ])
        .width(iced::Length::Shrink)
        .spacing(SPACING)
        .align_y(iced::Alignment::Center)
    }
}

pub fn playlist_options(
    is_playlist: bool,
    download_folder: PathBuf,
    output_template: Option<&str>,
) -> Vec<String> {
    let download_dir = download_folder.to_string_lossy().to_string();
    let template = output_template
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("%(title)s.%(ext)s");

    if is_playlist {
        vec![
            String::from("--yes-playlist"),
            String::from("-P"),
            download_dir,
            String::from("-o"),
            format!("%(playlist)s/{template}"),
        ]
    } else {
        vec![
            String::from("--break-on-reject"),
            String::from("--match-filter"),
            String::from("!playlist"),
            String::from("--no-playlist"),
            String::from("-P"),
            download_dir,
            String::from("-o"),
            template.to_string(),
        ]
    }
}
