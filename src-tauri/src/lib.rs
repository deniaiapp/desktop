use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use tauri::{
    menu::{AboutMetadataBuilder, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, Url, WebviewWindowBuilder},
    AppHandle, Manager, WindowEvent, Wry,
};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Error as UpdaterError, UpdaterExt};
use tauri_plugin_window_state::StateFlags;

const APP_ORIGIN: &str = "https://deniai.app";
const APP_START_URL: &str = "https://deniai.app/chat";
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";

const MENU_SHOW_WINDOW: &str = "show-window";
const MENU_HIDE_TO_TRAY: &str = "hide-to-tray";
const MENU_OPEN_IN_BROWSER: &str = "open-in-browser";
const MENU_OPEN_DOWNLOADS: &str = "open-downloads";
const MENU_OPEN_LATEST_DOWNLOAD: &str = "open-latest-download";
const MENU_NAV_BACK: &str = "nav-back";
const MENU_NAV_FORWARD: &str = "nav-forward";
const MENU_RELOAD: &str = "reload";
const MENU_RETRY_CONNECTION: &str = "retry-connection";
const MENU_ZOOM_IN: &str = "zoom-in";
const MENU_ZOOM_OUT: &str = "zoom-out";
const MENU_ZOOM_RESET: &str = "zoom-reset";
const MENU_CHECK_UPDATES: &str = "check-updates";
const MENU_QUIT: &str = "quit";

const INITIAL_SHOW_DELAY: Duration = Duration::from_secs(4);
const MIN_ZOOM_FACTOR: f64 = 0.8;
const MAX_ZOOM_FACTOR: f64 = 2.0;
const DEFAULT_ZOOM_FACTOR: f64 = 1.0;

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

struct DesktopState {
    app_title: String,
    current_url: Mutex<String>,
    zoom_factor: Mutex<f64>,
    last_download_path: Mutex<Option<PathBuf>>,
    is_quitting: AtomicBool,
    tray_notice_sent: AtomicBool,
}

impl DesktopState {
    fn new(app_title: String) -> Self {
        Self {
            app_title,
            current_url: Mutex::new(APP_START_URL.to_string()),
            zoom_factor: Mutex::new(DEFAULT_ZOOM_FACTOR),
            last_download_path: Mutex::new(None),
            is_quitting: AtomicBool::new(false),
            tray_notice_sent: AtomicBool::new(false),
        }
    }
}

struct TrayState {
    _tray: TrayIcon<Wry>,
}

fn default_app_url() -> Url {
    APP_START_URL
        .parse()
        .expect("default app URL must be valid")
}

fn app_title(app: &AppHandle) -> String {
    app.state::<DesktopState>().app_title.clone()
}

fn loading_title(app: &AppHandle) -> String {
    format!("{} - Loading", app_title(app))
}

fn set_current_url(app: &AppHandle, url: impl Into<String>) {
    *app.state::<DesktopState>().current_url.lock().unwrap() = url.into();
}

fn current_url(app: &AppHandle) -> String {
    app.state::<DesktopState>()
        .current_url
        .lock()
        .unwrap()
        .clone()
}

fn set_last_download_path(app: &AppHandle, path: PathBuf) {
    *app.state::<DesktopState>()
        .last_download_path
        .lock()
        .unwrap() = Some(path);
}

fn is_in_app_url(url: &Url) -> bool {
    if url.origin().ascii_serialization() != APP_ORIGIN {
        return false;
    }

    matches!(url.path(), "/chat" | "/auth/sign-in")
        || url.path().starts_with("/chat/")
        || url.path().starts_with("/auth/sign-in/")
}

fn is_app_managed_auth_url(url: &Url) -> bool {
    if url.origin().ascii_serialization() == APP_ORIGIN
        && url.path().starts_with("/api/auth/callback/google")
    {
        return true;
    }

    url.scheme() == "https"
        && (matches!(url.domain(), Some("accounts.google.com"))
            || url
                .domain()
                .is_some_and(|domain| domain.ends_with(".googleusercontent.com")))
}

fn is_allowed_url(url: &Url) -> bool {
    is_in_app_url(url) || is_app_managed_auth_url(url)
}

fn notify(app: &AppHandle, title: impl Into<String>, body: impl Into<String>) {
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        eprintln!("failed to show notification: {}", error);
    }
}

fn open_external(app: &AppHandle, url: &Url) {
    if let Err(error) = app.opener().open_url(url.as_str(), None::<&str>) {
        eprintln!("failed to open external URL {}: {}", url, error);
    }
}

fn main_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
}

fn show_main_window(window: &tauri::WebviewWindow) {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }

    let _ = window.show();
    let _ = window.set_focus();
}

fn show_main_window_from_app(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        show_main_window(&window);
    }
}

