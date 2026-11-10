//! Deep-link handling: map `whatsapp://send?...`, `https://wa.me/...` and
//! `https://api.whatsapp.com/send?...` to a WhatsApp Web chat URL, and pick
//! launch arguments apart. Pure functions only; everything here is unit-tested.
//!
//! The output is always `https://web.whatsapp.com/send?phone=<digits>[&text=<enc>]`;
//! every other component of the input is dropped, so a hostile link can only
//! ever open a chat or prefill a draft on the real WhatsApp Web origin.

const MAX_TEXT_CHARS: usize = 4096;
const WEB_SEND: &str = "https://web.whatsapp.com/send";

/// Map a deep link to a WhatsApp Web URL, or `None` when it is not a link we
/// can translate safely (invite links, unknown hosts, malformed input).
pub fn map_deep_link(link: &str) -> Option<String> {
    // Reject any raw control character or whitespace outright; a legitimate
    // link percent-encodes them.
    if link.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    // The fragment is never meaningful here; cut it before anything else so
    // `...#@evil` style tricks cannot influence parsing.
    let link = link.split('#').next()?;
    let (scheme, rest) = link.split_once(':')?;
    let rest = rest.strip_prefix("//")?;
    let (before_query, query) = match rest.split_once('?') {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };
    let (host, path) = match before_query.split_once('/') {
        Some((h, p)) => (h, p),
        None => (before_query, ""),
    };
    let host = host.to_ascii_lowercase();
    let path = path.strip_suffix('/').unwrap_or(path);

    let mut phone_from_path: Option<&str> = None;
    match (scheme.to_ascii_lowercase().as_str(), host.as_str()) {
        ("whatsapp", "send") if path.is_empty() => {}
        ("https", "api.whatsapp.com") if path == "send" => {}
        ("https", "wa.me") if !path.is_empty() && !path.contains('/') => {
            phone_from_path = Some(path);
        }
        _ => return None,
    }

    let mut phone: Option<String> = None;
    let mut text: Option<String> = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "phone" if phone.is_none() => phone = percent_decode(v, false),
            "text" if text.is_none() => text = percent_decode(v, true),
            _ => {}
        }
    }
    let phone = phone_from_path
        .and_then(|p| percent_decode(p, false))
        .or(phone)?;
    let phone = sanitize_phone(&phone)?;

    let mut out = format!("{WEB_SEND}?phone={phone}");
    if let Some(text) = text {
        let text: String = text
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .take(MAX_TEXT_CHARS)
            .collect();
        if !text.is_empty() {
            out.push_str("&text=");
            out.push_str(&percent_encode(&text));
        }
    }
    Some(out)
}

fn sanitize_phone(raw: &str) -> Option<String> {
    let digits = raw.strip_prefix('+').unwrap_or(raw);
    if (5..=20).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) {
        Some(digits.to_owned())
    } else {
        None
    }
}

/// Percent-decode `input`; `None` on a malformed escape or invalid UTF-8.
/// `plus_is_space` applies form-encoding semantics for `+`.
fn percent_decode(input: &str, plus_is_space: bool) -> Option<String> {
    let b = input.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                let hi = hex_val(*b.get(i + 1)?)?;
                let lo = hex_val(*b.get(i + 2)?)?;
                out.push(hi << 4 | lo);
                i += 3;
            }
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Strict encoding: only RFC 3986 unreserved bytes stay literal.
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Positional (non-flag) launch arguments, skipping `argv[0]` and the value of
/// a bare `--url`.
fn positional<'a>(args: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--url" {
            it.next();
        } else if !a.starts_with('-') {
            out.push(a);
        }
    }
    out
}

/// First positional launch argument that maps to a WhatsApp Web URL.
pub fn from_args<'a>(args: impl IntoIterator<Item = &'a str>) -> Option<String> {
    positional(args).into_iter().find_map(map_deep_link)
}

