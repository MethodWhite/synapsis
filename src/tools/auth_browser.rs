//! Authenticated browser tool for Synapsis
//! Enables login-protected web scraping with session/cookie persistence.
//! Requires Chrome/Chromium and the `browser` feature flag.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

#[cfg(feature = "browser")]
use headless_chrome::protocol::page::ScreenshotFormat;
#[cfg(feature = "browser")]
use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};

/// Persistent browser session that maintains cookies/state across requests
#[cfg(feature = "browser")]
pub struct AuthSession {
    browser: Browser,
    tab: Arc<Tab>,
    logged_in: bool,
    login_url: String,
}

use std::sync::Arc;

/// Global session store (for MCP tool stateless calls, we persist cookies to disk)
static COOKIE_STORE: Mutex<Option<String>> = Mutex::new(None);

/// Save cookies from current tab to disk for session persistence
#[cfg(feature = "browser")]
fn save_cookies_to_file(tab: &Tab, session_id: &str) -> Result<()> {
    let cookie_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".synapsis")
        .join("browser_sessions");
    fs::create_dir_all(&cookie_dir)?;

    let cookie_file = cookie_dir.join(format!("{}.json", session_id));

    // Extract cookies via JavaScript
    let js = "document.cookie";
    let result = tab.evaluate(js, false)
        .map_err(|e| anyhow!("JS eval failed: {}", e))?;
    let cookie_str = match result.value {
        Some(Value::String(s)) => s,
        _ => String::new(),
    };

    // Get cookies via Chrome DevTools protocol
    let all_cookies = tab.get_cookies()
        .map(|cookies| {
            cookies.iter().map(|c| {
                json!({
                    "name": c.name,
                    "value": c.value,
                    "domain": c.domain,
                    "path": c.path,
                    "secure": c.secure,
                    "http_only": c.http_only
                })
            }).collect::<Vec<_>>()
        })
        .map_err(|e| anyhow!("get_cookies failed: {}", e))
        .unwrap_or_default();

    let session_data = json!({
        "session_id": session_id,
        "document_cookie": cookie_str,
        "devtools_cookies": all_cookies,
        "created_at": chrono::Utc::now().to_rfc3339()
    });

    let pretty = serde_json::to_string_pretty(&session_data)?;
    fs::write(&cookie_file, pretty)?;

    Ok(())
}

/// Load cookies from disk and apply to tab
#[cfg(feature = "browser")]
fn load_cookies_from_file(tab: &Tab, session_id: &str) -> Result<()> {
    let cookie_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".synapsis")
        .join("browser_sessions");
    let cookie_file = cookie_dir.join(format!("{}.json", session_id));

    if !cookie_file.exists() {
        return Err(anyhow!("No saved session found for: {}", session_id));
    }

    let data = fs::read_to_string(&cookie_file)?;
    let session_data: Value = serde_json::from_str(&data)?;

    // Set document cookies via JavaScript
    if let Some(doc_cookie) = session_data.get("document_cookie").and_then(|v| v.as_str()) {
        if !doc_cookie.is_empty() {
            // Split cookies and set each one
            for cookie_pair in doc_cookie.split(';').map(|s| s.trim()) {
                if !cookie_pair.is_empty() {
                    let js = format!("document.cookie = '{}';", cookie_pair.replace("'", "\\'"));
                    let _ = tab.evaluate(&js, false);
                }
            }
        }
    }

    // Set cookies via DevTools protocol (headless_chrome doesn't expose set_cookie on Tab)
    // We rely on document.cookie JavaScript method which works for same-origin cookies
    if let Some(cookies) = session_data.get("devtools_cookies").and_then(|v| v.as_array()) {
        for cookie in cookies {
            if let (Some(name), Some(value), Some(domain)) = (
                cookie.get("name").and_then(|v| v.as_str()),
                cookie.get("value").and_then(|v| v.as_str()),
                cookie.get("domain").and_then(|v| v.as_str()),
            ) {
                // Use JavaScript to set cookie for the current domain
                let js = format!(
                    "document.cookie = '{}={}; domain={}; path=/';",
                    name.replace("'", "\\'"),
                    value.replace("'", "\\'"),
                    domain
                );
                let _ = tab.evaluate(&js, false);
            }
        }
    }

    Ok(())
}

