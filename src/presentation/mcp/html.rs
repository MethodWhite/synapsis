use serde_json::Value;

pub fn extract_title(html: &str) -> String {
    if let Some(start) = html.find("<title>")
        && let Some(end) = html[start + 7..].find("</title>")
    {
        return html_to_text(&html[start + 7..start + 7 + end]);
    }
    String::new()
}

pub fn extract_meta(html: &str, name: &str) -> String {
    let patterns = [
        format!("<meta name=\"{}\" content=\"", name),
        format!("<meta property=\"{}\" content=\"", name),
        format!("<meta name='{}' content='", name),
        format!("<meta property='{}' content='", name),
    ];
    for pat in &patterns {
        if let Some(start) = html.find(pat.as_str()) {
            let content_start = start + pat.len();
            if let Some(end) = html[content_start..].find(&['"', '\''][..]) {
                return html_to_text(&html[content_start..content_start + end]);
            }
        }
    }
    String::new()
}

pub fn extract_links(html: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut pos = 0;
    while let Some(start) = html[pos..].find("<a ") {
        let link_start = pos + start;
        let href = try_extract_href(html, link_start);
        if let Some(href) = href {
            if !href.starts_with('#') && !href.starts_with("javascript:") {
                links.push(href);
            }
            pos = link_start + 3;
        } else {
            pos = link_start + 3;
        }
    }
    links
}

fn try_extract_href(html: &str, start: usize) -> Option<String> {
    let patterns = [("href=\"", '"'), ("href='", '\''), ("href=", ' ')];
    for (prefix, delimiter) in &patterns {
        if let Some(href_start) = html[start..].find(prefix) {
            let val_start = start + href_start + prefix.len();
            if *delimiter == ' ' {
                let remaining = &html[val_start..];
                let end = remaining.find([' ', '>']).unwrap_or(remaining.len());
                return Some(remaining[..end].to_string());
            }
            if let Some(href_end) = html[val_start..].find(*delimiter) {
                return Some(html[val_start..val_start + href_end].to_string());
            }
        }
    }
    None
}

pub fn count_links(html: &str) -> usize {
    let mut count = 0;
    let mut pos = 0;
    while let Some(start) = html[pos..].find("<a ") {
        count += 1;
        pos += start + 3;
    }
    count
}

pub fn extract_headings(html: &str) -> String {
    let mut headings = Vec::new();
    for tag in &["h1", "h2", "h3"] {
        let mut pos = 0;
        while let Some(start) = html[pos..].find(&format!("<{}", tag)) {
            let content_start = pos + start;
            if let Some(close) = html[content_start..].find('>') {
                let text_start = content_start + close + 1;
                if let Some(end) = html[text_start..].find(&format!("</{}>", tag)) {
                    let text = html_to_text(&html[text_start..text_start + end]);
                    if !text.is_empty() {
                        headings.push(format!("<{}>{}", tag, text));
                    }
                    pos = text_start + end;
                    continue;
                }
            }
            pos = content_start + 2;
        }
    }
    headings.join("\n")
}

pub fn strip_html(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if in_script {
            if c == '<' {
                let mut tag_end = String::new();
                for ch in chars.by_ref() {
                    tag_end.push(ch);
                    if ch == '>' {
                        break;
                    }
                }
                if tag_end.to_lowercase().starts_with("/script>") {
                    in_script = false;
                }
            }
            continue;
        }
        if in_style {
            if c == '<' {
                let mut tag_end = String::new();
                for ch in chars.by_ref() {
                    tag_end.push(ch);
                    if ch == '>' {
                        break;
                    }
                }
                if tag_end.to_lowercase().starts_with("/style>") {
                    in_style = false;
                }
            }
            continue;
        }
        if c == '<' {
            let mut tag_name = String::new();
            let mut rest = String::new();
            for ch in chars.by_ref() {
                if ch == '>' || ch == ' ' {
                    if ch == '>' {
                        break;
                    }
                    rest.push(ch);
                    break;
                }
                tag_name.push(ch);
            }
            let lower = tag_name.to_lowercase();
            if lower == "script" {
                in_script = true;
                continue;
            }
            if lower == "style" {
                in_style = true;
                continue;
            }
            if lower == "br"
                || lower == "p"
                || lower == "/p"
                || lower == "div"
                || lower == "/div"
                || lower == "tr"
                || lower == "/tr"
                || lower == "li"
            {
                text.push('\n');
            }
            in_tag = true;
            continue;
        }
        if c == '>' && in_tag {
            in_tag = false;
            continue;
        }
        if !in_tag {
            text.push(c);
        }
    }

    text
}

fn html_to_text(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

pub fn format_size2(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    if bytes == 0 {
        return "0B".into();
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:.1}{}", size, UNITS[unit])
}

pub fn derive_encryption_key() -> [u8; 32] {
    if let Ok(hex_key) = std::env::var("SYNAPSIS_DB_KEY")
        && let Ok(decoded) = hex::decode(hex_key)
        && decoded.len() >= 32
    {
        let mut key = [0u8; 32];
        key.copy_from_slice(&decoded[..32]);
        return key;
    }
    let key_path = crate::config::data_dir().join(".browser_encryption_key");
    if let Ok(data) = std::fs::read(&key_path)
        && data.len() == 32
    {
        let mut key_vec = data.clone();
        key_vec.truncate(32);
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_vec);
        return key;
    }
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).expect("getrandom failed");
    if let Some(parent) = key_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&key_path, key);
    key
}

pub fn format_args_snapshot(tool: &str, args: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(obj) = args.as_object() {
        for (key, val) in obj.iter().take(4) {
            let v = match val {
                Value::String(s) => {
                    if s.len() > 80 {
                        let truncated: String = s.chars().take(77).collect();
                        format!("{}...", truncated)
                    } else {
                        s.clone()
                    }
                }
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Array(a) => format!("[{} items]", a.len()),
                Value::Object(o) => format!(
                    "{{{}}}",
                    o.keys().take(3).cloned().collect::<Vec<_>>().join(",")
                ),
                _ => "?".to_string(),
            };
            parts.push(format!("{}={}", key, v));
        }
    }
    if parts.len() > 3 {
        parts.truncate(3);
        parts.push("...".into());
    }
    let args_str = if parts.is_empty() {
        "()".to_string()
    } else {
        parts.join(" ")
    };
    format!("[{}] {}", tool, args_str)
}