fn hide_main_window(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

fn navigate_main_window(app: &AppHandle, url: Url) {
    if let Some(window) = main_window(app) {
        if let Err(error) = window.navigate(url) {
            eprintln!("failed to navigate main window: {}", error);
        }
    }
}

fn retry_current_page(app: &AppHandle) {
    let next_url = current_url(app)
        .parse::<Url>()
        .ok()
        .filter(is_allowed_url)
        .unwrap_or_else(default_app_url);
    navigate_main_window(app, next_url);
}

fn reload_current_page(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        if window.eval("window.location.reload();").is_err() {
            retry_current_page(app);
        }
    }
}

fn navigate_history(app: &AppHandle, direction: &str) {
    if let Some(window) = main_window(app) {
        let script = match direction {
            "back" => "window.history.back();",
            "forward" => "window.history.forward();",
            _ => return,
        };
        let _ = window.eval(script);
    }
}

fn set_zoom(app: &AppHandle, zoom_factor: f64) {
    let zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
    *app.state::<DesktopState>().zoom_factor.lock().unwrap() = zoom_factor;

    if let Some(window) = main_window(app) {
        if let Err(error) = window.set_zoom(zoom_factor) {
            eprintln!("failed to set zoom factor: {}", error);
        }
    }
}

fn adjust_zoom(app: &AppHandle, delta: f64) {
    let next_zoom = {
        let current_zoom = *app.state::<DesktopState>().zoom_factor.lock().unwrap();
        current_zoom + delta
    };
    set_zoom(app, next_zoom);
}

fn open_current_page_in_browser(app: &AppHandle) {
    if let Ok(url) = current_url(app).parse::<Url>() {
        open_external(app, &url);
    } else {
        open_external(app, &default_app_url());
    }
}

fn open_downloads_folder(app: &AppHandle) {
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

fn open_latest_download(app: &AppHandle) {
    let maybe_path = app
        .state::<DesktopState>()
        .last_download_path
        .lock()
        .unwrap()
        .clone();

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

fn inject_online_status_banner(window: &tauri::WebviewWindow) {
    if let Err(error) = window.eval(ONLINE_STATUS_SCRIPT) {
        eprintln!("failed to inject online status banner: {}", error);
    }
}

fn maybe_notify_tray_behavior(app: &AppHandle) {
    let state = app.state::<DesktopState>();
    if !state.tray_notice_sent.swap(true, Ordering::SeqCst) {
        notify(
            app,
            app_title(app),
            "Closing the window keeps Deni AI running in the tray. Use Quit to exit fully.",
        );
    }
}

fn check_for_updates(app: AppHandle, interactive: bool) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(updater) => updater,
            Err(error) => {
                if interactive {
                    notify(
                        &app,
                        app_title(&app),
                        format!("Couldn't start the updater: {error}"),
                    );
                } else {
                    eprintln!("failed to initialize updater: {}", error);
                }
                return;
            }
        };

        match updater.check().await {
      Ok(Some(update)) => {
        let version = update.version.clone();

        if interactive {
          notify(
            &app,
            app_title(&app),
            format!("Downloading Deni AI {version}. The installer will run when the download finishes."),
          );

          match update.download_and_install(|_, _| {}, || {}).await {
            Ok(()) => notify(
              &app,
              app_title(&app),
              format!("Deni AI {version} is ready to install."),
            ),
            Err(error) => notify(
              &app,
              app_title(&app),
              format!("The update download failed: {error}"),
            ),
          }
        } else {
          notify(
            &app,
            app_title(&app),
            format!("Deni AI {version} is available. Use Help > Check for Updates to install it."),
          );
        }
      }
      Ok(None) if interactive => {
        notify(&app, app_title(&app), "You're already on the latest desktop build.")
      }
      Ok(None) => {}
      Err(UpdaterError::EmptyEndpoints) if interactive => notify(
        &app,
        app_title(&app),
        "Updater support is wired in, but this build does not have a release feed configured yet.",
      ),
      Err(UpdaterError::EmptyEndpoints) => {}
      Err(error) if interactive => notify(
        &app,
        app_title(&app),
        format!("Couldn't check for updates: {error}"),
      ),
      Err(error) => eprintln!("background update check failed: {}", error),
    }
    });
}

