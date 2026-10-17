use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use serde_json::json;
use tauri::{
    http::{header, Request, Response, StatusCode},
    webview::{NewWindowResponse, Url, WebviewBuilder},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Rect, UriSchemeContext, Webview,
    WebviewUrl, Window, Wry,
};

use crate::menu::{app_menu_json, app_menu_sections, run_menu_command};
use crate::menu_popup::{active_menu, close_menu, is_menu_open, MENU_POPUP_LABEL};
use crate::state::app_title;
use crate::url_guard::{is_local_scheme_origin, local_scheme_url};
use crate::webview::{focus_content, main_window, CONTENT_WEBVIEW_LABEL};

pub(crate) const TITLEBAR_WEBVIEW_LABEL: &str = "titlebar";
pub(crate) const CHROME_SCHEME: &str = "deni-chrome";

/// Height of the custom title bar in logical pixels. Matches `#bar` in titlebar.html.
pub(crate) const TITLEBAR_HEIGHT: f64 = if cfg!(target_os = "macos") {
    38.0
} else {
    40.0
};

const TITLEBAR_TEMPLATE: &str = include_str!("titlebar.html");
const MENU_TEMPLATE: &str = include_str!("menu.html");
const APP_ICON_SVG: &[u8] = include_bytes!("../app-icon.svg");
const CHROME_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; \
    script-src 'unsafe-inline'; img-src 'self'; connect-src ipc: http://ipc.localhost";

#[cfg(target_os = "windows")]
const PLATFORM: &str = "windows";
#[cfg(target_os = "macos")]
const PLATFORM: &str = "macos";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PLATFORM: &str = "linux";

/// Width of each window control button in logical pixels. Matches `.control`
/// in titlebar.html.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const CAPTION_BUTTON_WIDTH: f64 = 46.0;

/// What the title bar shows about the chat page and its window controls.
pub(crate) struct TitlebarState {
    loading: AtomicBool,
    page_title: Mutex<String>,
    // The Snap Layouts overlay covers the maximize button, so the page never
    // sees the mouse there; the overlay reports hover and press instead.
    maximize_hover: AtomicBool,
    maximize_pressed: AtomicBool,
}

impl TitlebarState {
    pub(crate) fn new() -> Self {
        Self {
            loading: AtomicBool::new(true),
            page_title: Mutex::new(String::new()),
            maximize_hover: AtomicBool::new(false),
            maximize_pressed: AtomicBool::new(false),
        }
    }
}

pub(crate) fn chrome_url(path: &str) -> Url {
    local_scheme_url(CHROME_SCHEME, path)
}

/// Keeps the title bar and menu webviews on their own pages.
pub(crate) fn handle_chrome_navigation(url: &Url) -> bool {
    is_local_scheme_origin(url, CHROME_SCHEME)
}

/// Serializes `value` for embedding inside an inline `<script>` element.
fn script_json(value: &serde_json::Value) -> String {
    value.to_string().replace('<', "\\u003c")
}

fn html_response(html: String) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, CHROME_CSP)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(html.into_bytes()))
        .expect("chrome response must build")
}

fn text_response(status: StatusCode, body: &'static str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Cow::Borrowed(body.as_bytes()))
        .expect("static response must build")
}

fn render_titlebar(app: &AppHandle) -> String {
    let sections = app_menu_sections(app)
        .iter()
        .map(|section| json!({ "id": section.id, "label": section.label }))
        .collect::<Vec<_>>();
    let config = json!({
        "platform": PLATFORM,
        "appName": app_title(app),
        "canary": app.config().identifier.ends_with(".canary"),
        "menus": sections,
    });
    TITLEBAR_TEMPLATE
        .replace("{{platform}}", PLATFORM)
        .replace("{{config}}", &script_json(&config))
}

fn render_menu(app: &AppHandle) -> String {
    MENU_TEMPLATE.replace("{{menus}}", &script_json(&app_menu_json(app)))
}

/// Serves the title bar and menu popup pages to their own webviews only.
pub(crate) fn handle_chrome_protocol(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let label = ctx.webview_label();
    let app = ctx.app_handle();

    match (label, request.uri().path()) {
        (TITLEBAR_WEBVIEW_LABEL, "/") => html_response(render_titlebar(app)),
        (MENU_POPUP_LABEL, "/") => html_response(render_menu(app)),
        (TITLEBAR_WEBVIEW_LABEL, "/icon.svg") => Response::builder()
            .header(header::CONTENT_TYPE, "image/svg+xml")
            .body(Cow::Borrowed(APP_ICON_SVG))
            .expect("icon response must build"),
        (TITLEBAR_WEBVIEW_LABEL | MENU_POPUP_LABEL, _) => {
            text_response(StatusCode::NOT_FOUND, "Not Found")
        }
        _ => text_response(StatusCode::FORBIDDEN, "Forbidden"),
    }
}

fn titlebar_webview(app: &AppHandle) -> Option<Webview> {
    app.get_webview(TITLEBAR_WEBVIEW_LABEL)
}

/// Adds the title bar webview to the top of the main window.
pub(crate) fn add_titlebar(window: &Window) -> tauri::Result<Webview> {
    let builder = WebviewBuilder::new(
        TITLEBAR_WEBVIEW_LABEL,
        WebviewUrl::CustomProtocol(chrome_url("/")),
    )
    .background_color(tauri::window::Color(10, 10, 10, 255))
    .on_navigation(handle_chrome_navigation)
    .on_new_window(|_, _| NewWindowResponse::Deny);

    let (titlebar_bounds, _) = webview_bounds(window);
    window.add_child(builder, titlebar_bounds.position, titlebar_bounds.size)
}