/// Navigate to URL with authentication, performing login if needed
pub fn auth_navigate(
    url: &str,
    session_id: &str,
    login_url: Option<&str>,
    login_selector_user: Option<&str>,
    login_selector_pass: Option<&str>,
    username: Option<&str>,
    password: Option<&str>,
    login_button_selector: Option<&str>,
) -> Result<serde_json::Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled. Build with --features browser"));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;

        let browser =
            Browser::new(launch_options).map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))?;

        let tab = Arc::new(tab);

        // Try to load existing session
        let session_loaded = load_cookies_from_file(&tab, session_id).is_ok();

        if session_loaded {
            // Navigate to target URL with existing cookies
            tab.navigate_to(url)
                .map_err(|e| anyhow!("Navigation failed: {}", e))?;
            tab.wait_until_navigated()
                .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
            std::thread::sleep(Duration::from_secs(2));

            // Check if we're still logged in by verifying page content
            let html = get_html(&tab)?;
            if html.contains("login") || html.contains("sign in") || html.contains("auth") {
                // Session expired, need to re-login
            } else {
                save_cookies_to_file(&tab, session_id)?;
                return Ok(json!({
                    "status": "ok",
                    "session_restored": true,
                    "url": url,
                    "html_length": html.len()
                }));
            }
        }

        // Perform login if credentials provided
        if let (Some(l_url), Some(user), Some(pass)) =
            (login_url, username, password)
        {
            tab.navigate_to(l_url)
                .map_err(|e| anyhow!("Login page navigation failed: {}", e))?;
            tab.wait_until_navigated()
                .map_err(|e| anyhow!("Login navigation timeout: {}", e))?;
            std::thread::sleep(Duration::from_secs(2));

            // Fill username
            if let Some(sel) = login_selector_user {
                let js = format!(
                    "const el = document.querySelector('{}'); if(el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}",
                    sel.replace("'", "\\'"),
                    user.replace("'", "\\'")
                );
                tab.evaluate(&js, false)
                    .map_err(|e| anyhow!("Failed to fill username: {}", e))?;
            }

            // Fill password
            if let Some(sel) = login_selector_pass {
                let js = format!(
                    "const el = document.querySelector('{}'); if(el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}",
                    sel.replace("'", "\\'"),
                    pass.replace("'", "\\'")
                );
                tab.evaluate(&js, false)
                    .map_err(|e| anyhow!("Failed to fill password: {}", e))?;
            }

            // Click login button
            if let Some(sel) = login_button_selector {
                let js = format!(
                    "const el = document.querySelector('{}'); if(el) el.click();",
                    sel.replace("'", "\\'")
                );
                tab.evaluate(&js, false)
                    .map_err(|e| anyhow!("Failed to click login: {}", e))?;
                std::thread::sleep(Duration::from_secs(3));
                let _ = tab.wait_until_navigated();
            }

            std::thread::sleep(Duration::from_secs(2));
        }

        // Save session cookies
        save_cookies_to_file(&tab, session_id)?;

        // Navigate to target URL after login
        if url != login_url.unwrap_or("") {
            tab.navigate_to(url)
                .map_err(|e| anyhow!("Post-login navigation failed: {}", e))?;
            tab.wait_until_navigated()
                .map_err(|e| anyhow!("Post-login navigation timeout: {}", e))?;
            std::thread::sleep(Duration::from_secs(2));
            save_cookies_to_file(&tab, session_id)?;
        }

        let html = get_html(&tab)?;

        Ok(json!({
            "status": "ok",
            "session_created": true,
            "url": url,
            "html_length": html.len(),
            "html_preview": html.chars().take(500).collect::<String>()
        }))
    }
}

