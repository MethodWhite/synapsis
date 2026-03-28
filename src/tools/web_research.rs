//! Web research tool using DuckDuckGo Instant Answer API

use anyhow::Result;
use serde_json::json;
use std::time::Duration;

/// Perform web research using DuckDuckGo Instant Answer API
pub fn web_research(query: &str, limit: usize) -> Result<serde_json::Value> {
    if query.is_empty() {
        return Ok(json!({
            "status": "error",
            "message": "Query cannot be empty"
        }));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let response = client
        .get("https://api.duckduckgo.com/")
        .query(&[
            ("q", query),
            ("format", "json"),
            ("pretty", "1"),
            ("no_html", "1"),
            ("skip_disambig", "1"),
        ])
        .send()?;

    if !response.status().is_success() {
        return Ok(json!({
            "status": "error",
            "message": format!("HTTP error: {}", response.status())
        }));
    }

    let api_response: serde_json::Value = response.json()?;

    let mut results = Vec::new();

    // Extract AbstractText
    if let Some(abstract_text) = api_response.get("AbstractText").and_then(|v| v.as_str()) {
        if !abstract_text.is_empty() {
            results.push(json!({
                "type": "abstract",
                "text": abstract_text,
                "source": api_response.get("AbstractURL").and_then(|v| v.as_str()).unwrap_or("")
            }));
        }
    }

    // Extract RelatedTopics
    if let Some(related_topics) = api_response.get("RelatedTopics").and_then(|v| v.as_array()) {
        for topic in related_topics.iter().take(limit) {
            if let Some(text) = topic.get("Text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    results.push(json!({
                        "type": "related",
                        "text": text,
                        "source": topic.get("FirstURL").and_then(|v| v.as_str()).unwrap_or("")
                    }));
                }
            }
        }
    }

    // Extract Results from Results array
    if let Some(api_results) = api_response.get("Results").and_then(|v| v.as_array()) {
        for result in api_results.iter().take(limit) {
            if let Some(text) = result.get("Text").and_then(|v| v.as_str()) {
                results.push(json!({
                    "type": "result",
                    "text": text,
                    "source": result.get("FirstURL").and_then(|v| v.as_str()).unwrap_or("")
                }));
            }
        }
    }

    Ok(json!({
        "status": "ok",
        "query": query,
        "total_results": results.len(),
        "results": results
    }))
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_web_research(query: &str, limit: usize) -> serde_json::Value {
        match web_research(query, limit) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("Web research failed: {}", e)
            }),
        }
    }
}
