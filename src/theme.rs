use iced::overlay::menu;
use iced::widget::{self, pick_list};
use iced::{Background, Border, Color, Element, Shadow, Theme, Vector};
use iced_aw::tab_bar;

use crate::YtGUI;

const BACKGROUND: Color = Color::from_rgb8(0x3B, 0x3B, 0x3B);
const GLASS_PANEL: Color = Color::from_rgba8(0x30, 0x30, 0x30, 0.85);
const GLASS_PANEL_HOVER: Color = Color::from_rgba8(0x42, 0x45, 0x4C, 0.90);
const LIME_ACCENT: Color = Color::from_rgb8(0x67, 0xDA, 0x0D);
const LIME_HOVER: Color = Color::from_rgb8(0x75, 0xEE, 0x15);
const LIME_DARK: Color = Color::from_rgb8(0x52, 0xAE, 0x0A);
pub const LIME_HIGHLIGHT: Color = Color::from_rgb8(0xBF, 0xFF, 0x00);
const URBAN_ECLIPSE: Color = Color::from_rgb8(0x80, 0x83, 0x8A);
const SUBTLE_BORDER: Color = Color::from_rgba8(0x80, 0x83, 0x8A, 0.35);
const ACTIVE_BORDER: Color = Color::from_rgba8(0x80, 0x83, 0x8A, 0.70);
const DANGER_ACCENT: Color = Color::from_rgb8(0xE0, 0x48, 0x48);
const DANGER_HOVER: Color = Color::from_rgb8(0xF0, 0x58, 0x58);
const DANGER_DARK: Color = Color::from_rgb8(0xB8, 0x30, 0x30);

pub fn ytdlp_gui_theme(_state: &YtGUI) -> Theme {
    Theme::custom(
        String::from("urban_eclipse_3b_lime"),
        iced::theme::Palette {
            background: BACKGROUND,
            text: URBAN_ECLIPSE,
            primary: LIME_ACCENT,
            success: LIME_ACCENT,
            danger: DANGER_ACCENT,
            warning: Color::from_rgb(0.95, 0.75, 0.30),
        },
    )
}

pub fn tab_bar_style(theme: &Theme, status: tab_bar::Status) -> tab_bar::Style {
    let mut base = tab_bar::tab_bar::primary(theme, status);
    base.tab_label_background = Background::Color(GLASS_PANEL);
    base.background = Some(Background::Color(BACKGROUND));
    base
}

pub fn pick_list_style(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let palette = theme.extended_palette();

    let active = pick_list::Style {
        text_color: URBAN_ECLIPSE,
        background: GLASS_PANEL.into(),
        placeholder_color: palette.background.strong.color,
        handle_color: LIME_ACCENT,
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: SUBTLE_BORDER,
        },
    };

    match status {
        pick_list::Status::Active => active,
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => pick_list::Style {
            border: Border {
                color: ACTIVE_BORDER,
                ..active.border
            },
            ..active
        },
    }
}

pub fn pick_list_menu_style(_theme: &Theme) -> menu::Style {
    menu::Style {
        background: Color::from_rgba8(0x30, 0x30, 0x30, 0.95).into(),
        border: Border {
            width: 1.0,
            radius: 10.0.into(),
            color: SUBTLE_BORDER,
        },
        text_color: URBAN_ECLIPSE,
        selected_text_color: Color::BLACK,
        selected_background: LIME_ACCENT.into(),
        shadow: Shadow::default(),
    }
}

pub fn console_container_style(_theme: &Theme) -> widget::container::Style {
    widget::container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x22, 0x24, 0x29, 0.90))),
        border: Border {
            color: Color::from_rgba8(0x67, 0xDA, 0x0D, 0.35),
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.35),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..widget::container::Style::default()
    }
}

