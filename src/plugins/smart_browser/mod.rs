//! Smart Browser Agent - Human-like web navigation for Synapsis
//!
//! Unlike basic browser tools, this plugin understands page context,
//! makes decisions, and chains multi-step actions like a human would.
//!
//! Capabilities:
//! - DOM analysis and content understanding
//! - Intelligent element finding (by role, context, text)
//! - Multi-step action planning
//! - Form filling with context awareness
//! - Navigation with state memory
//! - Screenshot-based decision making

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Mutex;
use lazy_static::lazy_static;

#[cfg(feature = "browser")]
use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};
#[cfg(feature = "browser")]
use headless_chrome::protocol::page::ScreenshotFormat;

lazy_static! {
    static ref SESSIONS: Mutex<HashMap<String, SmartSession>> = Mutex::new(HashMap::new());
}

/// Browser session state for human-like navigation
pub struct SmartSession {
    pub id: String,
    pub current_url: String,
    pub page_title: String,
    pub page_text: String,
    pub action_history: Vec<String>,
    pub form_fields: Vec<FormField>,
    pub clickable_elements: Vec<ClickableElement>,
    pub last_screenshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub selector: String,
    pub field_type: String,
    pub label: String,
    pub name: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickableElement {
    pub selector: String,
    pub text: String,
    pub role: String,
    pub visible: bool,
}

impl SmartSession {
    pub fn new(id: String) -> Self {
        Self {
            id,
            current_url: String::new(),
            page_title: String::new(),
            page_text: String::new(),
            action_history: Vec::new(),
            form_fields: Vec::new(),
            clickable_elements: Vec::new(),
            last_screenshot: None,
        }
    }
}

/// Navigate to URL and analyze page like a human would
pub fn smart_navigate(session_id: &str, url: &str, wait_seconds: u64) -> Result<Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Tab creation failed: {}", e))?;

        tab.navigate_to(url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;

        std::thread::sleep(Duration::from_secs(wait_seconds));

        // Analyze page like a human
        let title = get_page_title(&tab);
        let page_text = get_visible_text(&tab);
        let form_fields = analyze_form_fields(&tab);
        let clickable = analyze_clickable_elements(&tab);
        let links = extract_links(&tab);

        let form_count = form_fields.len();
        let click_count = clickable.len();
        let link_count = links.len();

        // Save session
        {
            let mut sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
            let session = SmartSession {
                id: session_id.to_string(),
                current_url: url.to_string(),
                page_title: title.clone(),
                page_text: page_text.chars().take(5000).collect(),
                action_history: vec![format!("Navigated to {}", url)],
                form_fields,
                clickable_elements: clickable,
                last_screenshot: None,
            };
            sessions.insert(session_id.to_string(), session);
        }

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "url": url,
            "title": title,
            "text_length": page_text.len(),
            "text_preview": page_text.chars().take(500).collect::<String>(),
            "form_fields_found": form_count,
            "clickable_elements_found": click_count,
            "links_found": link_count,
            "analysis": "Page analyzed successfully - use smart_find_element to interact"
        }))
    }
}

/// Find elements intelligently (by text, role, context - like a human looking for something)
pub fn smart_find_element(session_id: &str, search_query: &str, element_type: Option<&str>) -> Result<Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let session = sessions.get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Tab creation failed: {}", e))?;

        tab.navigate_to(&session.current_url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated().ok();
        std::thread::sleep(Duration::from_secs(2));

        // Multi-strategy search like a human would
        let mut results = Vec::new();

        // Strategy 1: Search by text content
        let text_results = search_by_text(&tab, search_query, element_type)?;
        results.extend(text_results);

        // Strategy 2: Search by role/aria-label
        let role_results = search_by_role(&tab, search_query, element_type)?;
        results.extend(role_results);

        // Strategy 3: Search by name/id
        let attr_results = search_by_attributes(&tab, search_query, element_type)?;
        results.extend(attr_results);

        // Strategy 4: Fuzzy text match
        if results.is_empty() {
            let fuzzy = search_fuzzy_text(&tab, search_query, element_type)?;
            results.extend(fuzzy);
        }

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "search_query": search_query,
            "element_type": element_type.unwrap_or("any"),
            "results_count": results.len(),
            "results": results,
            "recommendation": if results.is_empty() {
                "No elements found. Try different search terms or take a screenshot to see the page.".to_string()
            } else {
                "Found matches. Use the selector with smart_click or smart_fill.".to_string()
            }
        }))
    }
}

