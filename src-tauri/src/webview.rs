use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use tauri::{
    webview::{DownloadEvent, NewWindowResponse, Url},
    AppHandle, Manager, WebviewWindow, Wry,
};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

use crate::state::{app_title, loading_title, DesktopState};
use crate::url_guard::{default_app_url, is_allowed_url};

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";
pub(crate) const INITIAL_SHOW_DELAY: Duration = Duration::from_secs(4);

const ONLINE_STATUS_SCRIPT: &str = r#"
(() => {
  const BANNER_ID = "__deniDesktopStatusBanner";
  const STYLE_ID = "__deniDesktopStatusBannerStyle";

  const ensureStyle = () => {
    if (document.getElementById(STYLE_ID)) {
      return;
    }

    const style = document.createElement("style");
    style.id = STYLE_ID;
    style.textContent = `
      #${BANNER_ID} {
        position: fixed;
        right: 16px;
        bottom: 16px;
        z-index: 2147483647;
        display: flex;
        gap: 12px;
        align-items: center;
        max-width: min(420px, calc(100vw - 32px));
        padding: 12px 14px;
        border-radius: 14px;
        border: 1px solid rgba(255, 255, 255, 0.14);
        background: rgba(19, 24, 33, 0.96);
        color: #f6f8fb;
        box-shadow: 0 18px 40px rgba(0, 0, 0, 0.28);
        font: 13px/1.45 "Segoe UI", system-ui, sans-serif;
        opacity: 0;
        pointer-events: none;
        transform: translateY(8px);
        transition: opacity 160ms ease, transform 160ms ease;
      }

      #${BANNER_ID}[data-visible="true"] {
        opacity: 1;
        pointer-events: auto;
        transform: translateY(0);
      }

      #${BANNER_ID} button {
        border: 0;
        border-radius: 999px;
        padding: 7px 12px;
        background: #f4f7fb;
        color: #111827;
        font: inherit;
        font-weight: 600;
        cursor: pointer;
      }

      #${BANNER_ID} strong {
        display: block;
        margin-bottom: 2px;
        font-size: 13px;
      }

      #${BANNER_ID} span {
        color: rgba(246, 248, 251, 0.78);
      }
    `;
    document.documentElement.appendChild(style);
  };

  const ensureBanner = () => {
    let banner = document.getElementById(BANNER_ID);
    if (banner) {
      return banner;
    }

    banner = document.createElement("div");
    banner.id = BANNER_ID;
    banner.innerHTML = `
      <div>
        <strong>Connection lost</strong>
        <span>Deni AI will keep the window open. Retry when your network is back.</span>
      </div>
      <button type="button">Retry</button>
    `;
    banner.querySelector("button")?.addEventListener("click", () => window.location.reload());
    document.documentElement.appendChild(banner);
    return banner;
  };

  const syncBanner = () => {
    ensureStyle();
    const banner = ensureBanner();
    banner.dataset.visible = String(!navigator.onLine);
  };

  if (!window.__deniDesktopBannerHooked) {
    window.addEventListener("online", syncBanner);
    window.addEventListener("offline", syncBanner);
    window.__deniDesktopBannerHooked = true;
  }

  syncBanner();
})();
"#;

pub(crate) fn notify(app: &AppHandle, title: impl Into<String>, body: impl Into<String>) {
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        eprintln!("failed to show notification: {}", error);
    }
}

pub(crate) fn open_external(app: &AppHandle, url: &Url) {
    if let Err(error) = app.opener().open_url(url.as_str(), None::<&str>) {
        eprintln!("failed to open external URL {}: {}", url, error);
    }
}

pub(crate) fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
}

pub(crate) fn show_main_window(window: &WebviewWindow) {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }

    let _ = window.show();
    let _ = window.set_focus();
}

pub(crate) fn show_main_window_from_app(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        show_main_window(&window);
    }
}

pub(crate) fn hide_main_window(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

pub(crate) fn navigate_main_window(app: &AppHandle, url: Url) {
    if let Some(window) = main_window(app) {
        if let Err(error) = window.navigate(url) {
            eprintln!("failed to navigate main window: {}", error);
        }
    }
}

pub(crate) fn retry_current_page(app: &AppHandle) {
    let next_url = app
        .state::<DesktopState>()
        .current_url()
        .parse::<Url>()
        .ok()
        .filter(is_allowed_url)
        .unwrap_or_else(default_app_url);
    navigate_main_window(app, next_url);
}

pub(crate) fn reload_current_page(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        if window.eval("window.location.reload();").is_err() {
            retry_current_page(app);
        }
    }
}

