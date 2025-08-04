use clap::{Args, Parser, Subcommand};
use dotenvy::dotenv;
use neems_data::{DataAggregator, NewSource, UpdateSource, create_source, list_sources, get_source_by_name, update_source, delete_source};
use std::env;
use std::error::Error;

#[derive(Parser)]
#[command(name = "neems-data")]
#[command(about = "Data aggregation service and source management for NEEMS")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the data monitoring and aggregation service
    Monitor {
        #[arg(short, long, help = "Enable verbose output showing data source polling")]
        verbose: bool,
    },
    /// List all sources
    #[command(alias = "ls")]
    List,
    /// Add a new source
    Add(AddArgs),
    /// Edit an existing source
    Edit(EditArgs),
    /// Remove a source
    #[command(alias = "rm")]
    Remove { 
        /// Name of the source to remove
        name: String 
    },
    /// Show details of a specific source
    Show { 
        /// Name of the source to show
        name: String 
    },
}

#[derive(Args)]
struct AddArgs {
    /// Name of the source
    name: String,
    /// Description of the source
    #[arg(short, long)]
    description: Option<String>,
    /// Interval in seconds (default: 1)
    #[arg(short, long, default_value = "1")]
    interval: i32,
    /// Whether the source is active (default: true)
    #[arg(short, long, default_value = "true")]
    active: bool,
}

#[derive(Args)]
struct EditArgs {
    /// Name of the source to edit
    name: String,
    /// New name for the source
    #[arg(long)]
    new_name: Option<String>,
    /// New description for the source
    #[arg(short, long)]
    description: Option<String>,
    /// New interval in seconds
    #[arg(short, long)]
    interval: Option<i32>,
    /// Set whether the source is active
    #[arg(short, long)]
    active: Option<bool>,
    /// Clear the description (set to null)
    #[arg(long)]
    clear_description: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenv().ok();

    let database_path = env::var("SITE_DATABASE_URL")
        .unwrap_or_else(|_| "site-data.sqlite".to_string());

    let aggregator = DataAggregator::new(Some(&database_path));
    let mut connection = aggregator.establish_connection()
        .map_err(|e| format!("Failed to establish database connection: {}", e))?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Monitor { verbose } => {
            println!("Starting neems-data aggregator...");
            println!("Database path: {}", database_path);
            if verbose {
                println!("Verbose mode enabled - will show data source polling details");
            }

            println!("Starting data aggregation process...");
            aggregator.start_aggregation(verbose).await?;
        }
        Commands::List => {
            let sources = list_sources(&mut connection)?;
            if sources.is_empty() {
                println!("No sources found.");
            } else {
                println!("{:<4} {:<25} {:<10} {:<8} {:<20} {}", 
                    "ID", "Name", "Interval", "Active", "Last Run", "Description");
                println!("{}", "-".repeat(80));
                for source in sources {
                    let last_run = source.last_run
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string());
                    
                    println!("{:<4} {:<25} {:<10} {:<8} {:<20} {}", 
                        source.id.unwrap_or(0),
                        source.name,
                        format!("{}s", source.interval_seconds),
                        source.active,
                        last_run,
                        source.description.unwrap_or_else(|| "".to_string())
                    );
                }
            }
        }
        Commands::Show { name } => {
            match get_source_by_name(&mut connection, &name)? {
                Some(source) => {
                    println!("Source Details:");
                    println!("  ID: {}", source.id.unwrap_or(0));
                    println!("  Name: {}", source.name);
                    println!("  Description: {}", source.description.unwrap_or_else(|| "(none)".to_string()));
                    println!("  Active: {}", source.active);
                    println!("  Interval: {} seconds", source.interval_seconds);
                    println!("  Created: {}", source.created_at.format("%Y-%m-%d %H:%M:%S"));
                    println!("  Updated: {}", source.updated_at.format("%Y-%m-%d %H:%M:%S"));
                    println!("  Last Run: {}", 
                        source.last_run
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "Never".to_string())
                    );
                }
                None => {
                    eprintln!("Error: Source '{}' not found.", name);
                    std::process::exit(1);
                }
            }
        }
        Commands::Add(args) => {
            // Check if source already exists
            if get_source_by_name(&mut connection, &args.name)?.is_some() {
                eprintln!("Error: Source '{}' already exists.", args.name);
                std::process::exit(1);
            }

            let new_source = NewSource {
                name: args.name.clone(),
                description: args.description,
                active: Some(args.active),
                interval_seconds: Some(args.interval),
            };

            let created = create_source(&mut connection, new_source)?;
            println!("Created source '{}' (ID: {})", created.name, created.id.unwrap_or(0));
        }
        Commands::Edit(args) => {
            // Check if source exists
            let existing = match get_source_by_name(&mut connection, &args.name)? {
                Some(source) => source,
                None => {
                    eprintln!("Error: Source '{}' not found.", args.name);
                    std::process::exit(1);
                }
            };

            let source_id = existing.id.unwrap();

            let description = if args.clear_description {
                Some(None)
            } else if args.description.is_some() {
                Some(args.description)
            } else {
                None
            };

            let updates = UpdateSource {
                name: args.new_name,
                description,
                active: args.active,
                interval_seconds: args.interval,
                last_run: None, // Don't modify last_run via CLI
            };

            let updated = update_source(&mut connection, source_id, updates)?;
            println!("Updated source '{}'", updated.name);
        }
        Commands::Remove { name } => {
            // Check if source exists
            let existing = match get_source_by_name(&mut connection, &name)? {
                Some(source) => source,
                None => {
                    eprintln!("Error: Source '{}' not found.", name);
                    std::process::exit(1);
                }
            };

            let source_id = existing.id.unwrap();

            // Delete the source
            let deleted_count = delete_source(&mut connection, source_id)?;

            if deleted_count > 0 {
                println!("Removed source '{}'", name);
            } else {
                eprintln!("Error: Failed to remove source '{}'", name);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
