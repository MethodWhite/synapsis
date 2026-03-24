use synapsis::tools::browser_navigation;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing browser navigation...");
    
    // Test 1: Navigate to example.com
    match browser_navigation::navigate_to_url("https://example.com") {
        Ok(html) => {
            println!("✓ Successfully navigated to example.com");
            println!("  HTML length: {} chars", html.len());
            if html.contains("Example Domain") {
                println!("  ✓ Found 'Example Domain' in HTML");
            }
        },
        Err(e) => {
            println!("✗ Failed to navigate: {}", e);
            return Err(e.into());
        }
    }
    
    // Test 2: Extract text from h1
    match browser_navigation::extract_text("https://example.com", "h1") {
        Ok(texts) => {
            println!("✓ Successfully extracted text from h1 elements");
            println!("  Found {} h1 elements", texts.len());
            for (i, text) in texts.iter().enumerate() {
                println!("  {}. {}", i + 1, text);
            }
        },
        Err(e) => {
            println!("✗ Failed to extract text: {}", e);
        }
    }
    
    println!("All tests completed!");
    Ok(())
}