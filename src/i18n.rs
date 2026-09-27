/*  */
//! Provides localization support for this crate.

use std::sync::LazyLock;

use i18n_embed::{
    DefaultLocalizer, LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
    unic_langid::LanguageIdentifier,
};

use rust_embed::RustEmbed;

/// Applies the requested language(s) to requested translations from the `fl!()` macro.
pub fn init(requested_languages: &[LanguageIdentifier]) {
    if let Err(why) = localizer().select(requested_languages) {
        eprintln!("error while loading fluent localizations: {why}");
    }
}

// Get the `Localizer` to be used for localizing this library.
#[must_use]
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error while loading fallback language");

    loader
});

/// Request a localized string by ID from the i18n/ directory.
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        $crate::i18n::LANGUAGE_LOADER.get($message_id)
    }};

    ($message_id:literal, $attribute:literal, $($key:ident = $value:expr),+ $(,)?) => {{
        let mut args = ::std::collections::HashMap::new();
        $(args.insert(stringify!($key), $value.into());)+
        $crate::i18n::LANGUAGE_LOADER.get_attr_args_concrete($message_id, $attribute, args)
    }};

    ($message_id:literal, $attribute:literal, $args:expr) => {{
        $crate::i18n::LANGUAGE_LOADER.get_attr_args($message_id, $attribute, $args)
    }};

    ($message_id:literal, $attribute:literal) => {{
        $crate::i18n::LANGUAGE_LOADER.get_attr($message_id, $attribute)
    }};

    ($message_id:literal, $($key:ident = $value:expr),+ $(,)?) => {{
        let mut args = ::std::collections::HashMap::new();
        $(args.insert(stringify!($key), $value.into());)+
        $crate::i18n::LANGUAGE_LOADER.get_args_concrete($message_id, args)
    }};

    ($message_id:literal, $args:expr) => {{
        $crate::i18n::LANGUAGE_LOADER.get_args($message_id, $args)
    }};
}

pub fn is_rtl() -> bool {
    matches!(
        localizer()
            .language_loader()
            .current_language()
            .language
            .as_str(),
        "ar"
    )
}
