use std::borrow::Cow;

use tauri::{
    http::{header, Request, Response, StatusCode},
    webview::{Url, WebviewWindowBuilder},
    window::Color,
    AppHandle, Manager, Theme, UriSchemeContext, WebviewUrl, WebviewWindow, WindowEvent, Wry,
};

use crate::autostart::is_autostart_enabled;
use crate::updater::check_for_updates;
use crate::url_guard::{is_local_scheme_origin, local_scheme_url};
use crate::webview::open_external;

pub(crate) const ABOUT_WINDOW_LABEL: &str = "about";
pub(crate) const ABOUT_SCHEME: &str = "deni-about";

const ABOUT_TEMPLATE: &str = include_str!("about.html");
const APP_ICON_SVG: &[u8] = include_bytes!("../app-icon.svg");
const ABOUT_CSP: &str =
    "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src 'self'";

const GIT_SHA: &str = env!("DENI_GIT_SHA");
const GIT_DATE: &str = env!("DENI_GIT_DATE");
const RUSTC_VERSION: &str = env!("DENI_RUSTC_VERSION");
const BUILD_TARGET: &str = env!("DENI_BUILD_TARGET");

#[cfg(target_os = "windows")]
const WEBVIEW_NAME: &str = "WebView2";
#[cfg(target_os = "macos")]
const WEBVIEW_NAME: &str = "WKWebView";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const WEBVIEW_NAME: &str = "WebKitGTK";

struct AboutInfo {
    name: String,
    version: String,
    identifier: String,
    channel: &'static str,
    profile: &'static str,
    webview_version: String,
    os: String,
    autostart: bool,
    data_dir: String,
}

impl AboutInfo {
    fn collect(app: &AppHandle) -> Self {
        let package = app.package_info();
        let identifier = app.config().identifier.clone();
        let channel = if identifier.ends_with(".canary") {
            "Canary"
        } else {
            "Stable"
        };

        Self {
            name: package.name.clone(),
            version: package.version.to_string(),
            identifier,
            channel,
            profile: if cfg!(debug_assertions) {
                "Debug"
            } else {
                "Release"
            },
            webview_version: tauri::webview_version().unwrap_or_else(|_| "unknown".to_string()),
            os: os_description(),
            autostart: is_autostart_enabled(app),
            data_dir: app
                .path()
                .app_data_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        }
    }

    fn commit(&self) -> String {
        if GIT_DATE == "unknown" {
            GIT_SHA.to_string()
        } else {
            format!("{GIT_SHA} ({GIT_DATE})")
        }
    }

    fn rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Commit", self.commit()),
            ("Tauri", tauri::VERSION.to_string()),
            (WEBVIEW_NAME, self.webview_version.clone()),
            ("OS", self.os.clone()),
            ("Architecture", std::env::consts::ARCH.to_string()),
            (
                "Launch at Login",
                if self.autostart { "On" } else { "Off" }.to_string(),
            ),
        ]
    }

    fn diagnostics(&self) -> String {
        let mut lines = vec![
            format!("{} {} ({})", self.name, self.version, self.channel),
            format!("Identifier: {}", self.identifier),
            format!("Build: {} {}", self.profile, BUILD_TARGET),
        ];
        lines.extend(
            self.rows()
                .into_iter()
                .map(|(label, value)| format!("{label}: {value}")),
        );
        lines.push(format!("Rust: {RUSTC_VERSION}"));
        lines.push(format!("Data: {}", self.data_dir));
        lines.join("\n")
    }

    fn render(&self) -> String {
        let rows = self
            .rows()
            .into_iter()
            .map(|(label, value)| {
                let value = escape_html(&value);
                format!(
                    r#"<div class="row"><dt>{}</dt><dd title="{value}">{value}</dd></div>"#,
                    escape_html(label)
                )
            })
            .collect::<String>();
        let profile_pill = if cfg!(debug_assertions) {
            r#"<span class="pill subtle">Debug</span>"#
        } else {
            ""
        };
        let copyright = match GIT_DATE.get(..4) {
            Some(year) if year.bytes().all(|byte| byte.is_ascii_digit()) => {
                format!("&copy; {year} Deni AI")
            }
            _ => "&copy; Deni AI".to_string(),
        };
        let release_notes_url = format!(
            "https://github.com/deniaiapp/desktop/releases/tag/v{}",
            self.version
        );

        ABOUT_TEMPLATE
            .replace("{{name}}", &escape_html(&self.name))
            .replace("{{version}}", &escape_html(&self.version))
            .replace("{{channel}}", self.channel)
            .replace("{{profile_pill}}", profile_pill)
            .replace("{{rows}}", &rows)
            .replace("{{release_notes_url}}", &escape_html(&release_notes_url))
            .replace("{{copyright}}", &copyright)
            .replace("{{diagnostics}}", &escape_html(&self.diagnostics()))
    }
}

