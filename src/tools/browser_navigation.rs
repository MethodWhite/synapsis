//! Browser navigation tool using headless Chrome for web interaction and data collection.
//! Requires Chrome/Chromium installed and the `browser` feature enabled.

use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use std::time::Duration;
use std::fs;

#[cfg(feature = "browser")]
use headless_chrome::{Browser, LaunchOptionsBuilder};
#[cfg(feature = "browser")]
use headless_chrome::protocol::page::ScreenshotFormat;

#[cfg(feature = "browser")]
fn get_html(tab: &headless_chrome::Tab) -> Result<String> {
    let expr = "document.documentElement.outerHTML";
    let remote_object = tab.evaluate(expr, false)
        .map_err(|e| anyhow!("Failed to evaluate JavaScript: {}", e))?;
    match remote_object.value {
        Some(serde_json::Value::String(s)) => Ok(s),
        Some(other) => Ok(other.to_string()),
        None => Err(anyhow!("Failed to get HTML from remote object")),
    }
}

pub fn navigate_to_url(url: &str) -> Result<String> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow::anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;
        
        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Failed to create new tab: {}", e))?;
        
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Failed to navigate: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
        
        std::thread::sleep(Duration::from_secs(2));

        get_html(&tab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(feature = "browser")]
    fn test_navigate_to_example() {
        // This is an integration test that requires Chrome and internet
        // We'll skip it in CI but run it locally
        if std::env::var("RUN_BROWSER_TESTS").is_ok() {
            let result = navigate_to_url("https://example.com");
            assert!(result.is_ok());
            let html = result.unwrap();
            assert!(html.contains("Example Domain"));
        }
    }
    
    #[test]
    #[cfg(feature = "browser")]
    fn test_extract_text_from_example() {
        if std::env::var("RUN_BROWSER_TESTS").is_ok() {
            let result = extract_text("https://example.com", "h1");
            assert!(result.is_ok());
            let texts = result.unwrap();
            // Should contain at least one h1 with "Example Domain"
            assert!(!texts.is_empty());
            assert!(texts.iter().any(|t| t.contains("Example")));
        }
    }
}

/// Extract text content from elements matching a CSS selector.
pub fn extract_text(url: &str, selector: &str) -> Result<Vec<String>> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow::anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;
        
        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Failed to create new tab: {}", e))?;
        
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Failed to navigate: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
        
        std::thread::sleep(Duration::from_secs(2));

        // Use JavaScript to extract innerText from all matching elements
        let js_code = format!(
            "Array.from(document.querySelectorAll('{}')).map(el => el.innerText)",
            selector.replace("'", "\\'")
        );
        let remote_object = tab.evaluate(&js_code, false)
            .map_err(|e| anyhow!("Failed to evaluate JavaScript: {}", e))?;
        
        match remote_object.value {
            Some(serde_json::Value::Array(arr)) => {
                let texts: Vec<String> = arr.into_iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                Ok(texts)
            },
            _ => Ok(vec![]),
        }
    }
}

/// Click an element matching a CSS selector.
pub fn click_element(url: &str, selector: &str) -> Result<()> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow::anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;
        
        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Failed to create new tab: {}", e))?;
        
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Failed to navigate: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
        
        std::thread::sleep(Duration::from_secs(2));

        // Use JavaScript to click the element
        let js_code = format!(
            "document.querySelector('{}').click()",
            selector.replace("'", "\\'")
        );
        tab.evaluate(&js_code, false)
            .map_err(|e| anyhow!("Failed to click element: {}", e))?;
        
        // Wait for navigation if any
        let _ = tab.wait_until_navigated();
        std::thread::sleep(Duration::from_secs(1));
        
        Ok(())
    }
}

/// Fill a form field (input, textarea) matching a CSS selector with a value.
pub fn fill_form(url: &str, selector: &str, value: &str) -> Result<()> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow::anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;
        
        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Failed to create new tab: {}", e))?;
        
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Failed to navigate: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
        
        std::thread::sleep(Duration::from_secs(2));

        // Use JavaScript to fill the form field
        let js_code = format!(
            "const el = document.querySelector('{}'); el.value = '{}'; el.dispatchEvent(new Event('input', {{ bubbles: true }}));",
            selector.replace("'", "\\'"),
            value.replace("'", "\\'")
        );
        tab.evaluate(&js_code, false)
            .map_err(|e| anyhow!("Failed to fill form: {}", e))?;
        
        Ok(())
    }
}

/// Take a screenshot of the current page and save it to a file.
pub fn screenshot(url: &str, output_path: &str) -> Result<()> {
    #[cfg(not(feature = "browser"))]
    return Err(anyhow::anyhow!("Browser feature not enabled."));

    #[cfg(feature = "browser")]
    {
        let launch_options = LaunchOptionsBuilder::default()
            .headless(true)
            .idle_browser_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create launch options: {}", e))?;
        
        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow!("Failed to launch browser: {}", e))?;

        let tab = browser.new_tab()
            .map_err(|e| anyhow!("Failed to create new tab: {}", e))?;
        
        tab.navigate_to(url)
            .map_err(|e| anyhow!("Failed to navigate: {}", e))?;
        tab.wait_until_navigated()
            .map_err(|e| anyhow!("Navigation timeout: {}", e))?;
        
        std::thread::sleep(Duration::from_secs(2));

        let png_data = tab.capture_screenshot(
            ScreenshotFormat::PNG,
            None,
            true,
        ).map_err(|e| anyhow!("Failed to capture screenshot: {}", e))?;
        
        // Save to file
        fs::write(output_path, png_data)
            .map_err(|e| anyhow!("Failed to write screenshot: {}", e))?;
        
        Ok(())
    }
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;
    
    pub fn handle_navigate_to_url(url: &str) -> serde_json::Value {
        match navigate_to_url(url) {
            Ok(html) => json!({
                "status": "ok",
                "url": url,
                "html_length": html.len(),
                "html_preview": html.chars().take(500).collect::<String>()
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Navigation failed: {}", e)
            })
        }
    }
    
    pub fn handle_extract_text(url: &str, selector: &str) -> serde_json::Value {
        match extract_text(url, selector) {
            Ok(texts) => json!({
                "status": "ok",
                "url": url,
                "selector": selector,
                "count": texts.len(),
                "texts": texts
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Extraction failed: {}", e)
            })
        }
    }
    
    pub fn handle_click_element(url: &str, selector: &str) -> serde_json::Value {
        match click_element(url, selector) {
            Ok(()) => json!({
                "status": "ok",
                "url": url,
                "selector": selector,
                "message": "Element clicked successfully"
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Click failed: {}", e)
            })
        }
    }
    
    pub fn handle_fill_form(url: &str, selector: &str, value: &str) -> serde_json::Value {
        match fill_form(url, selector, value) {
            Ok(()) => json!({
                "status": "ok",
                "url": url,
                "selector": selector,
                "value": value,
                "message": "Form filled successfully"
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Form fill failed: {}", e)
            })
        }
    }
    
    pub fn handle_screenshot(url: &str, output_path: &str) -> serde_json::Value {
        match screenshot(url, output_path) {
            Ok(()) => json!({
                "status": "ok",
                "url": url,
                "output_path": output_path,
                "message": "Screenshot saved successfully"
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Screenshot failed: {}", e)
            })
        }
    }
}