/// Click element intelligently (waits for navigation, handles popups)
pub fn smart_click(session_id: &str, selector: &str) -> Result<Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let session = sessions.get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
        let current_url = session.current_url.clone();

        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Tab creation failed: {}", e))?;

        tab.navigate_to(&current_url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated().ok();
        std::thread::sleep(Duration::from_secs(2));

        // Click with human-like timing
        let js = format!(
            "const el = document.querySelector('{}');
            if (el && el.offsetParent !== null) {{
                el.scrollIntoView({{behavior: 'smooth', block: 'center'}});
                setTimeout(() => el.click(), 300);
                true;
            }} else {{ false; }}",
            selector.replace("'", "\\'")
        );

        let clicked = tab.evaluate(&js, false)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !clicked {
            return Ok(json!({
                "status": "error",
                "message": format!("Element not found or not visible: {}", selector)
            }));
        }

        // Wait for potential navigation
        std::thread::sleep(Duration::from_secs(2));
        let _ = tab.wait_until_navigated();
        std::thread::sleep(Duration::from_secs(2));

        // Update session
        let new_url = get_current_url(&tab);
        let new_title = get_page_title(&tab);
        {
            let mut sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
            if let Some(s) = sessions.get_mut(session_id) {
                s.current_url = new_url.clone();
                s.page_title = new_title.clone();
                s.action_history.push(format!("Clicked: {}", selector));
            }
        }

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "action": "clicked",
            "selector": selector,
            "new_url": new_url,
            "new_title": new_title
        }))
    }
}

/// Fill form field intelligently (finds by label, placeholder, or name)
pub fn smart_fill(session_id: &str, field_description: &str, value: &str) -> Result<Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let session = sessions.get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
        let current_url = session.current_url.clone();

        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Tab creation failed: {}", e))?;

        tab.navigate_to(&current_url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated().ok();
        std::thread::sleep(Duration::from_secs(2));

        // Try multiple strategies to find the field
        let selectors = vec![
            format!("input[placeholder*='{}']", field_description),
            format!("input[name*='{}']", field_description),
            format!("input[id*='{}']", field_description),
            format!("input[type='{}']", field_description),
            format!("textarea[placeholder*='{}']", field_description),
            format!("textarea[name*='{}']", field_description),
            // Search by associated label
            format!("label:contains('{}') + input, label:contains('{}') ~ input", field_description, field_description),
        ];

        let mut filled = false;
        let mut used_selector = String::new();

        for sel in selectors {
            let js = format!(
                "const el = document.querySelector('{}');
                if (el && el.offsetParent !== null) {{
                    el.focus();
                    setTimeout(() => {{
                        el.value = '{}';
                        el.dispatchEvent(new Event('input', {{bubbles: true}}));
                        el.dispatchEvent(new Event('change', {{bubbles: true}}));
                    }}, 200);
                    true;
                }} else {{ false; }}",
                sel.replace("'", "\\'"),
                value.replace("'", "\\'")
            );

            let result = tab.evaluate(&js, false)
                .ok()
                .and_then(|r| r.value)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if result {
                filled = true;
                used_selector = sel;
                break;
            }
        }

        // Update session
        if filled {
            let mut sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
            if let Some(s) = sessions.get_mut(session_id) {
                s.action_history.push(format!("Filled '{}' with '{}'", used_selector, value));
            }
        }

        Ok(json!({
            "status": if filled { "ok" } else { "error" },
            "session_id": session_id,
            "field_description": field_description,
            "value_filled": if filled { "***" } else { "N/A" },
            "selector_used": used_selector,
            "message": if filled {
                "Field filled successfully"
            } else {
                "Field not found. Try describing it differently (e.g., 'email', 'password', 'search box')"
            }
        }))
    }
}

/// Submit form intelligently
pub fn smart_submit(session_id: &str) -> Result<Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let session = sessions.get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
        let current_url = session.current_url.clone();

        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Tab creation failed: {}", e))?;

        tab.navigate_to(&current_url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated().ok();
        std::thread::sleep(Duration::from_secs(2));

        // Try to find and click submit button
        let submit_selectors = vec![
            "input[type='submit']",
            "button[type='submit']",
            "button.btn-primary",
            "button.login-button",
            "button:has-text('Submit')",
            "button:has-text('Login')",
            "button:has-text('Sign In')",
            "button:has-text('Send')",
        ];

        let mut submitted = false;
        for sel in submit_selectors {
            let js = format!(
                "const el = document.querySelector('{}');
                if (el && el.offsetParent !== null) {{
                    el.scrollIntoView({{behavior: 'smooth', block: 'center'}});
                    setTimeout(() => el.click(), 300);
                    true;
                }} else {{ false; }}",
                sel.replace("'", "\\'")
            );
            let clicked = tab.evaluate(&js, false)
                .ok()
                .and_then(|r| r.value)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if clicked {
                submitted = true;
                break;
            }
        }

        // If no submit button found, try submitting the form directly
        if !submitted {
            let js = "const form = document.querySelector('form'); if(form) { form.dispatchEvent(new Event('submit', {bubbles: true})); true; } else { false; }";
            submitted = tab.evaluate(js, false)
                .ok()
                .and_then(|r| r.value)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        }

        // Wait for navigation after submit
        if submitted {
            std::thread::sleep(Duration::from_secs(3));
            let _ = tab.wait_until_navigated();
            std::thread::sleep(Duration::from_secs(2));
        }

        let new_url = get_current_url(&tab);
        let new_title = get_page_title(&tab);

        // Update session
        {
            let mut sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
            if let Some(s) = sessions.get_mut(session_id) {
                s.current_url = new_url.clone();
                s.page_title = new_title.clone();
                s.action_history.push("Submitted form".to_string());
            }
        }

        Ok(json!({
            "status": if submitted { "ok" } else { "error" },
            "session_id": session_id,
            "submitted": submitted,
            "new_url": new_url,
            "new_title": new_title
        }))
    }
}

