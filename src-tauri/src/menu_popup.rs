use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use serde_json::json;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Webview, WebviewWindow};

use crate::menu::run_menu_command;
use crate::titlebar::{open_adjacent_menu, sync_titlebar, TITLEBAR_WEBVIEW_LABEL};
use crate::webview::{focus_content, main_window};

pub(crate) const MENU_POPUP_LABEL: &str = "menu-popup";

/// Gap between a title bar button and the menu panel below it, in logical px.
const MENU_GAP: f64 = 4.0;
/// Transparent space menu.html leaves above and left of the panel for its shadow.
const PANEL_INSET: f64 = 8.0;
/// Pressing a title bar button blurs (and so closes) its open menu just before
/// the press arrives; within this window the press only closes the menu.
const REOPEN_GUARD: Duration = Duration::from_millis(300);
/// The click that opens a menu also focuses the title bar webview, and WebView2
/// applies that focus change after the popup has taken focus. A focus change
/// this soon after opening is usually that echo, not the user leaving.
const FOCUS_SETTLE: Duration = Duration::from_millis(250);

/// The custom menu popup: one hidden window reused for every title bar menu.
#[derive(Default)]
pub(crate) struct MenuPopupState {
    inner: Mutex<PopupInner>,
}

#[derive(Default)]
struct PopupInner {
    /// The menu that is open, or about to be shown.
    active: Option<String>,
    /// Bottom-left corner of the button that opened it, in logical client px.
    anchor: (f64, f64),
    last_closed: Option<(String, Instant)>,
    /// When the popup was last placed and focused.
    presented_at: Option<Instant>,
}

fn popup_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MENU_POPUP_LABEL)
}

pub(crate) fn active_menu(app: &AppHandle) -> Option<String> {
    app.state::<MenuPopupState>()
        .inner
        .lock()
        .unwrap()
        .active
        .clone()
}

pub(crate) fn is_menu_open(app: &AppHandle) -> bool {
    active_menu(app).is_some()
}

/// Creates the hidden menu popup window. macOS keeps the native menu bar, so
/// it does not need one.
#[cfg(not(target_os = "macos"))]
pub(crate) fn create_menu_popup(app: &AppHandle, owner: &tauri::Window) -> tauri::Result<()> {
    use tauri::{webview::NewWindowResponse, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    use crate::titlebar::{chrome_url, handle_chrome_navigation};

    let builder = WebviewWindowBuilder::new(
        app,
        MENU_POPUP_LABEL,
        WebviewUrl::CustomProtocol(chrome_url("/")),
    )
    .title("Menu")
    .inner_size(320.0, 480.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .on_navigation(handle_chrome_navigation)
    .on_new_window(|_, _| NewWindowResponse::Deny);

    // An owned window stays above the main window and never gets its own
    // taskbar button.
    #[cfg(windows)]
    let builder = builder.owner_raw(owner.hwnd()?);
    #[cfg(not(windows))]
    let _ = owner;

    let popup = builder.build()?;
    // Windows and Linux attach the app menu to every window.
    let _ = popup.remove_menu();

    let blur_handle = app.clone();
    popup.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            handle_popup_blur(&blur_handle);
        }
    });
    Ok(())
}

fn focus_popup(popup: &WebviewWindow) {
    let _ = popup.set_focus();
    // Focus the page itself so arrow keys work and leaving it closes the menu.
    let popup_webview: &Webview = popup.as_ref();
    let _ = popup_webview.set_focus();
}

/// Whether the popup was opened so recently that a focus change is more
/// likely the opening click settling than the user moving on.
fn is_settling(app: &AppHandle) -> bool {
    let state = app.state::<MenuPopupState>();
    let inner = state.inner.lock().unwrap();
    inner.active.is_some()
        && inner
            .presented_at
            .is_some_and(|at| at.elapsed() < FOCUS_SETTLE)
}

fn main_window_has_focus(app: &AppHandle) -> bool {
    main_window(app).is_some_and(|window| window.is_focused().unwrap_or(false))
}

fn handle_popup_blur(app: &AppHandle) {
    if is_settling(app) {
        // The main window is not reported active yet at this point, so decide
        // once focus has settled.
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(FOCUS_SETTLE);
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || recheck_popup_focus(&handle));
        });
        return;
    }
    dismiss_on_focus_loss(app);
}

/// After an early blur: take focus back from our own title bar, or close the
/// menu if the user really moved to another app.
fn recheck_popup_focus(app: &AppHandle) {
    let Some(popup) = popup_window(app) else {
        return;
    };
    if !is_menu_open(app) || popup.is_focused().unwrap_or(false) {
        return;
    }
    if main_window_has_focus(app) {
        focus_popup(&popup);
    } else {
        dismiss_on_focus_loss(app);
    }
}

fn dismiss_on_focus_loss(app: &AppHandle) {
    {
        let state = app.state::<MenuPopupState>();
        let mut inner = state.inner.lock().unwrap();
        let Some(active) = inner.active.clone() else {
            return;
        };
        inner.last_closed = Some((active, Instant::now()));
    }
    close_menu(app, false);
}