pub(crate) fn navigate_history(app: &AppHandle, direction: &str) {
    if let Some(window) = main_window(app) {
        let script = match direction {
            "back" => "window.history.back();",
            "forward" => "window.history.forward();",
            _ => return,
        };
        let _ = window.eval(script);
    }
}

pub(crate) fn set_zoom(app: &AppHandle, zoom_factor: f64) {
    let zoom_factor =
        zoom_factor.clamp(crate::state::MIN_ZOOM_FACTOR, crate::state::MAX_ZOOM_FACTOR);
    app.state::<DesktopState>().set_zoom_factor(zoom_factor);

    if let Some(window) = main_window(app) {
        if let Err(error) = window.set_zoom(zoom_factor) {
            eprintln!("failed to set zoom factor: {}", error);
        }
    }
}

pub(crate) fn adjust_zoom(app: &AppHandle, delta: f64) {
    let next_zoom = app.state::<DesktopState>().zoom_factor() + delta;
    set_zoom(app, next_zoom);
}

pub(crate) fn open_current_page_in_browser(app: &AppHandle) {
    if let Ok(url) = app.state::<DesktopState>().current_url().parse::<Url>() {
        open_external(app, &url);
    } else {
        open_external(app, &default_app_url());
    }
}

pub(crate) fn open_downloads_folder(app: &AppHandle) {
    match app.path().download_dir() {
        Ok(path) => {
            if let Err(error) = app
                .opener()
                .open_path(path.to_string_lossy().into_owned(), None::<&str>)
            {
                eprintln!("failed to open downloads folder: {}", error);
            }
        }
        Err(error) => notify(
            app,
            app_title(app),
            format!("Couldn't open Downloads: {error}"),
        ),
    }
}

pub(crate) fn open_latest_download(app: &AppHandle) {
    let maybe_path = app.state::<DesktopState>().last_download_path();

    match maybe_path {
        Some(path) if path.exists() => {
            if let Err(error) = app
                .opener()
                .open_path(path.to_string_lossy().into_owned(), None::<&str>)
            {
                eprintln!("failed to open latest download: {}", error);
            }
        }
        _ => notify(
            app,
            app_title(app),
            "No completed download has been captured in this session yet.",
        ),
    }
}

pub(crate) fn inject_online_status_banner(window: &WebviewWindow) {
    if let Err(error) = window.eval(ONLINE_STATUS_SCRIPT) {
        eprintln!("failed to inject online status banner: {}", error);
    }
}

pub(crate) fn maybe_notify_tray_behavior(app: &AppHandle) {
    let state = app.state::<DesktopState>();
    if !state.mark_tray_notice_sent() {
        notify(
            app,
            app_title(app),
            "Closing the window keeps Deni AI running in the tray. Use Quit to exit fully.",
        );
    }
}

pub(crate) fn handle_navigation_event(app: &AppHandle, url: &Url) -> bool {
    if is_allowed_url(url) {
        app.state::<DesktopState>().set_current_url(url.to_string());
        return true;
    }

    open_external(app, url);
    false
}

pub(crate) fn handle_new_window_event(app: &AppHandle, url: Url) -> NewWindowResponse<Wry> {
    if is_allowed_url(&url) {
        app.state::<DesktopState>().set_current_url(url.to_string());
        navigate_main_window(app, url);
    } else {
        open_external(app, &url);
    }

    NewWindowResponse::Deny
}

pub(crate) fn handle_download_event(app: &AppHandle, event: DownloadEvent) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => {
            eprintln!("downloading {} to {:?}", url, destination);
        }
        DownloadEvent::Finished { url, path, success } => {
            if success {
                if let Some(path) = path {
                    app.state::<DesktopState>()
                        .set_last_download_path(path.clone());
                    notify(
                        app,
                        app_title(app),
                        format!("Downloaded {} to {}", url, path.display()),
                    );
                } else {
                    notify(app, app_title(app), format!("Downloaded {}", url));
                }
            } else {
                notify(app, app_title(app), format!("Download failed for {}", url));
            }
        }
        _ => {}
    }

    true
}

pub(crate) fn handle_page_load_started(window: &WebviewWindow, url: String) {
    let app_handle = window.app_handle();
    app_handle.state::<DesktopState>().set_current_url(url);
    let _ = window.set_title(&loading_title(app_handle));
}

pub(crate) fn handle_page_load_finished(window: &WebviewWindow, url: String, shown: &AtomicBool) {
    let app_handle = window.app_handle();
    app_handle.state::<DesktopState>().set_current_url(url);
    let _ = window.set_title(&app_title(app_handle));
    let zoom_factor = app_handle.state::<DesktopState>().zoom_factor();
    let _ = window.set_zoom(zoom_factor);
    inject_online_status_banner(window);

    if !shown.swap(true, Ordering::SeqCst) {
        show_main_window(window);
    }
}
