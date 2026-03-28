//! SQLCipher performance benchmark
//! Measures overhead of encryption compared to plain SQLite.

use hex;
use rand;
use std::time::Instant;
use synapsis::infrastructure::database::Database;
use synapsis::StoragePort;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SQLCipher Performance Benchmark");
    println!("================================");

    // Use temporary directory for benchmark databases
    let temp_dir = std::env::temp_dir();
    let encrypted_path = temp_dir.join("synapsis_bench_encrypted.db");
    let plain_path = temp_dir.join("synapsis_bench_plain.db");

    // Clean up any existing files
    let _ = std::fs::remove_file(&encrypted_path);
    let _ = std::fs::remove_file(&plain_path);

    // Generate a random encryption key (32 bytes)
    let key: [u8; 32] = rand::random();
    let hex_key = hex::encode(key);

    // Set environment variable for encrypted database
    std::env::set_var("SYNAPSIS_DB_KEY", hex_key.clone());

    // Create encrypted database
    println!("Creating encrypted database...");
    let encrypted_db = Database::new_with_key(Some(key.to_vec()));
    encrypted_db.init()?;

    // Clear environment variable for plain database
    std::env::remove_var("SYNAPSIS_DB_KEY");

    // Create plain database
    println!("Creating plain database...");
    let plain_db = Database::new_with_key(None);
    plain_db.init()?;

    // Benchmark: Insert observations
    println!("\nBenchmarking insert operations...");
    let iterations = 1000;

    let encrypted_insert_time = benchmark_insert(&encrypted_db, iterations)?;
    let plain_insert_time = benchmark_insert(&plain_db, iterations)?;

    let overhead =
        (encrypted_insert_time.as_secs_f64() / plain_insert_time.as_secs_f64() - 1.0) * 100.0;

    println!("Encrypted insert time: {:?}", encrypted_insert_time);
    println!("Plain insert time: {:?}", plain_insert_time);
    println!("Overhead: {:.2}%", overhead);

    // Benchmark: Query operations
    println!("\nBenchmarking query operations...");
    let encrypted_query_time = benchmark_query(&encrypted_db)?;
    let plain_query_time = benchmark_query(&plain_db)?;

    let query_overhead =
        (encrypted_query_time.as_secs_f64() / plain_query_time.as_secs_f64() - 1.0) * 100.0;
    println!("Encrypted query time: {:?}", encrypted_query_time);
    println!("Plain query time: {:?}", plain_query_time);
    println!("Query overhead: {:.2}%", query_overhead);

    // Clean up
    let _ = std::fs::remove_file(&encrypted_path);
    let _ = std::fs::remove_file(&plain_path);

    println!("\nBenchmark complete.");
    if overhead < 5.0 {
        println!("✅ SQLCipher overhead is within target (<5%).");
    } else {
        println!(
            "⚠️  SQLCipher overhead exceeds target (>{:.2}%). Consider optimization.",
            overhead
        );
    }

    Ok(())
}

fn benchmark_insert(
    db: &Database,
    count: usize,
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let start = Instant::now();

    for i in 0..count {
        let project = "benchmark";
        let title = format!("Observation {}", i);
        let content = format!("Content for observation {}", i);

        // Use the create_chunk method as a representative insert operation
        db.create_chunk(project, &title, &content, None, 0)?;
    }

    Ok(start.elapsed())
}

fn benchmark_query(db: &Database) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let start = Instant::now();

    // Query chunks for the benchmark project
    let chunks = db.get_chunks_by_project("benchmark", None)?;
    let _count = chunks.len(); // Ensure we consume the result

    Ok(start.elapsed())
}
