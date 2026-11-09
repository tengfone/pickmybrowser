use crate::browsers::{self, Browser};
use std::path::Path;
use winreg::RegKey;
use winreg::enums::*;

const APP_NAME: &str = "PickMyBrowser";
const PROG_ID: &str = "PickMyBrowserURL";
const URL_SCHEMES: [&str; 2] = ["http", "https"];
const HTML_EXTENSIONS: [&str; 2] = [".htm", ".html"];

/// Where browsers advertise themselves to Windows (the "Default apps" list).
/// The WOW6432Node variant holds registrations made by 32-bit installers.
const START_MENU_INTERNET_KEYS: [&str; 2] = [
    r"Software\Clients\StartMenuInternet",
    r"Software\WOW6432Node\Clients\StartMenuInternet",
];

fn io_err(e: std::io::Error) -> String {
    e.to_string()
}

/// Extract the exe path from a registry command line like `"C:\x\chrome.exe" --foo`.
fn exe_from_command(command: &str) -> Option<String> {
    let command = command.trim();
    if let Some(after_quote) = command.strip_prefix('"') {
        return Some(after_quote.split('"').next()?.to_string());
    }
    // Unquoted: take everything up to and including ".exe".
    let end = command.to_lowercase().find(".exe")? + ".exe".len();
    Some(command[..end].to_string())
}

pub fn detect() -> Vec<Browser> {
    let mut found: Vec<Browser> = vec![];
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        for path in START_MENU_INTERNET_KEYS {
            let Ok(clients) = root.open_subkey(path) else { continue };
            for key_name in clients.enum_keys().flatten() {
                let Some((display_name, exe)) = read_client(&clients, &key_name) else { continue };
                // The same browser is often registered in several hives.
                if found.iter().any(|b| b.exec.eq_ignore_ascii_case(&exe)) {
                    continue;
                }
                found.push(browsers::make(&display_name, exe, vec![], &key_name));
            }
        }
    }
    found
}

/// Read one `StartMenuInternet\<key_name>` entry; returns its display name and an
/// exe path that exists on disk.
fn read_client(clients: &RegKey, key_name: &str) -> Option<(String, String)> {
    let client = clients.open_subkey(key_name).ok()?;
    let display_name: String = client.get_value("").unwrap_or_else(|_| key_name.to_string());
    let command: String = client.open_subkey(r"shell\open\command").ok()?.get_value("").ok()?;
    let exe = exe_from_command(&command)?;
    if !Path::new(&exe).exists() {
        return None;
    }
    Some((display_name, exe))
}

pub fn is_default() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    URL_SCHEMES.iter().all(|scheme| {
        let user_choice = format!(
            r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{scheme}\UserChoice"
        );
        hkcu.open_subkey(user_choice)
            .and_then(|key| key.get_value::<String, _>("ProgId"))
            .map(|prog_id| prog_id == PROG_ID)
            .unwrap_or(false)
    })
}

/// Write the per-user registry entries that make Windows list us as a browser.
fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(io_err)?;
    let exe = exe.to_string_lossy();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    register_prog_id(&hkcu, &exe).map_err(io_err)?;
    register_client(&hkcu, &exe).map_err(io_err)
}

/// The ProgID is what URL and file associations point at: it says how to open a link.
fn register_prog_id(hkcu: &RegKey, exe: &str) -> std::io::Result<()> {
    let (prog_id, _) = hkcu.create_subkey(format!(r"Software\Classes\{PROG_ID}"))?;
    prog_id.set_value("", &"PickMyBrowser URL")?;
    prog_id.set_value("URL Protocol", &"")?;
    let (icon, _) = prog_id.create_subkey("DefaultIcon")?;
    icon.set_value("", &format!("\"{exe}\",0"))?;
    let (command, _) = prog_id.create_subkey(r"shell\open\command")?;
    command.set_value("", &format!("\"{exe}\" \"%1\""))?;
    Ok(())
}

/// The StartMenuInternet client plus its Capabilities, referenced from
/// RegisteredApplications, is what makes us show up in Settings > Default apps.
fn register_client(hkcu: &RegKey, exe: &str) -> std::io::Result<()> {
    let client_path = format!(r"Software\Clients\StartMenuInternet\{APP_NAME}");
    let (client, _) = hkcu.create_subkey(&client_path)?;
    client.set_value("", &APP_NAME)?;
    let (command, _) = client.create_subkey(r"shell\open\command")?;
    command.set_value("", &format!("\"{exe}\""))?;

    let (capabilities, _) = client.create_subkey("Capabilities")?;
    capabilities.set_value("ApplicationName", &APP_NAME)?;
    capabilities.set_value("ApplicationDescription", &"Pick which browser opens each link")?;
    capabilities.set_value("ApplicationIcon", &format!("\"{exe}\",0"))?;
    let (url_associations, _) = capabilities.create_subkey("URLAssociations")?;
    for scheme in URL_SCHEMES {
        url_associations.set_value(scheme, &PROG_ID)?;
    }
    let (file_associations, _) = capabilities.create_subkey("FileAssociations")?;
    for ext in HTML_EXTENSIONS {
        file_associations.set_value(ext, &PROG_ID)?;
    }

    let (registered_apps, _) = hkcu.create_subkey(r"Software\RegisteredApplications")?;
    registered_apps.set_value(APP_NAME, &format!(r"{client_path}\Capabilities"))?;
    Ok(())
}

pub fn set_default() -> Result<(), String> {
    register()?;
    // Windows 10/11 don't let apps set the default themselves; the user must confirm in Settings.
    let settings_uri = format!("ms-settings:defaultapps?registeredAppUser={APP_NAME}");
    // rundll32 instead of `cmd /C start`: no console window flashes up.
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &settings_uri])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(io_err)
}

/// Best effort: every step ignores errors so one missing key doesn't stop the rest.
pub fn unregister() -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{PROG_ID}"));
    let _ = hkcu.delete_subkey_all(format!(r"Software\Clients\StartMenuInternet\{APP_NAME}"));
    if let Ok(key) = hkcu.open_subkey_with_flags(r"Software\RegisteredApplications", KEY_WRITE) {
        let _ = key.delete_value(APP_NAME);
    }

    // Bookkeeping Windows itself writes once we were offered as a handler.
    let toasts_path = r"Software\Microsoft\Windows\CurrentVersion\ApplicationAssociationToasts";
    if let Ok(toasts) = hkcu.open_subkey_with_flags(toasts_path, KEY_READ | KEY_WRITE) {
        let ours: Vec<String> = toasts
            .enum_values()
            .filter_map(|value| value.ok())
            .map(|(name, _)| name)
            .filter(|name| name.starts_with(PROG_ID))
            .collect();
        for name in ours {
            let _ = toasts.delete_value(name);
        }
    }

    for ext in HTML_EXTENSIONS {
        let open_with = format!(r"Software\Classes\{ext}\OpenWithProgids");
        if let Ok(key) = hkcu.open_subkey_with_flags(open_with, KEY_WRITE) {
            let _ = key.delete_value(PROG_ID);
        }
    }

    // The UserChoice keys are hash-protected; with our ProgID gone Windows falls back
    // to its own default and asks the user, which is the intended behaviour.
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