fn quit_app(app: &AppHandle) {
    app.state::<DesktopState>()
        .is_quitting
        .store(true, Ordering::SeqCst);
    app.exit(0);
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW_WINDOW => show_main_window_from_app(app),
        MENU_HIDE_TO_TRAY => hide_main_window(app),
        MENU_OPEN_IN_BROWSER => open_current_page_in_browser(app),
        MENU_OPEN_DOWNLOADS => open_downloads_folder(app),
        MENU_OPEN_LATEST_DOWNLOAD => open_latest_download(app),
        MENU_NAV_BACK => navigate_history(app, "back"),
        MENU_NAV_FORWARD => navigate_history(app, "forward"),
        MENU_RELOAD => reload_current_page(app),
        MENU_RETRY_CONNECTION => retry_current_page(app),
        MENU_ZOOM_IN => adjust_zoom(app, 0.1),
        MENU_ZOOM_OUT => adjust_zoom(app, -0.1),
        MENU_ZOOM_RESET => set_zoom(app, DEFAULT_ZOOM_FACTOR),
        MENU_CHECK_UPDATES => check_for_updates(app.clone(), true),
        MENU_QUIT => quit_app(app),
        _ => {}
    }
}

fn build_app_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let show_window = MenuItem::with_id(app, MENU_SHOW_WINDOW, "Show Deni AI", true, None::<&str>)?;
    let hide_to_tray =
        MenuItem::with_id(app, MENU_HIDE_TO_TRAY, "Hide to Tray", true, Some("Ctrl+W"))?;
    let open_in_browser = MenuItem::with_id(
        app,
        MENU_OPEN_IN_BROWSER,
        "Open in Browser",
        true,
        Some("Ctrl+Shift+O"),
    )?;
    let open_downloads = MenuItem::with_id(
        app,
        MENU_OPEN_DOWNLOADS,
        "Open Downloads Folder",
        true,
        Some("Ctrl+Shift+D"),
    )?;
    let open_latest_download = MenuItem::with_id(
        app,
        MENU_OPEN_LATEST_DOWNLOAD,
        "Open Latest Download",
        true,
        Some("Ctrl+Alt+O"),
    )?;
    let nav_back = MenuItem::with_id(app, MENU_NAV_BACK, "Back", true, Some("Alt+Left"))?;
    let nav_forward = MenuItem::with_id(app, MENU_NAV_FORWARD, "Forward", true, Some("Alt+Right"))?;
    let reload = MenuItem::with_id(app, MENU_RELOAD, "Reload", true, Some("Ctrl+R"))?;
    let retry_connection = MenuItem::with_id(
        app,
        MENU_RETRY_CONNECTION,
        "Retry Connection",
        true,
        Some("Ctrl+Shift+R"),
    )?;
    let zoom_in = MenuItem::with_id(app, MENU_ZOOM_IN, "Zoom In", true, Some("Ctrl+="))?;
    let zoom_out = MenuItem::with_id(app, MENU_ZOOM_OUT, "Zoom Out", true, Some("Ctrl+-"))?;
    let zoom_reset = MenuItem::with_id(app, MENU_ZOOM_RESET, "Actual Size", true, Some("Ctrl+0"))?;
    let check_updates = MenuItem::with_id(
        app,
        MENU_CHECK_UPDATES,
        "Check for Updates",
        true,
        Some("Ctrl+Shift+U"),
    )?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, Some("Ctrl+Q"))?;

    let about = PredefinedMenuItem::about(
        app,
        None,
        Some(
            AboutMetadataBuilder::new()
                .website(Some("https://deniai.app"))
                .website_label(Some("deniai.app"))
                .build(),
        ),
    )?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &show_window,
            &hide_to_tray,
            &PredefinedMenuItem::separator(app)?,
            &open_in_browser,
            &open_downloads,
            &open_latest_download,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &nav_back,
            &nav_forward,
            &PredefinedMenuItem::separator(app)?,
            &reload,
            &retry_connection,
            &PredefinedMenuItem::separator(app)?,
            &zoom_in,
            &zoom_out,
            &zoom_reset,
        ],
    )?;

    let help_menu = Submenu::with_items(
        app,
        "Help",
        true,
        &[&check_updates, &PredefinedMenuItem::separator(app)?, &about],
    )?;

    Menu::with_items(app, &[&file_menu, &edit_menu, &view_menu, &help_menu])
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let show_window = MenuItem::with_id(app, MENU_SHOW_WINDOW, "Show Deni AI", true, None::<&str>)?;
    let reload = MenuItem::with_id(app, MENU_RELOAD, "Reload", true, None::<&str>)?;
    let open_in_browser = MenuItem::with_id(
        app,
        MENU_OPEN_IN_BROWSER,
        "Open in Browser",
        true,
        None::<&str>,
    )?;
    let open_downloads = MenuItem::with_id(
        app,
        MENU_OPEN_DOWNLOADS,
        "Open Downloads Folder",
        true,
        None::<&str>,
    )?;
    let open_latest_download = MenuItem::with_id(
        app,
        MENU_OPEN_LATEST_DOWNLOAD,
        "Open Latest Download",
        true,
        None::<&str>,
    )?;
    let check_updates = MenuItem::with_id(
        app,
        MENU_CHECK_UPDATES,
        "Check for Updates",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &show_window,
            &reload,
            &PredefinedMenuItem::separator(app)?,
            &open_in_browser,
            &open_downloads,
            &open_latest_download,
            &PredefinedMenuItem::separator(app)?,
            &check_updates,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

fn build_tray_icon(app: &AppHandle) -> tauri::Result<TrayIcon<Wry>> {
    let tray_menu = build_tray_menu(app)?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&tray_menu)
        .tooltip(app_title(app))
        .show_menu_on_left_click(false);

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)
}

