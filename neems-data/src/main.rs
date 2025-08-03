use clap::Parser;
use dotenvy::dotenv;
use neems_data::DataAggregator;
use std::env;

#[derive(Parser)]
#[command(name = "neems-data")]
#[command(about = "Data aggregation service for NEEMS")]
struct Args {
    #[arg(short, long, help = "Enable verbose output showing data source polling")]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    let args = Args::parse();

    let database_path =
        env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string());

    println!("Starting neems-data aggregator...");
    println!("Database path: {}", database_path);
    if args.verbose {
        println!("Verbose mode enabled - will show data source polling details");
    }

    let aggregator = DataAggregator::new(Some(&database_path));

    match aggregator.establish_connection() {
        Ok(_) => println!("Database connection established successfully"),
        Err(e) => {
            eprintln!("Failed to establish database connection: {}", e);
            return Err(format!("Database connection failed: {}", e).into());
        }
    }

    println!("Starting data aggregation process...");
    aggregator.start_aggregation(args.verbose).await?;

    Ok(())
}
