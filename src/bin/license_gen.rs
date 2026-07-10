use std::io::{self, Write};

fn main() {
    println!("╔════════════════════════════════════════════════╗");
    println!("║     Synapsis License Generator                ║");
    println!("╚════════════════════════════════════════════════╝");
    println!();

    let customer = prompt("Customer name: ");
    let license_type = prompt("License type (individual/sme/commercial/enterprise): ");
    let features = prompt("Features (comma-separated, e.g.: pqc,web-search,ai-analysis): ");
    let days_valid = prompt("Days valid (e.g.: 365): ").parse::<i64>().unwrap_or(365);

    let features: Vec<String> = features.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::days(days_valid);

    let data = serde_json::json!({
        "customer": customer,
        "features": features,
        "issued_at": now.to_rfc3339(),
        "expires_at": expires.to_rfc3339(),
        "license_type": license_type,
    });

    let data_path = "/tmp/synapsis-license-data.json";
    std::fs::write(data_path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    println!();
    println!("License data written to: {}", data_path);
    println!();
    println!("Now sign it with:");
    println!("  echo '<your-private-key>' | synapsis license sign {}", data_path);
    println!();
    println!("Then send the .signed file to the customer.");
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
    s.trim().to_string()
}
