//! Synapsis Ollama CLI - Unified Ollama + MCP in Rust

use std::env;
use std::io::{self, BufRead, Write};

#[derive(Debug, serde::Deserialize)]
struct OllamaResponse {
    response: Option<String>,
    message: Option<Message>,
}

#[derive(Debug, serde::Deserialize)]
struct Message {
    content: String,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Synapsis Ollama CLI - Unified Ollama + MCP");
        println!();
        println!("Usage:");
        println!("  synapsis-ollama <model> <prompt>  Run a prompt");
        println!("  synapsis-ollama chat <model>      Interactive chat");
        println!("  synapsis-ollama mcp               MCP server mode");
        println!();
        println!("Models:");
        println!("  deepseek-coder:6.7b  - Coding");
        println!("  deepseek-r1-i1       - Reasoning");
        println!("  huihui-qwen-9b       - Chat");
        return;
    }

    match args[1].as_str() {
        "chat" => {
            let model = args.get(2).map(|s| s.as_str()).unwrap_or("huihui-qwen-9b");
            interactive_chat(model);
        }
        "mcp" => {
            println!("MCP server mode - connecting to Synapsis...");
            mcp_mode();
        }
        _ => {
            let model = &args[1];
            let prompt = args[2..].join(" ");
            run_prompt(model, &prompt);
        }
    }
}

fn run_prompt(model: &str, prompt: &str) {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post("http://127.0.0.1:11434/api/generate")
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .send();

    match response {
        Ok(resp) => {
            let json_res: Result<OllamaResponse, reqwest::Error> = resp.json();
            if let Ok(json) = json_res {
                if let Some(text) = json.response {
                    println!("{}", text);
                } else if let Some(msg) = json.message {
                    println!("{}", msg.content);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn interactive_chat(model: &str) {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!(
        "║  Synapsis Ollama Chat - Model: {}                  ║",
        model
    );
    println!("║  Type 'quit' to exit                                    ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    let client = reqwest::blocking::Client::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("\n🦙 > ");
        stdout.flush().unwrap();

        let mut input = String::new();
        if stdin.read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == "quit" || input == "exit" {
            break;
        }

        let response = client
            .post("http://127.0.0.1:11434/api/generate")
            .json(&serde_json::json!({
                "model": model,
                "prompt": input,
                "stream": false
            }))
            .send();

        match response {
            Ok(resp) => {
                let json_res: Result<OllamaResponse, reqwest::Error> = resp.json();
                if let Ok(json) = json_res {
                    if let Some(text) = json.response {
                        println!("🤖 {}", text);
                    }
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}

fn mcp_mode() {
    println!("MCP mode - reading from stdin...");
    let stdin = io::stdin();
    for line in stdin.lock().lines().flatten() {
        // Process MCP request
        println!("{{\"result\": \"processed: {}\"}}", line);
    }
}
