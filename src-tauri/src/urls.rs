//! URL helpers: rule matching, tracking-param stripping, lookalike detection.

use crate::config::Rule;
use url::Url;

const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "msclkid", "mc_cid", "mc_eid", "igshid", "yclid",
    "_hsenc", "_hsmi", "mkt_tok", "oly_anon_id", "oly_enc_id", "vero_id", "wickedid", "twclid",
    "ttclid", "si", "ref_src",
];

/// Returns (cleaned_url, number_of_params_removed).
///
/// Works on the raw query text and only drops whole `key=value` pieces, so the rest of the
/// query keeps its exact encoding (signed links such as S3 or OAuth URLs stay valid).
pub fn strip_tracking(raw: &str) -> (String, usize) {
    let Ok(mut parsed) = Url::parse(raw) else { return (raw.into(), 0) };
    let Some(query) = parsed.query() else { return (raw.into(), 0) };
    let pieces: Vec<&str> = query.split('&').collect();
    let keep: Vec<&str> = pieces.iter().copied().filter(|piece| !is_tracking_piece(piece)).collect();
    let removed = pieces.len() - keep.len();
    if removed == 0 {
        return (raw.into(), 0);
    }
    let new_query = keep.join("&");
    parsed.set_query(if new_query.is_empty() { None } else { Some(&new_query) });
    (parsed.into(), removed)
}

fn is_tracking_piece(piece: &str) -> bool {
    let key = piece.split('=').next().unwrap_or_default().to_lowercase();
    key.starts_with("utm_") || TRACKING_PARAMS.contains(&key.as_str())
}

/// Simple glob: `*` matches any run of chars. Pattern is matched against
/// `host + path` (no scheme). A bare host pattern also matches all its paths,
/// and `*.example.com` also matches `example.com`.
pub fn glob(pat: &str, text: &str) -> bool {
    fn rec(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    rec(pat.as_bytes(), text.as_bytes())
}

/// A rule pattern is `host[/path]`. The host part is matched against the URL's host only and
/// the path part against its path only, so a `*` in the host can never reach into the path:
/// `*.figma.com` must not match `https://evil.com/x.figma.com/`.
pub fn rule_matches(pattern: &str, raw: &str) -> bool {
    let Ok(parsed) = Url::parse(raw) else { return false };
    let host = parsed.host_str().unwrap_or("").trim_start_matches("www.");
    let pattern = pattern.trim().trim_start_matches("https://").trim_start_matches("http://");
    let (host_pattern, path_pattern) = match pattern.find('/') {
        Some(slash) => (&pattern[..slash], &pattern[slash..]),
        None => (pattern, "/*"),
    };
    if host_pattern.is_empty() {
        return false;
    }
    host_matches(host_pattern, host) && glob(path_pattern, parsed.path())
}

fn host_matches(pattern: &str, host: &str) -> bool {
    // `*.foo.com` means foo.com itself or any subdomain of it.
    if let Some(apex) = pattern.strip_prefix("*.")
        && !apex.contains('*')
    {
        let host = host.to_ascii_lowercase();
        let apex = apex.to_ascii_lowercase();
        return host == apex || host.ends_with(&format!(".{apex}"));
    }
    // Any other wildcard stays inside the host: it may not match across a dot boundary
    // into another domain, but since the path is matched separately it can't reach the path.
    glob(pattern, host)
}

pub fn find_rule<'a>(rules: &'a [Rule], url: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| r.enabled && rule_matches(&r.pattern, url))
}

const BRANDS: &[&str] = &[
    "paypal", "google", "apple", "microsoft", "amazon", "github", "facebook", "instagram",
    "netflix", "linkedin", "dropbox", "outlook", "office", "icloud", "chase", "wellsfargo",
    "bankofamerica", "coinbase", "binance", "steam", "discord", "whatsapp", "twitter",
];

fn deconfuse(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => 'o',
            '1' | '!' | '|' => 'l',
            '3' => 'e',
            '5' => 's',
            '@' => 'a',
            'i' => 'l',
            _ => c,
        })
        .collect::<String>()
        .replace("rn", "m")
        .replace("vv", "w")
}