/// The main window took focus back, e.g. from a click into the chat. That
/// normally blurs the popup too; this also covers a popup that never got
/// keyboard focus.
pub(crate) fn handle_main_window_focused(app: &AppHandle) {
    // While a menu is still being rendered the popup is hidden; the click that
    // opened it may be what focused the main window.
    let Some(popup) = popup_window(app).filter(|popup| popup.is_visible().unwrap_or(false)) else {
        return;
    };
    if is_settling(app) {
        focus_popup(&popup);
    } else {
        dismiss_on_focus_loss(app);
    }
}

/// Hides the menu popup if it is open. With `refocus`, keyboard focus goes
/// back to the chat page.
pub(crate) fn close_menu(app: &AppHandle, refocus: bool) {
    let was_open = app
        .state::<MenuPopupState>()
        .inner
        .lock()
        .unwrap()
        .active
        .take()
        .is_some();
    if !was_open {
        return;
    }

    if let Some(popup) = popup_window(app) {
        let _ = popup.hide();
    }
    if refocus {
        focus_content(app);
    }
    sync_titlebar(app);
}

fn require_label(webview: &Webview, label: &str) -> Result<(), String> {
    if webview.label() == label {
        Ok(())
    } else {
        Err("forbidden".into())
    }
}

/// Title bar: open (or toggle) the menu `section` below the button whose
/// bottom-left corner is at `x`, `y`.
#[tauri::command]
pub(crate) fn menu_open(
    webview: Webview,
    section: String,
    x: f64,
    y: f64,
    keyboard: bool,
) -> Result<(), String> {
    require_label(&webview, TITLEBAR_WEBVIEW_LABEL)?;
    let app = webview.app_handle();

    {
        let state = app.state::<MenuPopupState>();
        let mut inner = state.inner.lock().unwrap();
        if inner.active.as_deref() == Some(section.as_str()) {
            drop(inner);
            close_menu(app, true);
            return Ok(());
        }
        if let Some((closed, at)) = inner.last_closed.take() {
            if closed == section && at.elapsed() < REOPEN_GUARD {
                drop(inner);
                focus_content(app);
                return Ok(());
            }
        }
        inner.active = Some(section.clone());
        inner.anchor = (x, y);
    }

    if let Some(popup) = popup_window(app) {
        // menu.html renders the menu, measures it and calls `menu_present`.
        let script = format!(
            "window.__deniMenu && window.__deniMenu.open({}, {keyboard});",
            json!(section)
        );
        let _ = popup.eval(script);
    }
    sync_titlebar(app);
    Ok(())
}

/// Menu popup: the menu `section` is rendered and measures `width` x `height`
/// CSS px, shadow included; place the window and show it.
#[tauri::command]
pub(crate) fn menu_present(
    webview: Webview,
    section: String,
    width: f64,
    height: f64,
) -> Result<(), String> {
    require_label(&webview, MENU_POPUP_LABEL)?;
    let app = webview.app_handle();

    let anchor = {
        let state = app.state::<MenuPopupState>();
        let mut inner = state.inner.lock().unwrap();
        if inner.active.as_deref() != Some(section.as_str()) {
            // The user already moved on to another menu.
            return Ok(());
        }
        inner.presented_at = Some(Instant::now());
        inner.anchor
    };
    let (Some(window), Some(popup)) = (main_window(app), popup_window(app)) else {
        return Ok(());
    };

    let origin = window.inner_position().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let position = PhysicalPosition::new(
        origin.x + ((anchor.0 - PANEL_INSET) * scale).round() as i32,
        origin.y + ((anchor.1 + MENU_GAP - PANEL_INSET) * scale).round() as i32,
    );
    let size = PhysicalSize::new(
        (width * scale).ceil() as u32,
        (height * scale).ceil() as u32,
    );

    let _ = popup.set_position(position);
    let _ = popup.set_size(size);
    let _ = popup.show();
    focus_popup(&popup);
    Ok(())
}

/// Menu popup: Left/Right arrow keys move to the neighbouring menu.
#[tauri::command]
pub(crate) fn menu_step(webview: Webview, delta: i32) -> Result<(), String> {
    require_label(&webview, MENU_POPUP_LABEL)?;
    open_adjacent_menu(webview.app_handle(), delta.signum());
    Ok(())
}

/// Menu popup: Escape or a click outside the panel.
#[tauri::command]
pub(crate) fn menu_dismiss(webview: Webview) -> Result<(), String> {
    require_label(&webview, MENU_POPUP_LABEL)?;
    close_menu(webview.app_handle(), true);
    Ok(())
}

/// Menu popup: run the chosen menu item.
///
/// Async so it runs off the main thread: a menu command may open a window
/// (About), and creating a webview from a synchronous command deadlocks
/// WebView2 on Windows.
#[tauri::command]
pub(crate) async fn menu_invoke(webview: Webview, id: String) -> Result<(), String> {
    require_label(&webview, MENU_POPUP_LABEL)?;
    let app = webview.app_handle();
    close_menu(app, true);
    run_menu_command(app, &id);
    Ok(())
}