/// Take a screenshot within authenticated session
pub fn auth_screenshot(
    session_id: &str,
    output_path: &str,
    wait_seconds: u64,
) -> Result<serde_json::Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;

        let browser =
            Browser::new(launch_options).map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))?;

        let _ = load_cookies_from_file(&tab, session_id);

        let wait = Duration::from_secs(wait_seconds.max(3));
        std::thread::sleep(wait);

        let png_data = tab
            .capture_screenshot(ScreenshotFormat::PNG, None, true)
            .map_err(|e| anyhow!("Failed to capture screenshot: {}", e))?;

        fs::write(output_path, &png_data)
            .map_err(|e| anyhow!("Failed to write screenshot: {}", e))?;

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "output_path": output_path,
            "size_bytes": png_data.len()
        }))
    }
}

/// Login, navigate and extract visible text in ONE operation (for SPAs like Netacad)
pub fn auth_login_and_extract(
    url: &str,
    session_id: &str,
    login_url: Option<&str>,
    login_selector_user: Option<&str>,
    login_selector_pass: Option<&str>,
    username: Option<&str>,
    password: Option<&str>,
    login_button_selector: Option<&str>,
    wait_seconds: u64,
) -> Result<serde_json::Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;

        let browser =
            Browser::new(launch_options).map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))?;

        // Try to load existing session
        let session_loaded = load_cookies_from_file(&tab, session_id).is_ok();

        if !session_loaded && login_url.is_some() {
            // Perform login - handle SSO redirect
            if let (Some(l_url), Some(user), Some(pass)) =
                (login_url, username, password)
            {
                // Navigate to login page
                tab.navigate_to(l_url)
                    .map_err(|e| anyhow!("Login page navigation failed: {}", e))?;
                tab.wait_until_navigated()
                    .map_err(|e| anyhow!("Login navigation timeout: {}", e))?;
                
                // Wait for SPA/SSO to render the form
                std::thread::sleep(Duration::from_secs(5));

                // Check if we were redirected to an SSO page - look for actual input fields
                let check_inputs = "JSON.stringify(Array.from(document.querySelectorAll('input[type=\"text\"], input[type=\"email\"], input[name=\"username\"], input[name=\"email\"])).map(i => ({id: i.id, name: i.name, type: i.type, classes: i.className})))";
                let inputs_json = tab.evaluate(check_inputs, false)
                    .ok()
                    .and_then(|r| r.value)
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();

                // Fill username - try multiple selectors
                let user_selectors = vec![
                    login_selector_user.unwrap_or(""),
                    "input[type=\"email\"]",
                    "input[name=\"username\"]",
                    "input[name=\"email\"]",
                    "#username",
                    "#email",
                    "input[type=\"text\"]",
                ];

                for sel in &user_selectors {
                    if sel.is_empty() { continue; }
                    let js = format!(
                        "const el = document.querySelector('{}'); if(el && el.offsetParent !== null) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{ bubbles: true }})); el.dispatchEvent(new Event('change', {{ bubbles: true }})); true; }} else {{ false; }}",
                        sel.replace("'", "\\'"),
                        user.replace("'", "\\'")
                    );
                    let filled = tab.evaluate(&js, false)
                        .ok()
                        .and_then(|r| r.value)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if filled { break; }
                }

                // Fill password
                let pass_selectors = vec![
                    login_selector_pass.unwrap_or(""),
                    "input[type=\"password\"]",
                    "#password",
                    "input[name=\"password\"]",
                ];

                for sel in &pass_selectors {
                    if sel.is_empty() { continue; }
                    let js = format!(
                        "const el = document.querySelector('{}'); if(el && el.offsetParent !== null) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{ bubbles: true }})); el.dispatchEvent(new Event('change', {{ bubbles: true }})); true; }} else {{ false; }}",
                        sel.replace("'", "\\'"),
                        pass.replace("'", "\\'")
                    );
                    let filled = tab.evaluate(&js, false)
                        .ok()
                        .and_then(|r| r.value)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if filled { break; }
                }

                // Click login button - try multiple selectors
                let btn_selectors = vec![
                    login_button_selector.unwrap_or(""),
                    "button[type=\"submit\"]",
                    "input[type=\"submit\"]",
                    "button.login-button",
                    "button.btn-primary",
                    "#login-button",
                    "#submit",
                    "button:has-text('Sign In')",
                    "button:has-text('Log In')",
                ];

                for sel in &btn_selectors {
                    if sel.is_empty() { continue; }
                    let js = format!(
                        "const el = document.querySelector('{}'); if(el && el.offsetParent !== null) {{ el.click(); true; }} else {{ false; }}",
                        sel.replace("'", "\\'")
                    );
                    let clicked = tab.evaluate(&js, false)
                        .ok()
                        .and_then(|r| r.value)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if clicked {
                        std::thread::sleep(Duration::from_secs(3));
                        let _ = tab.wait_until_navigated();
                        break;
                    }
                }

                // Wait for SSO redirect and page load
                std::thread::sleep(Duration::from_secs(5));
            }
        }

        // Save session
        save_cookies_to_file(&tab, session_id)?;

        // Navigate to target URL
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Navigation to target failed: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;

        // Wait for SPA rendering
        let wait = Duration::from_secs(wait_seconds.max(5));
        std::thread::sleep(wait);

        // Extract visible text
        let js_code = "document.body.innerText";
        let remote_object = tab
            .evaluate(js_code, false)
            .map_err(|e| anyhow!("JS evaluation failed: {}", e))?;

        let visible_text = match remote_object.value {
            Some(Value::String(s)) => s,
            _ => String::new(),
        };

        let title_js = "document.title";
        let title = tab.evaluate(title_js, false)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        // Save final cookies
        save_cookies_to_file(&tab, session_id)?;

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "title": title,
            "text_length": visible_text.len(),
            "text": visible_text.chars().take(15000).collect::<String>()
        }))
    }
}

