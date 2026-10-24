//! Read-only release checks. Download and installation remain manual.
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::Networking::WinHttp::*,
};

pub const RELEASES_URL: &str = "https://github.com/molow159-boop/zaaps-dofus3/releases/latest";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Debug)]
pub enum ResultNotice {
    Available(Release),
    Current,
    Failed,
}
pub enum Request {
    Enabled(bool),
    Check,
}

fn version(text: &str) -> Option<[u32; 3]> {
    let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
    let mut numbers = [0; 3];
    for number in &mut numbers {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *number = part.parse().ok()?;
    }
    parts.next().is_none().then_some(numbers)
}

fn parse_release(body: &[u8], current: &str) -> Result<Option<Release>, String> {
    let json: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| "Réponse GitHub invalide")?;
    if json["draft"].as_bool() != Some(false) || json["prerelease"].as_bool() != Some(false) {
        return Err("La réponse ne décrit pas une version publique stable".into());
    }
    let tag = json["tag_name"].as_str().ok_or("Version manquante")?;
    let remote = version(tag).ok_or("Version invalide")?;
    if remote <= version(current).ok_or("Version locale invalide")? {
        return Ok(None);
    }
    Ok(Some(Release {
        version: tag.trim_start_matches('v').to_owned(),
        url: format!("https://github.com/molow159-boop/zaaps-dofus3/releases/tag/{tag}"),
    }))
}

struct Internet(*mut c_void);
impl Internet {
    fn new(handle: *mut c_void) -> Result<Self, String> {
        if handle.is_null() {
            Err("Connexion GitHub indisponible".into())
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for Internet {
    fn drop(&mut self) {
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

pub fn check() -> Result<Option<Release>, String> {
    unsafe {
        let session = Internet::new(WinHttpOpen(
            w!("Zaaps-Dofus3-Release-Check"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 2000, 3000, 3000, 3000).map_err(|e| e.to_string())?;
        let connection = Internet::new(WinHttpConnect(session.0, w!("api.github.com"), 443, 0))?;
        let request = Internet::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            w!("/repos/molow159-boop/zaaps-dofus3/releases/latest"),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        let redirects = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        WinHttpSetOption(
            Some(request.0.cast()),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&redirects.to_ne_bytes()),
        )
        .map_err(|e| e.to_string())?;
        let headers: Vec<u16> =
            "Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\n"
                .encode_utf16()
                .collect();
        WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0).map_err(|e| e.to_string())?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| e.to_string())?;
        let mut status = 0u32;
        let mut length = 4;
        let mut index = 0;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut length,
            &mut index,
        )
        .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(format!("GitHub HTTP {status}"));
        }
        let mut body = Vec::new();
        let started = Instant::now();
        loop {
            let mut buffer = [0u8; 4096];
            let mut read = 0;
            WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut read,
            )
            .map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            if body.len() + read as usize > 131_072 || started.elapsed() > Duration::from_secs(10) {
                return Err("Réponse GitHub trop volumineuse ou trop lente".into());
            }
            body.extend_from_slice(&buffer[..read as usize]);
        }
        parse_release(&body, env!("CARGO_PKG_VERSION"))
    }
}

#[derive(Default)]
struct Announcements {
    last: Option<String>,
}
impl Announcements {
    fn should_notify(&mut self, enabled: bool, release: &Release) -> bool {
        if !enabled || self.last.as_ref() == Some(&release.version) {
            return false;
        }
        self.last = Some(release.version.clone());
        true
    }
}

pub fn start(
    enabled: bool,
    context: eframe::egui::Context,
    stop: Arc<AtomicBool>,
    tray: Option<crate::tray::Tray>,
) -> (mpsc::Sender<Request>, mpsc::Receiver<ResultNotice>) {
    let (tx, rx) = mpsc::channel();
    let (results, notices) = mpsc::channel();
    thread::Builder::new()
        .name("release-check".into())
        .spawn(move || {
            let mut enabled = enabled;
            let mut due = enabled;
            let mut announcements = Announcements::default();
            while !stop.load(Ordering::Relaxed) {
                if due {
                    let result = match check() {
                        Ok(Some(release)) => ResultNotice::Available(release),
                        Ok(None) => ResultNotice::Current,
                        Err(_) => ResultNotice::Failed,
                    };
                    // Honor preferences changed during the HTTPS request.
                    while let Ok(request) = rx.try_recv() {
                        if let Request::Enabled(value) = request {
                            enabled = value;
                        }
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if let ResultNotice::Available(release) = &result {
                        if announcements.should_notify(enabled, release) {
                            if let Some(tray) = &tray {
                                tray.notify(
                                    "Nouvelle version disponible",
                                    format!(
                                        "Zaaps Dofus 3 {} — ouvrez l'application pour télécharger.",
                                        release.version
                                    ),
                                );
                            }
                        }
                    }
                    let _ = results.send(result);
                    context.request_repaint();
                }
                match rx.recv_timeout(Duration::from_secs(6 * 60 * 60)) {
                    Ok(Request::Enabled(value)) => {
                        due = value && !enabled;
                        enabled = value;
                    }
                    Ok(Request::Check) => due = true,
                    Err(mpsc::RecvTimeoutError::Timeout) => due = enabled,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .expect("impossible de démarrer la vérification de version");
    (tx, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_notifications_respect_disabled_checks_and_do_not_repeat() {
        let mut state = Announcements::default();
        let release = Release {
            version: "1.2.0".into(),
            url: RELEASES_URL.into(),
        };
        assert!(!state.should_notify(false, &release));
        assert!(state.should_notify(true, &release));
        assert!(!state.should_notify(true, &release));
        assert!(state.should_notify(
            true,
            &Release {
                version: "1.3.0".into(),
                ..release
            }
        ));
    }
    #[test]
    #[ignore = "nécessite un accès HTTPS à GitHub"]
    fn native_https_release_check() {
        check().expect("Vérification HTTPS GitHub");
    }
    fn release(tag: &str, prerelease: bool) -> Vec<u8> {
        serde_json::json!({"tag_name":tag,"draft":false,"prerelease":prerelease,
            "html_url":"https://untrusted.invalid/download"})
        .to_string()
        .into_bytes()
    }
    #[test]
    fn only_newer_stable_versions_produce_a_trusted_download_link() {
        let found = parse_release(&release("v1.10.0", false), "1.9.0")
            .unwrap()
            .unwrap();
        assert_eq!(
            found.url,
            "https://github.com/molow159-boop/zaaps-dofus3/releases/tag/v1.10.0"
        );
        assert!(parse_release(&release("v1.0.3", false), "1.1.0")
            .unwrap()
            .is_none());
        assert!(parse_release(&release("v1.1.0", false), "1.1.0")
            .unwrap()
            .is_none());
        for (tag, pre) in [("v1.2.0-beta", true), ("../../file", false), ("1.2", false)] {
            assert!(parse_release(&release(tag, pre), "1.1.0").is_err());
        }
        assert!(parse_release(b"{}", "1.1.0").is_err());
    }
}
