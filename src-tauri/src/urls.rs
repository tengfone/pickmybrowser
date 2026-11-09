//! URL helpers: rule matching, tracking-param stripping, lookalike detection.

use crate::config::Rule;
use url::Url;

const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "msclkid", "mc_cid", "mc_eid", "igshid", "yclid",
    "_hsenc", "_hsmi", "mkt_tok", "oly_anon_id", "oly_enc_id", "vero_id", "wickedid", "twclid",
    "ttclid", "si", "ref_src",
];

/// Returns (cleaned_url, number_of_params_removed).
pub fn strip_tracking(raw: &str) -> (String, usize) {
    let Ok(mut parsed) = Url::parse(raw) else { return (raw.into(), 0) };
    let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
    if pairs.is_empty() {
        return (raw.into(), 0);
    }
    let keep: Vec<_> = pairs.iter().filter(|(key, _)| !is_tracking_param(key)).collect();
    let removed = pairs.len() - keep.len();
    if removed == 0 {
        return (raw.into(), 0);
    }
    if keep.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(keep);
    }
    (parsed.into(), removed)
}

fn is_tracking_param(key: &str) -> bool {
    let key = key.to_lowercase();
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

pub fn rule_matches(pattern: &str, raw: &str) -> bool {
    let Ok(parsed) = Url::parse(raw) else { return false };
    let host = parsed.host_str().unwrap_or("").trim_start_matches("www.");
    let target = format!("{host}{}", parsed.path());
    let mut pattern = pattern.trim().trim_start_matches("https://").trim_start_matches("http://").to_string();
    if pattern.is_empty() {
        return false;
    }
    if !pattern.contains('/') {
        pattern.push_str("/*");
    }
    if glob(&pattern, &target) {
        return true;
    }
    // `*.foo.com` should also match apex `foo.com`.
    match pattern.strip_prefix("*.") {
        Some(apex_pattern) => glob(apex_pattern, &target),
        None => false,
    }
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
    let apex = labels[labels.len() - 2];
    if BRANDS.contains(&apex) {
        return None; // the real brand domain
    }
    // Check every label except the TLD, splitting on hyphens so `paypal-secure` is caught.
    for label in &labels[..labels.len() - 1] {
        let hyphenated = label.contains('-');
        for part in label.split('-').filter(|p| !p.is_empty()) {
            for brand in BRANDS {
                let disguised = part != *brand && deconfuse(part) == deconfuse(brand);
                let embedded = hyphenated && part == *brand;
                if disguised || embedded {
                    return Some(format!("{brand}.com"));
                }
            }
        }
    }
    None
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
    }
}