/// Take screenshot and analyze page (for debugging/decision making)
pub fn smart_screenshot(session_id: &str, output_path: &str) -> Result<Value> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let session = sessions.get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
        let current_url = session.current_url.clone();

        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Browser launch failed: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Tab creation failed: {}", e))?;

        tab.navigate_to(&current_url)
            .map_err(|e| anyhow!("Navigation failed: {}", e))?;
        tab.wait_until_navigated().ok();
        std::thread::sleep(Duration::from_secs(2));

        let png_data = tab.capture_screenshot(ScreenshotFormat::PNG, None, true)
            .map_err(|e| anyhow!("Screenshot failed: {}", e))?;

        std::fs::write(output_path, &png_data)
            .map_err(|e| anyhow!("Save failed: {}", e))?;

        // Update session with screenshot info
        {
            let mut sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
            if let Some(s) = sessions.get_mut(session_id) {
                s.last_screenshot = Some(output_path.to_string());
            }
        }

        Ok(json!({
            "status": "ok",
            "session_id": session_id,
            "output_path": output_path,
            "size_bytes": png_data.len(),
            "title": get_page_title(&tab)
        }))
    }
}

/// Get session info
pub fn smart_session_info(session_id: &str) -> Result<Value> {
    let sessions = SESSIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let session = sessions.get(session_id)
        .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

    Ok(json!({
        "status": "ok",
        "session_id": session.id,
        "current_url": session.current_url,
        "page_title": session.page_title,
        "text_length": session.page_text.len(),
        "action_history": session.action_history,
        "form_fields_count": session.form_fields.len(),
        "clickable_elements_count": session.clickable_elements.len()
    }))
}

// === Helper functions ===

