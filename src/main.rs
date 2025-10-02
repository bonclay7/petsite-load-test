use clap::Parser;
use colored::*;
use std::time::Instant;

mod load_tester;
mod ssm_discovery;
mod types;

use load_tester::LoadTester;
use ssm_discovery::SSMEndpointDiscovery;

#[derive(Parser)]
#[command(name = "microservice-load-tester")]
#[command(about = "High concurrent load testing CLI for microservices")]
#[command(version = "1.0.0")]
struct Args {
    /// Number of concurrent users
    #[arg(short, long, default_value = "10")]
    users: usize,

    /// Concurrent requests per user
    #[arg(short, long, default_value = "5")]
    concurrent: usize,

    /// AWS region
    #[arg(short, long, default_value = "us-east-1")]
    region: String,

    /// Show what would be tested without executing
    #[arg(long)]
    dry_run: bool,

    /// Enable verbose output showing individual request results
    #[arg(short, long)]
    verbose: bool,

    /// Ramp-up time in seconds to gradually increase load (0 = no ramp-up)
    #[arg(long, default_value = "0")]
    rampup: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("{}", "🚀 Microservice Load Tester".blue().bold());
    println!(
        "{}",
        format!(
            "Users: {}, Concurrent: {}, Region: {}",
            args.users, args.concurrent, args.region
        )
        .bright_black()
    );

    let start_time = Instant::now();

    // Discover endpoints from SSM
    let discovery = SSMEndpointDiscovery::new(&args.region).await?;
    let endpoints = discovery.discover_endpoints().await?;

    if endpoints.is_empty() {
        println!(
            "{}",
            "⚠️  No endpoints found in SSM. Using fallback endpoints.".yellow()
        );
    }

    // Initialize load tester
    let load_tester = LoadTester::new(
        args.users,
        args.concurrent,
        endpoints,
        args.dry_run,
        args.verbose,
        args.rampup,
    );

    // Run the load test
    let results = load_tester.run_load_test().await?;

    // Display results
    load_tester.display_results(&results, start_time.elapsed());

    Ok(())
}