fn os_description() -> String {
    let info = os_info::get();
    let name = info
        .edition()
        .map(str::to_string)
        .unwrap_or_else(|| info.os_type().to_string());
    format!("{name} {}", info.version())
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn about_url() -> Url {
    local_scheme_url(ABOUT_SCHEME, "/")
}

fn is_about_origin(url: &Url) -> bool {
    is_local_scheme_origin(url, ABOUT_SCHEME)
}

fn text_response(status: StatusCode, body: &'static str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Cow::Borrowed(body.as_bytes()))
        .expect("static response must build")
}

pub(crate) fn handle_about_protocol(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    // Only the About window may read this page; it carries local diagnostics.
    if ctx.webview_label() != ABOUT_WINDOW_LABEL {
        return text_response(StatusCode::FORBIDDEN, "Forbidden");
    }

    match request.uri().path() {
        "/" => {}
        "/icon.svg" => {
            return Response::builder()
                .header(header::CONTENT_TYPE, "image/svg+xml")
                .body(Cow::Borrowed(APP_ICON_SVG))
                .expect("icon response must build");
        }
        _ => return text_response(StatusCode::NOT_FOUND, "Not Found"),
    }

    let html = AboutInfo::collect(ctx.app_handle()).render();
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, ABOUT_CSP)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(html.into_bytes()))
        .expect("about response must build")
}

fn run_about_action(app: &AppHandle, action: &str) {
    match action {
        "check-updates" => check_for_updates(app.clone(), true),
        "close" => {
            if let Some(window) = app.get_webview_window(ABOUT_WINDOW_LABEL) {
                let _ = window.close();
            }
        }
        _ => {}
    }
}

fn handle_about_navigation(app: &AppHandle, url: &Url) -> bool {
    if is_about_origin(url) {
        if let Some(action) = url.path().strip_prefix("/action/") {
            run_about_action(app, action);
            return false;
        }
        return url.path() == "/";
    }

    if url.scheme() == "https" {
        open_external(app, url);
    }
    false
}

/// Window background behind the page, matching `--bg` in about.html so the
/// window does not flash the other theme while the page loads.
fn about_background(theme: Theme) -> Color {
    match theme {
        Theme::Light => Color(255, 255, 255, 255),
        _ => Color(10, 10, 10, 255),
    }
}

fn sync_about_background(window: &WebviewWindow, theme: Theme) {
    let _ = window.set_background_color(Some(about_background(theme)));
}

pub(crate) fn open_about_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ABOUT_WINDOW_LABEL) {
        // Reload so values such as Launch at Login reflect the current state.
        let _ = window.navigate(about_url());
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let navigation_handle = app.clone();
    let title = format!("About {}", app.package_info().name);
    let result = WebviewWindowBuilder::new(
        app,
        ABOUT_WINDOW_LABEL,
        WebviewUrl::CustomProtocol(about_url()),
    )
    .title(title)
    .inner_size(400.0, 580.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .center()
    // Follow the system light/dark setting; the page switches with it.
    .theme(None)
    .visible(false)
    .on_navigation(move |url| handle_about_navigation(&navigation_handle, url))
    .build();

    let window = match result {
        Ok(window) => window,
        Err(error) => {
            eprintln!("failed to open About window: {}", error);
            return;
        }
    };

    // The app menu is attached to every window on Windows and Linux; the
    // About window does not need it. On macOS the menu is app-wide.
    #[cfg(not(target_os = "macos"))]
    let _ = window.remove_menu();

    sync_about_background(&window, window.theme().unwrap_or(Theme::Dark));
    let theme_window = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::ThemeChanged(theme) = event {
            sync_about_background(&theme_window, *theme);
        }
    });
    let _ = window.show();
    let _ = window.set_focus();
}