/// Extract visible text content from an authenticated session (rendered text, not HTML)
pub fn auth_extract_text(
    session_id: &str,
    wait_seconds: u64,
) -> Result<serde_json::Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;

        let browser =
            Browser::new(launch_options).map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))?;

        // Load session cookies
        load_cookies_from_file(&tab, session_id)
            .map_err(|e| anyhow!("No active session: {}", e))?;

        // Wait for SPA to render
        let wait = Duration::from_secs(wait_seconds.max(3));
        std::thread::sleep(wait);

        // Get visible text (innerText)
        let js_code = "document.body.innerText";
        let remote_object = tab
            .evaluate(js_code, false)
            .map_err(|e| anyhow!("JS evaluation failed: {}", e))?;

        let visible_text = match remote_object.value {
            Some(Value::String(s)) => s,
            _ => String::new(),
        };

        // Also get page title
        let title_js = "document.title";
        let title = tab.evaluate(title_js, false)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "title": title,
            "text_length": visible_text.len(),
            "text": visible_text.chars().take(10000).collect::<String>()
        }))
    }
}

/// Extract content from authenticated session
pub fn auth_extract(
    session_id: &str,
    selector: &str,
) -> Result<serde_json::Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        // For stateless MCP calls, we reload cookies into a fresh browser instance
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;

        let browser =
            Browser::new(launch_options).map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))?;

        // Load session cookies
        load_cookies_from_file(&tab, session_id)
            .map_err(|e| anyhow!("No active session: {}", e))?;

        // Extract text from selector
        let js_code = format!(
            "Array.from(document.querySelectorAll('{}')).map(el => el.innerText)",
            selector.replace("'", "\\'")
        );
        let remote_object = tab
            .evaluate(&js_code, false)
            .map_err(|e| anyhow!("JS evaluation failed: {}", e))?;

        match remote_object.value {
            Some(Value::Array(arr)) => {
                let texts: Vec<String> = arr
                    .into_iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                Ok(json!({
                    "status": "ok",
                    "session_id": session_id,
                    "selector": selector,
                    "count": texts.len(),
                    "texts": texts
                }))
            }
            _ => Ok(json!({
                "status": "ok",
                "session_id": session_id,
                "selector": selector,
                "texts": []
            })),
        }
    }
}