fn handle_second_instance(app: &AppHandle, args: Vec<String>) {
    for arg in args {
        if let Ok(url) = arg.parse::<Url>() {
            if is_allowed_url(&url) {
                set_current_url(app, url.to_string());
                navigate_main_window(app, url);
                break;
            }
        }
    }

    show_main_window_from_app(app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let window_shown = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_second_instance(app, args);
        }))
        .menu(build_app_menu)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|app, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window_from_app(app),
            _ => {}
        })
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let main_window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == MAIN_WINDOW_LABEL)
                .cloned()
                .expect("main window config must exist");

            app.manage(DesktopState::new(main_window_config.title.clone()));
            app.manage(TrayState {
                _tray: build_tray_icon(&app_handle)?,
            });

            let navigation_handle = app_handle.clone();
            let navigation_state_handle = app_handle.clone();
            let new_window_handle = app_handle.clone();
            let new_window_state_handle = app_handle.clone();
            let page_load_visible = Arc::clone(&window_shown);
            let fallback_handle = app_handle.clone();
            let fallback_visible = Arc::clone(&window_shown);
            let download_handle = app_handle.clone();
            let close_handle = app_handle.clone();

            let main_window = WebviewWindowBuilder::from_config(app.handle(), &main_window_config)?
                .on_navigation(move |url| {
                    if is_allowed_url(url) {
                        set_current_url(&navigation_state_handle, url.to_string());
                        return true;
                    }

                    open_external(&navigation_handle, url);
                    false
                })
                .on_new_window(move |url, _features| {
                    if is_allowed_url(&url) {
                        set_current_url(&new_window_state_handle, url.to_string());
                        navigate_main_window(&new_window_handle, url);
                    } else {
                        open_external(&new_window_handle, &url);
                    }

                    NewWindowResponse::Deny
                })
                .on_download(move |_webview, event| {
                    match event {
                        DownloadEvent::Requested { url, destination } => {
                            eprintln!("downloading {} to {:?}", url, destination);
                        }
                        DownloadEvent::Finished { url, path, success } => {
                            if success {
                                if let Some(path) = path {
                                    set_last_download_path(&download_handle, path.clone());
                                    notify(
                                        &download_handle,
                                        app_title(&download_handle),
                                        format!("Downloaded {} to {}", url, path.display()),
                                    );
                                } else {
                                    notify(
                                        &download_handle,
                                        app_title(&download_handle),
                                        format!("Downloaded {}", url),
                                    );
                                }
                            } else {
                                notify(
                                    &download_handle,
                                    app_title(&download_handle),
                                    format!("Download failed for {}", url),
                                );
                            }
                        }
                        _ => {}
                    }

                    true
                })
                .on_page_load(move |window, payload| match payload.event() {
                    PageLoadEvent::Started => {
                        set_current_url(&window.app_handle(), payload.url().to_string());
                        let _ = window.set_title(&loading_title(&window.app_handle()));
                    }
                    PageLoadEvent::Finished => {
                        let app_handle = window.app_handle();
                        set_current_url(&app_handle, payload.url().to_string());
                        let _ = window.set_title(&app_title(&app_handle));
                        let zoom_factor = *app_handle
                            .state::<DesktopState>()
                            .zoom_factor
                            .lock()
                            .unwrap();
                        let _ = window.set_zoom(zoom_factor);
                        inject_online_status_banner(&window);

                        if !page_load_visible.swap(true, Ordering::SeqCst) {
                            show_main_window(&window);
                        }
                    }
                })
                .build()?;

            let _ = main_window.set_title(&loading_title(&app_handle));
            let close_window = main_window.clone();
            main_window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if !close_handle
                        .state::<DesktopState>()
                        .is_quitting
                        .load(Ordering::SeqCst)
                    {
                        api.prevent_close();
                        let _ = close_window.hide();
                        maybe_notify_tray_behavior(&close_handle);
                    }
                }
            });

            std::thread::spawn(move || {
                std::thread::sleep(INITIAL_SHOW_DELAY);

                if !fallback_visible.swap(true, Ordering::SeqCst) {
                    if let Some(window) = fallback_handle.get_webview_window(MAIN_WINDOW_LABEL) {
                        let _ = window.set_title(&loading_title(&fallback_handle));
                        show_main_window(&window);
                    }
                }
            });

            check_for_updates(app_handle.clone(), false);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
