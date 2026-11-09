//! Real browser icons pulled from the OS, cached as PNG/SVG data URLs in `<config>/icons`.

use crate::browsers::Browser;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::collections::HashMap;
use std::path::PathBuf;

fn cache_dir() -> PathBuf {
    crate::config::dir().join("icons")
}

fn key(b: &Browser) -> String {
    // Re-extract when the executable/bundle path changes (update, reinstall).
    let mut h: u64 = 1469598103934665603;
    for c in b.exec.bytes() {
        h = (h ^ c as u64).wrapping_mul(1099511628211);
    }
    format!("{}-{h:x}.txt", crate::browsers::slug(&b.id))
}

/// Map of browser id → `data:` URL. Missing entries mean "use the built-in glyph".
pub fn all(list: &[Browser]) -> HashMap<String, String> {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut out = HashMap::new();
    for b in list {
        let path = dir.join(key(b));
        let url = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                let s = extract(b).unwrap_or_default();
                let _ = std::fs::write(&path, &s); // empty file = known miss
                s
            }
        };
        if !url.is_empty() {
            out.insert(b.id.clone(), url);
        }
    }
    out
}

pub fn clear_cache() {
    let _ = std::fs::remove_dir_all(cache_dir());
}

fn png_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

#[allow(dead_code)]
fn encode_rgba(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().ok()?.write_image_data(rgba).ok()?;
    }
    Some(buf)
}

/* ------------------------------------------------------------- windows */

#[cfg(windows)]
fn extract(b: &Browser) -> Option<String> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    const S: i32 = 64;
    let wide: Vec<u16> = b.exec.encode_utf16().chain([0]).collect();
    unsafe {
        let mut icon: HICON = null_mut();
        let mut id = 0u32;
        let n = PrivateExtractIconsW(wide.as_ptr(), 0, S, S, &mut icon, &mut id, 1, 0);
        if n == 0 || n == u32::MAX || icon.is_null() {
            return None;
        }
        let mut info: ICONINFO = std::mem::zeroed();
        let ok = GetIconInfo(icon, &mut info) != 0;
        let mut px = vec![0u8; (S * S * 4) as usize];
        let mut got = false;
        if ok && !info.hbmColor.is_null() {
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = S;
            bmi.bmiHeader.biHeight = -S; // top-down
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;
            let dc = GetDC(null_mut());
            got = GetDIBits(dc, info.hbmColor, 0, S as u32, px.as_mut_ptr().cast(), &mut bmi, DIB_RGB_COLORS) != 0;
            ReleaseDC(null_mut(), dc);
        }
        if ok {
            if !info.hbmColor.is_null() {
                DeleteObject(info.hbmColor);
            }
            if !info.hbmMask.is_null() {
                DeleteObject(info.hbmMask);
            }
        }
        DestroyIcon(icon);
        if !got {
            return None;
        }
        let has_alpha = px.chunks(4).any(|p| p[3] != 0);
        for p in px.chunks_mut(4) {
            p.swap(0, 2); // BGRA → RGBA
            if !has_alpha {
                p[3] = 255;
            }
        }
        encode_rgba(S as u32, S as u32, &px).map(|b| png_url(&b))
    }
}

/* --------------------------------------------------------------- macos */

#[cfg(target_os = "macos")]
fn extract(b: &Browser) -> Option<String> {
    let app = PathBuf::from(&b.exec);
    let plist = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let mut name = plist.as_dictionary()?.get("CFBundleIconFile")?.as_string()?.to_string();
    if !name.ends_with(".icns") {
        name.push_str(".icns");
    }
    let f = std::fs::File::open(app.join("Contents/Resources").join(name)).ok()?;
    let fam = icns::IconFamily::read(std::io::BufReader::new(f)).ok()?;
    // Smallest icon that is at least 64px, else the largest available.
    let mut types = fam.available_icons();
    types.sort_by_key(|t| t.pixel_width());
    let t = types.iter().find(|t| t.pixel_width() >= 64).or(types.last())?;
    let img = fam.get_icon_with_type(*t).ok()?;
    let mut buf = Vec::new();
    img.write_png(&mut buf).ok()?;
    Some(png_url(&buf))
}

/* --------------------------------------------------------------- linux */

#[cfg(all(unix, not(target_os = "macos")))]
fn extract(b: &Browser) -> Option<String> {
    let name = b.icon.as_deref()?;
    let file = if name.starts_with('/') {
        Some(PathBuf::from(name))
    } else {
        let mut cands = vec![];
        for d in crate::platform::linux_data_dirs() {
            for size in ["64x64", "128x128", "96x96", "256x256", "48x48", "scalable"] {
                for ext in ["png", "svg"] {
                    cands.push(d.join(format!("icons/hicolor/{size}/apps/{name}.{ext}")));
                }
            }
            for ext in ["png", "svg"] {
                cands.push(d.join(format!("pixmaps/{name}.{ext}")));
            }
        }
        cands.into_iter().find(|p| p.is_file())
    }?;
    let bytes = std::fs::read(&file).ok()?;
    let mime = if file.extension().is_some_and(|e| e == "svg") { "image/svg+xml" } else { "image/png" };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}
