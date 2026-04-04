pub(crate) fn coerce_reference(reference: &str, default_scheme: &str) -> String {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Preserve explicit locator schemes.
    if trimmed.contains("://") {
        return trimmed.to_string();
    }

    // Preserve workspace-relative paths.
    if trimmed.contains('/') || trimmed.contains('\\') {
        return trimmed.to_string();
    }

    format!("{default_scheme}://{trimmed}")
}

pub(crate) fn normalize_resource_uri(uri: &str) -> String {
    let trimmed = uri.trim();
    if let Some((scheme, rest)) = trimmed.split_once("://") {
        if rest.is_empty() {
            return trimmed.to_string();
        }
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() {
            return format!("{scheme}://");
        }
        return format!("{scheme}://{rest}");
    }

    trimmed.trim_end_matches('/').to_string()
}