/// Navigate within an authenticated session
pub fn auth_navigate_session(
    session_id: &str,
    url: &str,
) -> Result<serde_json::Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;

        let browser =
            Browser::new(launch_options).map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))?;

        // Load session cookies
        load_cookies_from_file(&tab, session_id)
            .map_err(|e| anyhow!("No active session: {}", e))?;

        // Navigate to new URL
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
        std::thread::sleep(Duration::from_secs(2));

        let html = get_html(&tab)?;

        // Re-save cookies
        save_cookies_to_file(&tab, session_id)?;

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "url": url,
            "html_length": html.len(),
            "html_preview": html.chars().take(300).collect::<String>()
        }))
    }
}

/// Clear/delete an authenticated session
pub fn auth_clear_session(session_id: &str) -> Result<serde_json::Value> {
    let cookie_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".synapsis")
        .join("browser_sessions");
    let cookie_file = cookie_dir.join(format!("{}.json", session_id));

    if cookie_file.exists() {
        fs::remove_file(&cookie_file)?;
        Ok(json!({
            "status": "ok",
            "message": format!("Session {} cleared", session_id)
        }))
    } else {
        Ok(json!({
            "status": "ok",
            "message": format!("Session {} not found, nothing to clear", session_id)
        }))
    }
}

/// List all saved sessions
pub fn auth_list_sessions() -> Result<serde_json::Value> {
    let cookie_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".synapsis")
        .join("browser_sessions");

    if !cookie_dir.exists() {
        return Ok(json!({
            "status": "ok",
            "sessions": []
        }));
    }

    let sessions: Vec<Value> = fs::read_dir(&cookie_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let data = fs::read_to_string(&path).ok()?;
                let session: Value = serde_json::from_str(&data).ok()?;
                Some(session)
            } else {
                None
            }
        })
        .collect();

    Ok(json!({
        "status": "ok",
        "sessions": sessions
    }))
}

/// Helper: get HTML content
#[cfg(feature = "browser")]
fn get_html(tab: &Tab) -> Result<String> {
    let expr = "document.documentElement.outerHTML";
    let remote_object = tab
        .evaluate(expr, false)
        .map_err(|e| anyhow!("Failed to evaluate JavaScript: {}", e))?;
    match remote_object.value {
        Some(serde_json::Value::String(s)) => Ok(s),
        Some(other) => Ok(other.to_string()),
        None => Err(anyhow!("Failed to get HTML from remote object")),
    }
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_auth_navigate(
        url: &str,
        session_id: &str,
        login_url: Option<&str>,
        login_selector_user: Option<&str>,
        login_selector_pass: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
        login_button_selector: Option<&str>,
    ) -> serde_json::Value {
        match auth_navigate(
            url,
            session_id,
            login_url,
            login_selector_user,
            login_selector_pass,
            username,
            password,
            login_button_selector,
        ) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Auth navigation failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_screenshot(session_id: &str, output_path: &str, wait_seconds: Option<u64>) -> serde_json::Value {
        match auth_screenshot(session_id, output_path, wait_seconds.unwrap_or(5)) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Screenshot failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_login_and_extract(
        url: &str,
        session_id: &str,
        login_url: Option<&str>,
        login_selector_user: Option<&str>,
        login_selector_pass: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
        login_button_selector: Option<&str>,
        wait_seconds: u64,
    ) -> serde_json::Value {
        match auth_login_and_extract(
            url, session_id, login_url, login_selector_user,
            login_selector_pass, username, password, login_button_selector, wait_seconds,
        ) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Login and extract failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_extract(session_id: &str, selector: &str) -> serde_json::Value {
        match auth_extract(session_id, selector) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Extraction failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_extract_text(session_id: &str, wait_seconds: Option<u64>) -> serde_json::Value {
        match auth_extract_text(session_id, wait_seconds.unwrap_or(8)) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Text extraction failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_navigate_session(session_id: &str, url: &str) -> serde_json::Value {
        match auth_navigate_session(session_id, url) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Session navigation failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_clear_session(session_id: &str) -> serde_json::Value {
        match auth_clear_session(session_id) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Clear session failed: {}", e)
            }),
        }
    }

    pub fn handle_auth_list_sessions() -> serde_json::Value {
        match auth_list_sessions() {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("List sessions failed: {}", e)
            }),
        }
    }
}
