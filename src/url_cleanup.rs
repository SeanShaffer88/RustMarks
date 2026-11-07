//! Loss-averse cleanup of web URL tracking parameters.
//! Raw parameter order, encoding and fragments are meaningful and must survive.
fn tracking_key(raw: &str) -> bool {
    let key = raw.split_once('=').map(|(key, _)| key).unwrap_or(raw);
    let decoded = url::form_urlencoded::parse(key.as_bytes())
        .next()
        .map(|(key, _)| key.to_lowercase())
        .unwrap_or_default();
    decoded.starts_with("utm_") || ["fbclid", "gclid", "msclkid"].contains(&decoded.as_str())
}
fn signed_query(query: &str) -> bool {
    url::form_urlencoded::parse(query.as_bytes()).any(|(key, _)| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "signature" | "sig" | "x-amz-signature" | "x-goog-signature" | "oauth_signature"
        )
    })
}
/// Return the original string verbatim when there is nothing safe to remove.
pub fn clean(raw: &str) -> String {
    let Ok(parsed) = url::Url::parse(raw) else {
        return raw.to_owned();
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return raw.to_owned();
    }
    let Some(query) = parsed.query() else {
        return raw.to_owned();
    };
    if signed_query(query) {
        return raw.to_owned();
    }
    // Work on the original string, not form-decoded/reserialized values.
    let (without_fragment, fragment) = raw
        .split_once('#')
        .map(|(url, fragment)| (url, Some(fragment)))
        .unwrap_or((raw, None));
    let Some((base, query)) = without_fragment.split_once('?') else {
        return raw.to_owned();
    };
    let parts: Vec<_> = query.split('&').collect();
    let retained: Vec<_> = parts
        .iter()
        .copied()
        .filter(|part| !tracking_key(part))
        .collect();
    if parts.len() == retained.len() {
        return raw.to_owned();
    }
    let mut output = base.to_owned();
    if !retained.is_empty() {
        output.push('?');
        output.push_str(&retained.join("&"));
    }
    if let Some(fragment) = fragment {
        output.push('#');
        output.push_str(fragment);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_encoding_order_duplicate_keys_and_routes() {
        assert_eq!(
            clean("https://example.com/?b=%20&utm_source=x&a=1&a=2#route"),
            "https://example.com/?b=%20&a=1&a=2#route"
        );
        assert_eq!(
            clean("https://example.com/?utm_source=x#route"),
            "https://example.com/#route"
        );
        assert_eq!(
            clean("https://example.com/?%75tm_source=x&Q=%2f+z#top"),
            "https://example.com/?Q=%2f+z#top"
        );
    }
    #[test]
    fn cleanup_leaves_signed_nonweb_and_unmodified_urls_verbatim() {
        for value in [
            "https://EXAMPLE.com:443?b=%20&a=1",
            "https://example.com/?utm_source=x&X-Amz-Signature=123",
            "mailto:user@example.com?utm_source=letter",
            "invalid url?utm_source=x",
        ] {
            assert_eq!(clean(value), value);
        }
    }
}