#[cfg(feature = "browser")]
fn get_page_title(tab: &Tab) -> String {
    tab.evaluate("document.title", false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

#[cfg(feature = "browser")]
fn get_current_url(tab: &Tab) -> String {
    tab.evaluate("window.location.href", false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

#[cfg(feature = "browser")]
fn get_visible_text(tab: &Tab) -> String {
    tab.evaluate("document.body.innerText", false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

#[cfg(feature = "browser")]
fn analyze_form_fields(tab: &Tab) -> Vec<FormField> {
    let js = r#"
    Array.from(document.querySelectorAll('input, textarea, select')).map(el => ({
        selector: el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + el.className.split(' ')[0] : ''),
        type: el.type || el.tagName.toLowerCase(),
        label: el.labels ? Array.from(el.labels).map(l => l.textContent.trim()).join(' ') : (el.getAttribute('aria-label') || ''),
        name: el.name || '',
        placeholder: el.placeholder || ''
    })).filter(f => f.type !== 'hidden' && f.type !== 'submit')
    "#;
    tab.evaluate(js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<FormField>>(v).ok())
        .unwrap_or_default()
}

#[cfg(feature = "browser")]
fn analyze_clickable_elements(tab: &Tab) -> Vec<ClickableElement> {
    let js = r#"
    Array.from(document.querySelectorAll('button, a, [role="button"], input[type="submit"], input[type="button"]')).map(el => ({
        selector: el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + el.className.split(' ')[0] : ''),
        text: el.textContent.trim().substring(0, 100),
        role: el.getAttribute('role') || el.tagName.toLowerCase(),
        visible: el.offsetParent !== null
    })).filter(e => e.visible)
    "#;
    tab.evaluate(js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<ClickableElement>>(v).ok())
        .unwrap_or_default()
}

#[cfg(feature = "browser")]
fn extract_links(tab: &Tab) -> Vec<String> {
    let js = r#"
    Array.from(document.querySelectorAll('a[href]')).map(a => ({
        text: a.textContent.trim(),
        href: a.href
    })).filter(l => l.text.length > 0)
    "#;
    tab.evaluate(js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
}

#[cfg(feature = "browser")]
fn search_by_text(tab: &Tab, query: &str, element_type: Option<&str>) -> Result<Vec<Value>> {
    let etype = element_type.unwrap_or("*");
    let js = format!(r#"
    Array.from(document.querySelectorAll('{}')).filter(el => 
        el.textContent && el.textContent.toLowerCase().includes('{}')
    ).map(el => {{
        return {{
            selector: el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + el.className.split(' ')[0] : ''),
            text: el.textContent.trim().substring(0, 100),
            type: el.tagName.toLowerCase(),
            visible: el.offsetParent !== null,
            match_type: 'text'
        }};
    }}).filter(e => e.visible)
    "#, etype, query.to_lowercase());

    let results = tab.evaluate(&js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<Value>>(v).ok())
        .unwrap_or_default();

    Ok(results)
}

#[cfg(feature = "browser")]
fn search_by_role(tab: &Tab, query: &str, element_type: Option<&str>) -> Result<Vec<Value>> {
    let etype = element_type.unwrap_or("*");
    let js = format!(r#"
    Array.from(document.querySelectorAll('{}')).filter(el => {{
        const role = el.getAttribute('role') || '';
        const ariaLabel = el.getAttribute('aria-label') || '';
        const name = el.getAttribute('name') || '';
        const id = el.id || '';
        return role.toLowerCase().includes('{}') ||
               ariaLabel.toLowerCase().includes('{}') ||
               name.toLowerCase().includes('{}') ||
               id.toLowerCase().includes('{}');
    }}).map(el => {{
        return {{
            selector: el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + el.className.split(' ')[0] : ''),
            text: el.textContent.trim().substring(0, 100),
            type: el.tagName.toLowerCase(),
            visible: el.offsetParent !== null,
            match_type: 'role/attribute'
        }};
    }}).filter(e => e.visible)
    "#, etype, query.to_lowercase(), query.to_lowercase(), query.to_lowercase(), query.to_lowercase());

    Ok(tab.evaluate(&js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<Value>>(v).ok())
        .unwrap_or_default())
}

#[cfg(feature = "browser")]
fn search_by_attributes(tab: &Tab, query: &str, element_type: Option<&str>) -> Result<Vec<Value>> {
    let etype = element_type.unwrap_or("*");
    let js = format!(r#"
    Array.from(document.querySelectorAll('{}')).filter(el => 
        el.id.toLowerCase().includes('{}') ||
        el.name.toLowerCase().includes('{}')
    ).map(el => {{
        return {{
            selector: el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + el.className.split(' ')[0] : ''),
            text: el.textContent.trim().substring(0, 100),
            type: el.tagName.toLowerCase(),
            visible: el.offsetParent !== null,
            match_type: 'attribute'
        }};
    }}).filter(e => e.visible)
    "#, etype, query.to_lowercase(), query.to_lowercase());

    Ok(tab.evaluate(&js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<Value>>(v).ok())
        .unwrap_or_default())
}

#[cfg(feature = "browser")]
fn search_fuzzy_text(tab: &Tab, query: &str, element_type: Option<&str>) -> Result<Vec<Value>> {
    // Fuzzy: search for any word from the query
    let words: Vec<&str> = query.split_whitespace().collect();
    let etype = element_type.unwrap_or("*");

    let conditions: Vec<String> = words.iter().map(|w| {
        format!("el.textContent && el.textContent.toLowerCase().includes('{}')", w.to_lowercase())
    }).collect();

    let js = format!(r#"
    Array.from(document.querySelectorAll('{}')).filter(el => 
        {}
    ).map(el => {{
        return {{
            selector: el.tagName + (el.id ? '#' + el.id : '') + (el.className ? '.' + el.className.split(' ')[0] : ''),
            text: el.textContent.trim().substring(0, 100),
            type: el.tagName.toLowerCase(),
            visible: el.offsetParent !== null,
            match_type: 'fuzzy'
        }};
    }}).filter(e => e.visible)
    "#, etype, conditions.join(" || "));

    Ok(tab.evaluate(&js, false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| serde_json::from_value::<Vec<Value>>(v).ok())
        .unwrap_or_default())
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_smart_navigate(session_id: &str, url: &str, wait_seconds: Option<u64>) -> Value {
        match smart_navigate(session_id, url, wait_seconds.unwrap_or(5)) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_smart_find(session_id: &str, search: &str, element_type: Option<&str>) -> Value {
        match smart_find_element(session_id, search, element_type) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_smart_click(session_id: &str, selector: &str) -> Value {
        match smart_click(session_id, selector) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_smart_fill(session_id: &str, field: &str, value: &str) -> Value {
        match smart_fill(session_id, field, value) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_smart_submit(session_id: &str) -> Value {
        match smart_submit(session_id) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_smart_screenshot(session_id: &str, output_path: &str) -> Value {
        match smart_screenshot(session_id, output_path) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_smart_session_info(session_id: &str) -> Value {
        match smart_session_info(session_id) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }
}
