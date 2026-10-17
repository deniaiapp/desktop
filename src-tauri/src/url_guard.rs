use tauri::webview::Url;

pub(crate) const APP_ORIGIN: &str = "https://deniai.app";
pub(crate) const APP_START_URL: &str = "https://deniai.app/chat";

pub(crate) fn default_app_url() -> Url {
    APP_START_URL
        .parse()
        .expect("default app URL must be valid")
}

/// URL for a page served by one of the app's own custom URI schemes.
pub(crate) fn local_scheme_url(scheme: &str, path: &str) -> Url {
    // Windows and Android serve custom schemes from http://<scheme>.localhost.
    let url = if cfg!(any(target_os = "windows", target_os = "android")) {
        format!("http://{scheme}.localhost{path}")
    } else {
        format!("{scheme}://localhost{path}")
    };
    url.parse().expect("local scheme URL must be valid")
}

/// Whether `url` points at the app's own custom URI scheme `scheme`.
pub(crate) fn is_local_scheme_origin(url: &Url, scheme: &str) -> bool {
    let local = local_scheme_url(scheme, "/");
    url.scheme() == local.scheme() && url.host_str() == local.host_str()
}

/// URLs that belong to the app itself and should load inside the main window.
pub(crate) fn is_in_app_url(url: &Url) -> bool {
    if url.origin().ascii_serialization() != APP_ORIGIN {
        return false;
    }

    matches!(url.path(), "/" | "/home" | "/chat" | "/auth/sign-in")
        || url.path().starts_with("/home/")
        || url.path().starts_with("/chat/")
        || url.path().starts_with("/auth/sign-in/")
}

/// URLs involved in the Google sign-in flow that the app itself drives
/// (callback landing page, or the accounts.google.com / googleusercontent.com
/// pages the OAuth flow redirects through).
pub(crate) fn is_app_managed_auth_url(url: &Url) -> bool {
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

pub(crate) fn is_allowed_url(url: &Url) -> bool {
    is_in_app_url(url) || is_app_managed_auth_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_redirect_stays_in_app() {
        for url in [
            "https://deniai.app/home",
            "https://deniai.app/home/features",
        ] {
            assert!(is_allowed_url(&url.parse().unwrap()));
        }
    }

    #[test]
    fn unrelated_routes_and_origins_still_open_externally() {
        for url in [
            "https://deniai.app/pricing",
            "https://deniai.app/homepage",
            "https://example.com/home",
        ] {
            assert!(!is_allowed_url(&url.parse().unwrap()));
        }
    }
}
