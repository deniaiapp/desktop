use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use tauri::{AppHandle, Manager};

pub(crate) const MIN_ZOOM_FACTOR: f64 = 0.8;
pub(crate) const MAX_ZOOM_FACTOR: f64 = 2.0;
pub(crate) const DEFAULT_ZOOM_FACTOR: f64 = 1.0;

/// Shared, mutable desktop-shell state managed by Tauri.
///
/// All fields are private; callers go through the accessor methods below so
/// the locking strategy stays in one place.
pub(crate) struct DesktopState {
    app_title: String,
    current_url: Mutex<String>,
    zoom_factor: Mutex<f64>,
    last_download_path: Mutex<Option<PathBuf>>,
    is_quitting: AtomicBool,
    tray_notice_sent: AtomicBool,
}

impl DesktopState {
    pub(crate) fn new(app_title: String, start_url: &str) -> Self {
        Self {
            app_title,
            current_url: Mutex::new(start_url.to_string()),
            zoom_factor: Mutex::new(DEFAULT_ZOOM_FACTOR),
            last_download_path: Mutex::new(None),
            is_quitting: AtomicBool::new(false),
            tray_notice_sent: AtomicBool::new(false),
        }
    }

    pub(crate) fn app_title(&self) -> String {
        self.app_title.clone()
    }

    pub(crate) fn current_url(&self) -> String {
        self.current_url.lock().unwrap().clone()
    }

    pub(crate) fn set_current_url(&self, url: impl Into<String>) {
        *self.current_url.lock().unwrap() = url.into();
    }

    pub(crate) fn zoom_factor(&self) -> f64 {
        *self.zoom_factor.lock().unwrap()
    }

    pub(crate) fn set_zoom_factor(&self, zoom_factor: f64) {
        *self.zoom_factor.lock().unwrap() = zoom_factor;
    }

    pub(crate) fn last_download_path(&self) -> Option<PathBuf> {
        self.last_download_path.lock().unwrap().clone()
    }

    pub(crate) fn set_last_download_path(&self, path: PathBuf) {
        *self.last_download_path.lock().unwrap() = Some(path);
    }

    pub(crate) fn is_quitting(&self) -> bool {
        self.is_quitting.load(Ordering::SeqCst)
    }

    pub(crate) fn mark_quitting(&self) {
        self.is_quitting.store(true, Ordering::SeqCst);
    }

    /// Returns `true` if the tray-behavior notice had already been sent
    /// before this call (mirrors `AtomicBool::swap`).
    pub(crate) fn mark_tray_notice_sent(&self) -> bool {
        self.tray_notice_sent.swap(true, Ordering::SeqCst)
    }
}

pub(crate) fn app_title(app: &AppHandle) -> String {
    app.state::<DesktopState>().app_title()
}

pub(crate) fn loading_title(app: &AppHandle) -> String {
    format!("{} - Loading", app_title(app))
}
