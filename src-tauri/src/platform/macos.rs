use crate::browsers::{self, Browser};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::{CFURL, CFURLRef};
use std::path::{Path, PathBuf};

const BUNDLE_ID: &str = "com.tengfone.pickmybrowser";
const URL_SCHEMES: [&str; 2] = ["http", "https"];

/// Used by `--uninstall` to drop our LaunchServices registration.
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyApplicationURLsForURL(url: CFURLRef, roles: u32) -> CFArrayRef;
    fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
    fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundle_id: CFStringRef) -> i32;
}
const K_LS_ROLES_ALL: u32 = 0xFFFF_FFFF;

/// Every app LaunchServices says can open an https link, i.e. the installed browsers.
fn apps_handling_https() -> Vec<PathBuf> {
    let url_string = CFString::new("https://example.com");
    let url = unsafe {
        let raw = core_foundation::url::CFURLCreateWithString(
            std::ptr::null(),
            url_string.as_concrete_TypeRef(),
            std::ptr::null(),
        );
        if raw.is_null() {
            return vec![];
        }
        CFURL::wrap_under_create_rule(raw)
    };
    unsafe {
        let raw = LSCopyApplicationURLsForURL(url.as_concrete_TypeRef(), K_LS_ROLES_ALL);
        if raw.is_null() {
            return vec![];
        }
        let apps: CFArray<CFType> = CFArray::wrap_under_create_rule(raw);
        apps.iter()
            .filter_map(|item| item.downcast::<CFURL>())
            .filter_map(|app_url| app_url.to_path())
            .collect()
    }
}

/// Display name and bundle identifier from the app's Info.plist.
fn bundle_info(app: &Path) -> (String, String) {
    let plist = plist::Value::from_file(app.join("Contents/Info.plist")).ok();
    let dict = plist.as_ref().and_then(|p| p.as_dictionary());
    let get = |key: &str| {
        dict.and_then(|d| d.get(key))
            .and_then(|v| v.as_string())
            .map(String::from)
    };
    let name = get("CFBundleDisplayName")
        .or_else(|| get("CFBundleName"))
        .unwrap_or_else(|| app.file_stem().unwrap_or_default().to_string_lossy().into());
    let bundle_id = get("CFBundleIdentifier").unwrap_or_default();
    (name, bundle_id)
}

pub fn detect() -> Vec<Browser> {
    let mut found: Vec<Browser> = vec![];
    for app in apps_handling_https() {
        let (name, bundle_id) = bundle_info(&app);
        let app_path: String = app.to_string_lossy().into();
        if bundle_id == BUNDLE_ID || found.iter().any(|b| b.exec == app_path) {
            continue;
        }
        found.push(browsers::make(&name, app_path, vec![], &bundle_id));
    }
    found
}

pub fn is_default() -> bool {
    let scheme = CFString::new("https");
    unsafe {
        let handler = LSCopyDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef());
        if handler.is_null() {
            return false;
        }
        CFString::wrap_under_create_rule(handler)
            .to_string()
            .eq_ignore_ascii_case(BUNDLE_ID)
    }
}

pub fn set_default() -> Result<(), String> {
    // macOS shows its own confirmation dialog for each scheme.
    let bundle_id = CFString::new(BUNDLE_ID);
    for scheme in URL_SCHEMES {
        let scheme = CFString::new(scheme);
        let status = unsafe {
            LSSetDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef(), bundle_id.as_concrete_TypeRef())
        };
        if status != 0 {
            return Err(format!("LaunchServices error {status}"));
        }
    }
    Ok(())
}

/// The `.app` bundle containing the running executable, if any.
fn own_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"))?;
    Some(bundle.to_path_buf())
}

pub fn unregister() -> Result<(), String> {
    // Drop our LaunchServices registration so we no longer appear as a browser choice.
    // If we were the default, macOS falls back to Safari once the app is gone.
    if let Some(app) = own_app_bundle() {
        let _ = std::process::Command::new(LSREGISTER).arg("-u").arg(app).status();
    }
    Ok(())
}
