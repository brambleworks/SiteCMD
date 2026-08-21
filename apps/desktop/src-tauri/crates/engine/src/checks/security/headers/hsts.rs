//! RFC 6797 Strict-Transport-Security parsing with duplicate-directive checks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HstsPolicy {
    pub(super) max_age: u64,
    pub(super) include_subdomains: bool,
}

fn is_http_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte > 0x20
                && byte < 0x7f
                && !matches!(
                    byte,
                    b'(' | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b':'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                        | b'{'
                        | b'}'
                )
        })
}

fn split_directives(value: &str) -> Result<Vec<&str>, String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parts.push(&value[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if quoted || escaped {
        return Err("the header contains an unterminated quoted value".into());
    }
    parts.push(&value[start..]);
    Ok(parts)
}

fn normalize_directive_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with('"') {
        if !value.ends_with('"') || value.len() < 2 {
            return None;
        }
        let mut normalized = String::new();
        let mut escaped = false;
        for character in value[1..value.len() - 1].chars() {
            if escaped {
                normalized.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' || character == '\r' || character == '\n' {
                return None;
            } else {
                normalized.push(character);
            }
        }
        (!escaped).then_some(normalized)
    } else {
        is_http_token(value).then(|| value.to_string())
    }
}

pub(super) fn parse_hsts_policy(header_value: &str) -> Result<HstsPolicy, String> {
    let mut seen = std::collections::HashSet::new();
    let mut max_age = None;
    let mut include_subdomains = false;

    for raw_directive in split_directives(header_value)? {
        let raw_directive = raw_directive.trim();
        if raw_directive.is_empty() {
            continue;
        }
        let (raw_name, raw_value) = raw_directive
            .split_once('=')
            .map(|(name, value)| (name.trim(), Some(value)))
            .unwrap_or((raw_directive, None));
        if !is_http_token(raw_name) {
            return Err("the header contains an invalid directive name".into());
        }
        let name = raw_name.to_ascii_lowercase();
        if !seen.insert(name.clone()) {
            return Err(format!("the '{}' directive is repeated", raw_name));
        }

        match name.as_str() {
            "max-age" => {
                let value = raw_value
                    .and_then(normalize_directive_value)
                    .filter(|value| {
                        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                    })
                    .ok_or_else(|| {
                        "max-age is missing a valid non-negative integer value".to_string()
                    })?;
                max_age = Some(value.parse::<u64>().unwrap_or(u64::MAX));
            }
            "includesubdomains" => {
                if raw_value.is_some() {
                    return Err("includeSubDomains must not have a value".into());
                }
                include_subdomains = true;
            }
            _ => {
                if raw_value
                    .map(|value| normalize_directive_value(value).is_none())
                    .unwrap_or(false)
                {
                    return Err(format!(
                        "the '{}' extension directive has invalid syntax",
                        raw_name
                    ));
                }
            }
        }
    }

    Ok(HstsPolicy {
        max_age: max_age.ok_or_else(|| "the required max-age directive is missing".to_string())?,
        include_subdomains,
    })
}

#[cfg(test)]
pub(super) fn parse_hsts_max_age(header_value: &str) -> Option<u64> {
    parse_hsts_policy(header_value)
        .ok()
        .map(|policy| policy.max_age)
}