/// Splits the window's client area into the title bar strip and the content
/// area below it.
fn webview_bounds(window: &Window) -> (Rect, Rect) {
    let size = window.inner_size().unwrap_or_default();
    let scale = window.scale_factor().unwrap_or(1.0);
    let bar = ((TITLEBAR_HEIGHT * scale).round() as u32).min(size.height);

    let titlebar = Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: PhysicalSize::new(size.width, bar).into(),
    };
    let content = Rect {
        position: PhysicalPosition::new(0, bar as i32).into(),
        size: PhysicalSize::new(size.width, size.height - bar).into(),
    };
    (titlebar, content)
}

/// Content bounds for the chat webview when it is first created.
pub(crate) fn content_bounds(window: &Window) -> Rect {
    webview_bounds(window).1
}

/// Re-applies the webview layout after the window is resized.
pub(crate) fn layout_main_window(window: &Window) {
    let size = window.inner_size().unwrap_or_default();
    // A minimized window reports a zero size on Windows; keep the old layout.
    if size.width == 0 || size.height == 0 {
        return;
    }

    let (titlebar_bounds, content_bounds) = webview_bounds(window);
    for webview in window.webviews() {
        let bounds = match webview.label() {
            TITLEBAR_WEBVIEW_LABEL => titlebar_bounds,
            CONTENT_WEBVIEW_LABEL => content_bounds,
            _ => continue,
        };
        if let Err(error) = webview.set_bounds(bounds) {
            eprintln!("failed to lay out {} webview: {}", webview.label(), error);
        }
    }

    #[cfg(windows)]
    crate::snap_layout::place_overlay(window);
}

/// Pushes the current window and page state into the title bar page.
pub(crate) fn sync_titlebar(app: &AppHandle) {
    let (Some(titlebar), Some(window)) = (titlebar_webview(app), main_window(app)) else {
        return;
    };
    let state = app.state::<TitlebarState>();
    let page_title = state.page_title.lock().unwrap().clone();
    let payload = json!({
        "maximized": window.is_maximized().unwrap_or(false),
        // The menu popup takes focus while open; the window still counts as active.
        "focused": window.is_focused().unwrap_or(false) || is_menu_open(app),
        "loading": state.loading.load(Ordering::SeqCst),
        "title": page_title,
        "activeMenu": active_menu(app),
        "maximizeHover": state.maximize_hover.load(Ordering::SeqCst),
        "maximizePressed": state.maximize_pressed.load(Ordering::SeqCst),
    });
    let script = format!("window.__deniTitlebar && window.__deniTitlebar.update({payload});");
    let _ = titlebar.eval(script);
}

pub(crate) fn set_page_loading(app: &AppHandle, loading: bool) {
    app.state::<TitlebarState>()
        .loading
        .store(loading, Ordering::SeqCst);
    sync_titlebar(app);
}

/// Hover and press state of the maximize button, reported by the Snap Layouts
/// overlay on Windows.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn set_maximize_button_state(app: &AppHandle, hover: bool, pressed: bool) {
    let state = app.state::<TitlebarState>();
    let hover_changed = state.maximize_hover.swap(hover, Ordering::SeqCst) != hover;
    let pressed_changed = state.maximize_pressed.swap(pressed, Ordering::SeqCst) != pressed;
    if hover_changed || pressed_changed {
        sync_titlebar(app);
    }
}

pub(crate) fn set_page_title(app: &AppHandle, title: String) {
    *app.state::<TitlebarState>().page_title.lock().unwrap() = title;
    sync_titlebar(app);
}

/// Asks the title bar to open the menu next to the current one, for arrow-key
/// navigation inside the menu popup. The title bar knows where its buttons are.
pub(crate) fn open_adjacent_menu(app: &AppHandle, delta: i32) {
    if let Some(titlebar) = titlebar_webview(app) {
        let script =
            format!("window.__deniTitlebar && window.__deniTitlebar.openAdjacent({delta});");
        let _ = titlebar.eval(script);
    }
}

fn require_titlebar(webview: &Webview) -> Result<(), String> {
    if webview.label() == TITLEBAR_WEBVIEW_LABEL {
        Ok(())
    } else {
        Err("forbidden".into())
    }
}

#[tauri::command]
pub(crate) fn titlebar_ready(webview: Webview) -> Result<(), String> {
    require_titlebar(&webview)?;
    sync_titlebar(webview.app_handle());
    Ok(())
}

#[tauri::command]
pub(crate) fn titlebar_window(webview: Webview, action: String) -> Result<(), String> {
    require_titlebar(&webview)?;
    let app = webview.app_handle();
    let window = webview.window();
    close_menu(app, false);

    let result = match action.as_str() {
        "drag" => window.start_dragging(),
        "minimize" => window.minimize(),
        "toggle-maximize" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        // Same as the native close button: hides to the tray.
        "close" => window.close(),
        _ => return Err(format!("unknown window action: {action}")),
    };
    if let Err(error) = result {
        eprintln!("failed to run window action {action}: {error}");
    }

    // Clicking the title bar moves keyboard focus into it; hand it back so
    // typing keeps going to the chat.
    if matches!(action.as_str(), "drag" | "toggle-maximize") {
        focus_content(app);
    }
    Ok(())
}

// Async so it runs off the main thread: a menu command may open a window
// (About), and creating a webview from a synchronous command deadlocks
// WebView2 on Windows.
#[tauri::command]
pub(crate) async fn titlebar_command(webview: Webview, id: String) -> Result<(), String> {
    require_titlebar(&webview)?;
    let app = webview.app_handle();
    close_menu(app, false);
    focus_content(app);
    run_menu_command(app, &id);
    Ok(())
}
