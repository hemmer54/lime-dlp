#![windows_subsystem = "windows"]

use gpui_kit::component::{Root, Theme, ThemeMode, TitleBar};
use gpui_kit::*;
use lime_dlp::{
    Config, Flags, Message, WindowSize, YtGUI, git_hash, logging, update::check_for_update,
};
use std::borrow::Cow;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut url = None;
    if let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("Usage: lime-dlp <OPTIONS>\n");
            println!("Options:");
            println!("-h, --help      Print help");
            println!("-V, --version   Print version");
            println!(
                "-u, --url       Starts the application with the provided URL as the download URL"
            );
            std::process::exit(0);
        } else if arg == "--version" || arg == "-V" {
            let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
            let git_hash = git_hash!();
            println!("version: {version}");
            println!("git hash: {git_hash}");
            std::process::exit(0);
        } else if arg == "--url" || arg == "-u" {
            url = args.next();
        } else {
            println!("Invalid option/argument");
            std::process::exit(1);
        }
    }

    logging();

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    lime_dlp::i18n::init(&requested_languages);

    let config_dir = dirs::config_dir()
        .expect("config directory")
        .join("lime-dlp/");
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let mut config = match std::fs::read_to_string(config_dir.join("config.toml")) {
        Ok(config_str) => toml::from_str::<Config>(&config_str).unwrap_or_else(|error| {
            tracing::error!("failed to parse config: {error:#?}");
            let config = Config::default();
            tracing::warn!("falling back to default configs: {config:#?}");
            config
        }),
        Err(error) => match error.kind() {
            std::io::ErrorKind::NotFound => {
                let config = Config::default();
                tracing::warn!(
                    "Config file not found, falling back to default configs: {config:#?}"
                );
                config
            }
            _ => panic!("{error}"),
        },
    };
    config.migrate_legacy_bin_path();
    let flags = Flags { url, config };

    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let update_sender = sender.clone();
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(|runtime| runtime.block_on(check_for_update()))
            .unwrap_or_else(|error| {
                Err(lime_dlp::update::Error::UpdateFetchFailed(
                    error.to_string(),
                ))
            });
        let _ = update_sender.send(Message::UpdateCheck(result));
    });

    let runtime_update_sender = sender.clone();
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime
                    .block_on(lime_dlp::runtime_update::check_for_update())
                    .map_err(|error| error.to_string())
            });
        let _ = runtime_update_sender.send(Message::RuntimeUpdateCheck(result));
    });

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../assets/fonts/FSEX302.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/jh_fallout-webfont.ttf").as_slice()),
            ])
            .expect("failed to load embedded fonts");
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::global_mut(cx).font_family = "Fixedsys Excelsior".into();

        let window_size = flags.config.window_size.unwrap_or(WindowSize {
            width: 920.0,
            height: 540.0,
        });
        let size = gpui::size(gpui::px(window_size.width), gpui::px(window_size.height));
        let bounds = if flags.config.save_window_position {
            flags
                .config
                .window_position
                .as_ref()
                .map(|position| {
                    WindowBounds::Windowed(Bounds::new(
                        gpui::point(gpui::px(position.x), gpui::px(position.y)),
                        size,
                    ))
                })
                .unwrap_or_else(|| WindowBounds::centered(size, cx))
        } else {
            WindowBounds::centered(size, cx)
        };

        let mut options = TitleBar::window_options();
        options.window_bounds = Some(bounds);
        options.window_background = WindowBackgroundAppearance::Transparent;
        options.is_resizable = true;
        options.is_movable = true;
        options.window_min_size = Some(gpui::size(gpui::px(720.0), gpui::px(480.0)));

        let flags = flags.clone();
        let sender = sender.clone();
        cx.spawn(async move |cx| {
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| YtGUI::new(flags, sender, receiver, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open Lime DLP window");
        })
        .detach();
    });
}
