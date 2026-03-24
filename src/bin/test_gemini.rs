use synapsis::tools::browser_navigation;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Testing browser navigation with gemini.google.com...");
    
    // Test 1: Navigate to Gemini
    println!("\n1. Navigating to gemini.google.com...");
    match browser_navigation::navigate_to_url("https://gemini.google.com") {
        Ok(html) => {
            println!("✓ Successfully navigated to gemini.google.com");
            println!("  HTML length: {} chars", html.len());
            
            // Check for key elements
            if html.contains("Gemini") || html.contains("gemini") {
                println!("  ✓ Found 'Gemini' in HTML");
            }
            if html.contains("chat") || html.contains("Chat") {
                println!("  ✓ Found chat-related content");
            }
            
            // Save HTML for inspection
            std::fs::write("/tmp/gemini_test.html", &html)?;
            println!("  HTML saved to /tmp/gemini_test.html");
        },
        Err(e) => {
            println!("✗ Failed to navigate: {}", e);
            return Err(e.into());
        }
    }
    
    // Test 2: Try to extract text from page (basic elements)
    println!("\n2. Extracting text from page...");
    match browser_navigation::extract_text("https://gemini.google.com", "h1, h2, h3, p") {
        Ok(texts) => {
            println!("✓ Successfully extracted text elements");
            println!("  Found {} text elements", texts.len());
            
            // Show first 10 elements
            for (i, text) in texts.iter().take(10).enumerate() {
                if !text.trim().is_empty() {
                    println!("  {}. {}", i + 1, text.trim());
                }
            }
            
            if texts.len() > 10 {
                println!("  ... and {} more elements", texts.len() - 10);
            }
        },
        Err(e) => {
            println!("✗ Failed to extract text: {}", e);
        }
    }
    
    // Test 3: Take screenshot
    println!("\n3. Taking screenshot...");
    let screenshot_path = "/tmp/gemini_screenshot.png";
    match browser_navigation::screenshot("https://gemini.google.com", screenshot_path) {
        Ok(()) => {
            println!("✓ Screenshot saved to {}", screenshot_path);
        },
        Err(e) => {
            println!("✗ Screenshot failed: {}", e);
        }
    }
    
    println!("\nAll tests completed!");
    Ok(())
}