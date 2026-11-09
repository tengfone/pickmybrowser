use crate::browsers::{self, Browser};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::{CFURL, CFURLRef};
use std::path::PathBuf;

const BUNDLE_ID: &str = "com.tengfone.pickmybrowser";

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyApplicationURLsForURL(url: CFURLRef, roles: u32) -> CFArrayRef;
    fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
    fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundle_id: CFStringRef) -> i32;
}
const K_LS_ROLES_ALL: u32 = 0xFFFF_FFFF;

fn app_urls_for_https() -> Vec<PathBuf> {
    let s = CFString::new("https://example.com");
    let url = unsafe {
        let u = core_foundation::url::CFURLCreateWithString(std::ptr::null(), s.as_concrete_TypeRef(), std::ptr::null());
        if u.is_null() {
            return vec![];
        }
        CFURL::wrap_under_create_rule(u)
    };
    unsafe {
        let arr = LSCopyApplicationURLsForURL(url.as_concrete_TypeRef(), K_LS_ROLES_ALL);
        if arr.is_null() {
            return vec![];
        }
        let arr: CFArray<CFType> = CFArray::wrap_under_create_rule(arr);
        arr.iter()
            .filter_map(|v| v.downcast::<CFURL>())
            .filter_map(|u| u.to_path())
            .collect()
    }
}

fn bundle_info(app: &PathBuf) -> (String, String) {
    let plist = plist::Value::from_file(app.join("Contents/Info.plist")).ok();
    let dict = plist.as_ref().and_then(|p| p.as_dictionary());
    let get = |k: &str| dict.and_then(|d| d.get(k)).and_then(|v| v.as_string()).map(String::from);
    let name = get("CFBundleDisplayName")
        .or_else(|| get("CFBundleName"))
        .unwrap_or_else(|| app.file_stem().unwrap_or_default().to_string_lossy().into());
    (name, get("CFBundleIdentifier").unwrap_or_default())
}

pub fn detect() -> Vec<Browser> {
    let mut out: Vec<Browser> = vec![];
    for app in app_urls_for_https() {
        let (name, bid) = bundle_info(&app);
        if bid == BUNDLE_ID || out.iter().any(|b| b.exec == app.to_string_lossy()) {
            continue;
        }
        out.push(browsers::make(&name, app.to_string_lossy().into(), vec![], &bid));
    }
    out
}

pub fn is_default() -> bool {
    unsafe {
        let r = LSCopyDefaultHandlerForURLScheme(CFString::new("https").as_concrete_TypeRef());
        if r.is_null() {
            return false;
        }
        CFString::wrap_under_create_rule(r).to_string().eq_ignore_ascii_case(BUNDLE_ID)
    }
}

pub fn set_default() -> Result<(), String> {
    // macOS shows its own confirmation dialog.
    let id = CFString::new(BUNDLE_ID);
    for s in ["http", "https"] {
        let rc = unsafe { LSSetDefaultHandlerForURLScheme(CFString::new(s).as_concrete_TypeRef(), id.as_concrete_TypeRef()) };
        if rc != 0 {
            return Err(format!("LaunchServices error {rc}"));
        }
    }
    Ok(())
}

pub fn unregister() -> Result<(), String> {
    // Drop our LaunchServices registration so we no longer appear as a browser choice.
    // If we were the default, macOS falls back to Safari once the app is gone.
    const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    if let Some(app) = std::env::current_exe().ok().and_then(|e| e.ancestors().find(|p| p.extension().is_some_and(|x| x == "app")).map(|p| p.to_path_buf())) {
        let _ = std::process::Command::new(LSREGISTER).arg("-u").arg(app).status();
    }
    Ok(())
}