/// Arguments to hand to Chromium: drop positional ones (deep links) so they
/// are never treated as a URL to open. `argv[0]` and `--url <value>` are kept.
pub fn chromium_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut after_url_flag = false;
    for (i, a) in args.into_iter().enumerate() {
        let keep = i == 0 || after_url_flag || a.starts_with('-');
        after_url_flag = a == "--url";
        if keep {
            out.push(a);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whatsapp_scheme() {
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&text=hello%20world").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567&text=hello%20world")
        );
        assert_eq!(
            map_deep_link("whatsapp://send/?phone=+15551234567").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
        assert_eq!(
            map_deep_link("WhatsApp://SEND?phone=15551234567").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
    }

    #[test]
    fn wa_me_and_api() {
        assert_eq!(
            map_deep_link("https://wa.me/15551234567?text=Hi+there").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567&text=Hi%20there")
        );
        assert_eq!(
            map_deep_link("https://wa.me/+15551234567/").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
        assert_eq!(
            map_deep_link("https://api.whatsapp.com/send?phone=15551234567&text=a%26b").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567&text=a%26b")
        );
    }

    #[test]
    fn extra_params_dropped_and_text_reencoded() {
        let out = map_deep_link(
            "whatsapp://send?phone=15551234567&app_absent=1&text=%3Cb%3E%0Aline%F0%9F%98%80&x=y",
        )
        .unwrap();
        assert_eq!(
            out,
            "https://web.whatsapp.com/send?phone=15551234567&text=%3Cb%3E%0Aline%F0%9F%98%80"
        );
    }

    #[test]
    fn rejects_bad_phone() {
        assert_eq!(map_deep_link("whatsapp://send?text=hi"), None);
        assert_eq!(map_deep_link("whatsapp://send?phone=1234"), None);
        assert_eq!(map_deep_link("whatsapp://send?phone=123456789012345678901"), None);
        assert_eq!(map_deep_link("whatsapp://send?phone=12abc45"), None);
        assert_eq!(map_deep_link("whatsapp://send?phone=%2B1555%2F123"), None);
        assert_eq!(map_deep_link("https://wa.me/abc"), None);
        assert_eq!(map_deep_link("https://wa.me/"), None);
        assert_eq!(map_deep_link("https://wa.me/15551234567/extra"), None);
    }

    #[test]
    fn hostile_inputs() {
        // The fragment is dropped, so only the (too short) phone remains.
        assert_eq!(map_deep_link("whatsapp://send?phone=1&text=x#@evil"), None);
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&text=x#@evil").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567&text=x")
        );
        assert_eq!(map_deep_link("whatsapp://send?phone=15551234567\n&text=x"), None);
        assert_eq!(map_deep_link("whatsapp://send?phone=15551234567&text=a b"), None);
        assert_eq!(map_deep_link("javascript:alert(1)"), None);
        assert_eq!(map_deep_link("javascript://wa.me/15551234567"), None);
        assert_eq!(map_deep_link("whatsapp://evil.com/send?phone=15551234567"), None);
        assert_eq!(map_deep_link("whatsapp://send@evil.com?phone=15551234567"), None);
        assert_eq!(map_deep_link("whatsapp://send//evil.com?phone=15551234567"), None);
        assert_eq!(map_deep_link("https://wa.me.evil.com/15551234567"), None);
        assert_eq!(map_deep_link("https://evil.com/wa.me/15551234567"), None);
        assert_eq!(map_deep_link("http://wa.me/15551234567"), None);
        assert_eq!(map_deep_link("https://wa.me:444/15551234567"), None);
        assert_eq!(map_deep_link("https://user@wa.me/15551234567"), None);
        assert_eq!(map_deep_link("https://chat.whatsapp.com/AbCdEf123"), None);
        assert_eq!(map_deep_link("file:///etc/passwd"), None);
        assert_eq!(map_deep_link("//host/send?phone=15551234567"), None);
        assert_eq!(map_deep_link("whatsapp:send?phone=15551234567"), None);
        assert_eq!(map_deep_link(""), None);
        // Invalid escapes / UTF-8 in text drop the text, not the chat.
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&text=%ZZ").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&text=%FF").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
        // Encoded control characters other than newline are stripped.
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&text=a%00b%07c").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567&text=abc")
        );
        // Encoded delimiters in text stay encoded, never inject parameters.
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&text=x%26phone%3D1%23%2F").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567&text=x%26phone%3D1%23%2F")
        );
        // First occurrence wins.
        assert_eq!(
            map_deep_link("whatsapp://send?phone=15551234567&phone=19998887777").as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
    }

    #[test]
    fn text_is_capped() {
        let link = format!("whatsapp://send?phone=15551234567&text={}", "a".repeat(10_000));
        let out = map_deep_link(&link).unwrap();
        let text = out.split("&text=").nth(1).unwrap();
        assert_eq!(text.len(), MAX_TEXT_CHARS);
    }

    #[test]
    fn args_picking() {
        let a = ["karere", "--debug", "whatsapp://send?phone=15551234567"];
        assert_eq!(
            from_args(a).as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
        // `--url` value is not a deep link; a later positional still is.
        let b = [
            "karere",
            "--url",
            "whatsapp://send?phone=11111111111",
            "https://wa.me/15551234567",
        ];
        assert_eq!(
            from_args(b).as_deref(),
            Some("https://web.whatsapp.com/send?phone=15551234567")
        );
        assert_eq!(from_args(["karere", "--background"]), None);
        assert_eq!(from_args(["karere", "junk"]), None);
        assert_eq!(from_args(Vec::<&str>::new()), None);
    }

    #[test]
    fn chromium_args_drop_positionals() {
        let args = ["karere", "--debug", "whatsapp://send?phone=1", "--url", "x", "foo"]
            .map(String::from);
        assert_eq!(
            chromium_args(args),
            ["karere", "--debug", "--url", "x"].map(String::from)
        );
    }
}
