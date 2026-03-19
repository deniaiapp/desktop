use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use tauri::{
  Manager,
  webview::{NewWindowResponse, PageLoadEvent, Url, WebviewWindowBuilder},
};
use tauri_plugin_opener::OpenerExt;

const APP_ORIGIN: &str = "https://deniai.app";
const MAIN_WINDOW_LABEL: &str = "main";

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

fn open_external(app: &tauri::AppHandle, url: &Url) {
  if let Err(error) = app.opener().open_url(url.as_str(), None::<&str>) {
    eprintln!("failed to open external URL {}: {}", url, error);
  }
}

fn navigate_main_window(app: &tauri::AppHandle, url: Url) {
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    if let Err(error) = window.navigate(url) {
      eprintln!("failed to navigate main window: {}", error);
    }
  }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let window_shown = Arc::new(AtomicBool::new(false));

  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
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

      let navigation_handle = app_handle.clone();
      let new_window_handle = app_handle.clone();
      let page_load_visible = Arc::clone(&window_shown);
      let fallback_handle = app_handle.clone();
      let fallback_visible = Arc::clone(&window_shown);

      let _main_window = WebviewWindowBuilder::from_config(app.handle(), &main_window_config)?
        .on_navigation(move |url| {
          if is_allowed_url(url) {
            return true;
          }

          open_external(&navigation_handle, url);
          false
        })
        .on_new_window(move |url, _features| {
          if is_allowed_url(&url) {
            navigate_main_window(&new_window_handle, url);
          } else {
            open_external(&new_window_handle, &url);
          }

          NewWindowResponse::Deny
        })
        .on_page_load(move |window, payload| {
          if matches!(payload.event(), PageLoadEvent::Finished)
            && !page_load_visible.swap(true, Ordering::SeqCst)
          {
            let _ = window.show();
            let _ = window.set_focus();
          }
        })
        .build()?;

      std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));

        if !fallback_visible.swap(true, Ordering::SeqCst) {
          if let Some(window) = fallback_handle.get_webview_window(MAIN_WINDOW_LABEL) {
            let _ = window.show();
            let _ = window.set_focus();
          }
        }
      });

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
