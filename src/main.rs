//! Runs Grove directly on the machine's native backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

/// The app's id: it names the directory Grove keeps its data in
/// (`<data_dir>/dev.grove.app`), and it is the window's AppUserModelID.
const APP_ID: &str = "dev.grove.app";

/// The root the window runs, with what `nsh.json` declares: the theme's
/// tokens and the id. `nsh run`'s generated host does the same; this binary
/// is the one `cargo run` builds, and a root without them draws unstyled and
/// writes into the shared `default` directory.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn root() -> platform::Root {
    // The reference IDE draws its own title bar: the window's is hidden and
    // the app's 40pt bar takes its place, with the system's caption buttons
    // drawn over its right end.
    let mut title_bar = nsh_core::desktop::TitleBarOptions::default();
    title_bar.style = nsh_core::desktop::TitleBarStyle::Hidden;
    title_bar.height = Some(nsh_core::geometry::px(40.0));

    let mut attributes = nsh_core::desktop::WindowAttributes::default();
    attributes.title_bar = Some(title_bar);

    let mut main_window = nsh_core::desktop::MainWindowOptions::default();
    main_window.default_size = Some(nsh_core::geometry::size(
        nsh_core::geometry::px(1600.0),
        nsh_core::geometry::px(980.0),
    ));
    main_window.attributes = attributes;

    platform::Root::of::<grove::App>()
        .tokens(
            nsh_base::Theme::from_json(include_str!("../theme.json"))
                .expect("theme.json")
                .into_tokens(),
        )
        .app_id(APP_ID)
        .main_window(main_window)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    env_logger::init();
    platform::run_app(
        platform::AppSource::Rust(root()),
        platform::AppOptions {
            title: "Grove".to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: APP_ID.to_owned(),
            size: (1360, 860),
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; use the TSX entry with nsh run for iOS and Android."
);