/// If the URL's registrable-ish label imitates a well-known brand, return that brand.
pub fn lookalike(raw: &str) -> Option<String> {
    let u = Url::parse(raw).ok()?;
    let host = u.host_str()?.to_lowercase();
    if host.starts_with("xn--") || host.contains(".xn--") {
        return Some("an internationalised (punycode) domain".into());
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    // The registrable name: `paypal` in `www.paypal.com`, and also in `paypal.co.uk`.
    let apex_index = if labels.len() >= 3 && is_second_level_suffix(labels[labels.len() - 2]) {
        labels.len() - 3
    } else {
        labels.len() - 2
    };
    if BRANDS.contains(&labels[apex_index]) {
        return None; // the real brand domain
    }
    // Check every label except the TLD, splitting on hyphens so `paypal-secure` is caught.
    // A brand name as a subdomain of some other site (`paypal.evil.com`) is suspicious too.
    for (index, label) in labels[..labels.len() - 1].iter().enumerate() {
        let hyphenated = label.contains('-');
        let above_apex = index < apex_index;
        for part in label.split('-').filter(|p| !p.is_empty()) {
            for brand in BRANDS {
                let disguised = part != *brand && deconfuse(part) == deconfuse(brand);
                let embedded = (hyphenated || above_apex) && part == *brand;
                if disguised || embedded {
                    return Some(format!("{brand}.com"));
                }
            }
        }
    }
    None
}

/// Common second-level registry suffixes (`co.uk`, `com.au`, …). A rough stand-in for the
/// public-suffix list, good enough to find the brand label of a country domain.
fn is_second_level_suffix(label: &str) -> bool {
    matches!(label, "co" | "com" | "org" | "net" | "ac" | "gov" | "edu" | "ne" | "or")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips() {
        let (u, n) = strip_tracking("https://a.com/x?utm_source=t&id=3&fbclid=9");
        assert_eq!((u.as_str(), n), ("https://a.com/x?id=3", 2));
        let (u, n) = strip_tracking("https://a.com/x?utm_source=t");
        assert_eq!((u.as_str(), n), ("https://a.com/x", 1));
        assert_eq!(strip_tracking("https://a.com/?q=1").1, 0);
        // The kept parameters keep their exact encoding.
        let (u, n) = strip_tracking("https://a.com/x?q=a%20b&flag&utm_medium=x&sig=a%2Bb");
        assert_eq!((u.as_str(), n), ("https://a.com/x?q=a%20b&flag&sig=a%2Bb", 1));
    }

    #[test]
    fn rule_wildcards_stay_in_the_host() {
        assert!(!rule_matches("*.figma.com", "https://evil.com/x.figma.com/"));
        assert!(!rule_matches("*.figma.com", "https://evil.com/login.figma.com/file"));
        assert!(!rule_matches("*.figma.com", "https://figma.com.evil.com/"));
        assert!(!rule_matches("figma.com", "https://figma.com@evil.com/"));
        assert!(rule_matches("*.figma.com", "https://a.b.figma.com/x"));
        assert!(rule_matches("*.atlassian.net/wiki/*", "https://acme.atlassian.net/wiki/page"));
        assert!(!rule_matches("*.atlassian.net/wiki/*", "https://acme.atlassian.net/jira"));
    }

    #[test]
    fn rules() {
        assert!(rule_matches("*.figma.com", "https://www.figma.com/file/1"));
        assert!(rule_matches("*.figma.com", "https://figma.com/"));
        assert!(rule_matches("github.com/acme/*", "https://github.com/acme/repo"));
        assert!(!rule_matches("github.com/acme/*", "https://github.com/other"));
        assert!(rule_matches("meet.google.com", "https://meet.google.com/abc"));
        assert!(!rule_matches("google.com", "https://evilgoogle.com/"));
    }

    #[test]
    fn lookalikes() {
        assert_eq!(lookalike("https://paypa1-secure.com/login").as_deref(), Some("paypal.com"));
        assert_eq!(lookalike("https://paypal-secure.com/login").as_deref(), Some("paypal.com"));
        assert_eq!(lookalike("https://www.paypal.com/"), None);
        assert_eq!(lookalike("https://github.com/x"), None);
        assert_eq!(lookalike("https://example.org/"), None);
        assert!(lookalike("https://xn--pypal-4ve.com/").is_some());
        assert_eq!(lookalike("https://paypal.evil.com/").as_deref(), Some("paypal.com"));
        assert_eq!(lookalike("https://paypal.com.evil.net/").as_deref(), Some("paypal.com"));
        assert_eq!(lookalike("https://www.amazon.co.uk/"), None);
        assert_eq!(lookalike("https://mail.google.com/"), None);
    }
}
