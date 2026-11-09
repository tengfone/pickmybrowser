use crate::browsers::{self, Browser};
use winreg::RegKey;
use winreg::enums::*;

const APP: &str = "PickMyBrowser";
const PROG_ID: &str = "PickMyBrowserURL";

/// Split a registry command line like `"C:\x\chrome.exe" --foo` into the exe path.
fn exe_from_command(cmd: &str) -> Option<String> {
    let c = cmd.trim();
    let exe = if let Some(rest) = c.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        // Unquoted: take up to ".exe".
        let lower = c.to_lowercase();
        let end = lower.find(".exe")? + 4;
        c[..end].to_string()
    };
    Some(exe)
}

pub fn detect() -> Vec<Browser> {
    let mut out: Vec<Browser> = vec![];
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        for path in [
            r"Software\Clients\StartMenuInternet",
            r"Software\WOW6432Node\Clients\StartMenuInternet",
        ] {
            let Ok(key) = root.open_subkey(path) else { continue };
            for name in key.enum_keys().flatten() {
                let Ok(sub) = key.open_subkey(&name) else { continue };
                let display: String = sub.get_value("").unwrap_or_else(|_| name.clone());
                let Ok(cmd) = sub
                    .open_subkey(r"shell\open\command")
                    .and_then(|k| k.get_value::<String, _>(""))
                else {
                    continue;
                };
                let Some(exe) = exe_from_command(&cmd) else { continue };
                if !std::path::Path::new(&exe).exists() {
                    continue;
                }
                if out.iter().any(|b| b.exec.eq_ignore_ascii_case(&exe)) {
                    continue;
                }
                out.push(browsers::make(&display, exe, vec![], &name));
            }
        }
    }
    out
}

pub fn is_default() -> bool {
    let hk = RegKey::predef(HKEY_CURRENT_USER);
    ["http", "https"].iter().all(|s| {
        hk.open_subkey(format!(
            r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{s}\UserChoice"
        ))
        .and_then(|k| k.get_value::<String, _>("ProgId"))
        .map(|p| p == PROG_ID)
        .unwrap_or(false)
    })
}

fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy();
    let hk = RegKey::predef(HKEY_CURRENT_USER);
    let e = |e: std::io::Error| e.to_string();

    // ProgID
    let (pid, _) = hk.create_subkey(format!(r"Software\Classes\{PROG_ID}")).map_err(e)?;
    pid.set_value("", &"PickMyBrowser URL").map_err(e)?;
    pid.set_value("URL Protocol", &"").map_err(e)?;
    let (icon, _) = pid.create_subkey("DefaultIcon").map_err(e)?;
    icon.set_value("", &format!("\"{exe}\",0")).map_err(e)?;
    let (cmd, _) = pid.create_subkey(r"shell\open\command").map_err(e)?;
    cmd.set_value("", &format!("\"{exe}\" \"%1\"")).map_err(e)?;

    // StartMenuInternet + Capabilities
    let base = format!(r"Software\Clients\StartMenuInternet\{APP}");
    let (app, _) = hk.create_subkey(&base).map_err(e)?;
    app.set_value("", &APP).map_err(e)?;
    let (cmd, _) = app.create_subkey(r"shell\open\command").map_err(e)?;
    cmd.set_value("", &format!("\"{exe}\"")).map_err(e)?;
    let (caps, _) = app.create_subkey("Capabilities").map_err(e)?;
    caps.set_value("ApplicationName", &APP).map_err(e)?;
    caps.set_value("ApplicationDescription", &"Pick which browser opens each link").map_err(e)?;
    caps.set_value("ApplicationIcon", &format!("\"{exe}\",0")).map_err(e)?;
    let (urls, _) = caps.create_subkey("URLAssociations").map_err(e)?;
    for s in ["http", "https"] {
        urls.set_value(s, &PROG_ID).map_err(e)?;
    }
    let (assoc, _) = caps.create_subkey("FileAssociations").map_err(e)?;
    for ext in [".htm", ".html"] {
        assoc.set_value(ext, &PROG_ID).map_err(e)?;
    }
    let (reg, _) = hk.create_subkey(r"Software\RegisteredApplications").map_err(e)?;
    reg.set_value(APP, &format!(r"{base}\Capabilities")).map_err(e)?;
    Ok(())
}

pub fn set_default() -> Result<(), String> {
    register()?;
    // Windows 10/11 require the user to confirm in Settings.
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &format!("ms-settings:defaultapps?registeredAppUser={APP}")])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn unregister() -> Result<(), String> {
    let hk = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hk.delete_subkey_all(format!(r"Software\Classes\{PROG_ID}"));
    let _ = hk.delete_subkey_all(format!(r"Software\Clients\StartMenuInternet\{APP}"));
    if let Ok(k) = hk.open_subkey_with_flags(r"Software\RegisteredApplications", KEY_WRITE) {
        let _ = k.delete_value(APP);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::exe_from_command;
    #[test]
    fn splits_commands() {
        assert_eq!(exe_from_command(r#""C:\a b\chrome.exe" --x"#).unwrap(), r"C:\a b\chrome.exe");
        assert_eq!(exe_from_command(r"C:\x\firefox.exe -osint").unwrap(), r"C:\x\firefox.exe");
    }
}
