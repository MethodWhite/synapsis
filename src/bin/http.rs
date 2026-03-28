//! HTTP REST API server for Synapsis

use clap::Parser;
use std::net::SocketAddr;
use synapsis::api::rest;

#[derive(Parser)]
#[command(name = "synapsis-http")]
#[command(about = "HTTP REST API server for Synapsis")]
struct Args {
    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.verbose {
        env_logger::init();
        println!(
            "Starting Synapsis HTTP server on {}:{}",
            args.host, args.port
        );
    }

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .expect("Invalid address");

    let routes = rest::routes();

    println!("Synapsis HTTP server listening on http://{}", addr);

    warp::serve(routes).run(addr).await;
}