/// Standard secondary frosted glass button (e.g., Browse, Collapsible headers)
pub fn button<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> widget::Button<'a, Message> {
    widget::button(content).style(|_theme: &Theme, status| match status {
        widget::button::Status::Active => widget::button::Style {
            background: Some(Background::Color(GLASS_PANEL)),
            text_color: LIME_HIGHLIGHT,
            border: Border {
                color: LIME_HIGHLIGHT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0xBF, 0xFF, 0x00, 0.20),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            },
            ..widget::button::Style::default()
        },
        widget::button::Status::Hovered => widget::button::Style {
            background: Some(Background::Color(GLASS_PANEL_HOVER)),
            text_color: LIME_HIGHLIGHT,
            border: Border {
                color: LIME_HIGHLIGHT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0xBF, 0xFF, 0x00, 0.60),
                offset: Vector::new(0.0, 0.0),
                blur_radius: 10.0,
            },
            ..widget::button::Style::default()
        },
        widget::button::Status::Pressed => widget::button::Style {
            background: Some(Background::Color(Color::from_rgba8(0x24, 0x26, 0x2A, 0.95))),
            text_color: LIME_HIGHLIGHT,
            border: Border {
                color: LIME_HIGHLIGHT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
        widget::button::Status::Disabled => widget::button::Style {
            background: Some(Background::Color(Color::from_rgba8(0x28, 0x28, 0x28, 0.4))),
            text_color: Color::from_rgba8(0x80, 0x83, 0x8A, 0.4),
            border: Border {
                color: Color::TRANSPARENT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
    })
}

/// Primary Call-to-Action button with vibrant lime accent and glow (e.g., Download)
pub fn primary_button<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> widget::Button<'a, Message> {
    widget::button(content).style(|_theme: &Theme, status| match status {
        widget::button::Status::Active => widget::button::Style {
            background: Some(Background::Color(LIME_ACCENT)),
            text_color: Color::from_rgb8(0x10, 0x18, 0x06),
            border: Border {
                color: LIME_ACCENT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0x67, 0xDA, 0x0D, 0.30),
                offset: Vector::new(0.0, 2.0),
                blur_radius: 6.0,
            },
            ..widget::button::Style::default()
        },
        widget::button::Status::Hovered => widget::button::Style {
            background: Some(Background::Color(LIME_HOVER)),
            text_color: Color::from_rgb8(0x05, 0x0B, 0x02),
            border: Border {
                color: LIME_HIGHLIGHT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0xBF, 0xFF, 0x00, 0.60),
                offset: Vector::new(0.0, 0.0),
                blur_radius: 10.0,
            },
            ..widget::button::Style::default()
        },
        widget::button::Status::Pressed => widget::button::Style {
            background: Some(Background::Color(LIME_DARK)),
            text_color: Color::from_rgb8(0x05, 0x0B, 0x02),
            border: Border {
                color: LIME_DARK,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
        widget::button::Status::Disabled => widget::button::Style {
            background: Some(Background::Color(Color::from_rgba8(0x40, 0x45, 0x3E, 0.4))),
            text_color: Color::from_rgba8(0x80, 0x88, 0x7E, 0.5),
            border: Border {
                color: Color::TRANSPARENT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
    })
}

/// Danger/Cancel button (e.g., Stop Download)
pub fn danger_button<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> widget::Button<'a, Message> {
    widget::button(content).style(|_theme: &Theme, status| match status {
        widget::button::Status::Active => widget::button::Style {
            background: Some(Background::Color(Color::from_rgba8(0x40, 0x20, 0x20, 0.40))),
            text_color: DANGER_ACCENT,
            border: Border {
                color: Color::from_rgba8(0xE0, 0x48, 0x48, 0.40),
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
        widget::button::Status::Hovered => widget::button::Style {
            background: Some(Background::Color(DANGER_HOVER)),
            text_color: Color::WHITE,
            border: Border {
                color: LIME_HIGHLIGHT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0xBF, 0xFF, 0x00, 0.60),
                offset: Vector::new(0.0, 0.0),
                blur_radius: 10.0,
            },
            ..widget::button::Style::default()
        },
        widget::button::Status::Pressed => widget::button::Style {
            background: Some(Background::Color(DANGER_DARK)),
            text_color: Color::WHITE,
            border: Border {
                color: DANGER_DARK,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
        widget::button::Status::Disabled => widget::button::Style {
            background: Some(Background::Color(Color::from_rgba8(0x28, 0x28, 0x28, 0.4))),
            text_color: Color::from_rgba8(0x80, 0x83, 0x8A, 0.4),
            border: Border {
                color: Color::TRANSPARENT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..widget::button::Style::default()
        },
    })
}
