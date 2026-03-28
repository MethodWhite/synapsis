//! CVE search tool using NVD API

use anyhow::Result;
use serde_json::json;
use std::time::Duration;

/// Search for CVEs using NVD API
pub fn cve_search(
    cve_id: Option<&str>,
    keyword: Option<&str>,
    limit: usize,
) -> Result<serde_json::Value> {
    if cve_id.is_none() && keyword.is_none() {
        return Ok(json!({
            "status": "error",
            "message": "Either cve_id or keyword must be provided"
        }));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let url = if let Some(id) = cve_id {
        // Search by CVE ID
        format!(
            "https://services.nvd.nist.gov/rest/json/cves/2.0?cveId={}",
            id
        )
    } else if let Some(kw) = keyword {
        // Keyword search
        format!(
            "https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch={}",
            kw
        )
    } else {
        unreachable!()
    };

    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        return Ok(json!({
            "status": "error",
            "message": format!("HTTP error: {}", response.status())
        }));
    }

    let api_response: serde_json::Value = response.json()?;

    let mut vulnerabilities = Vec::new();

    if let Some(vulns) = api_response
        .get("vulnerabilities")
        .and_then(|v| v.as_array())
    {
        for vuln in vulns.iter().take(limit) {
            if let Some(cve) = vuln.get("cve") {
                let cve_id = cve.get("id").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let description = cve
                    .get("descriptions")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|desc| desc.get("value").and_then(|v| v.as_str()))
                    .unwrap_or("");
                let published = cve.get("published").and_then(|v| v.as_str()).unwrap_or("");
                let last_modified = cve
                    .get("lastModified")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let cvss_score = cve
                    .get("metrics")
                    .and_then(|m| m.get("cvssMetricV31"))
                    .and_then(|arr| arr.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|metric| metric.get("cvssData"))
                    .and_then(|data| data.get("baseScore").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);

                vulnerabilities.push(json!({
                    "cve_id": cve_id,
                    "description": description,
                    "published": published,
                    "last_modified": last_modified,
                    "cvss_score": cvss_score,
                    "severity": match cvss_score {
                        s if s >= 9.0 => "CRITICAL",
                        s if s >= 7.0 => "HIGH",
                        s if s >= 4.0 => "MEDIUM",
                        s if s > 0.0 => "LOW",
                        _ => "UNKNOWN"
                    }
                }));
            }
        }
    }

    Ok(json!({
        "status": "ok",
        "total_results": vulnerabilities.len(),
        "vulnerabilities": vulnerabilities
    }))
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_cve_search(
        cve_id: Option<&str>,
        keyword: Option<&str>,
        limit: usize,
    ) -> serde_json::Value {
        match cve_search(cve_id, keyword, limit) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("CVE search failed: {}", e)
            }),
        }
    }
}
