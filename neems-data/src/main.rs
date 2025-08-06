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
    /// Test type (ping, charging_state, disk_space)
    #[arg(short = 't', long)]
    test_type: String,
    /// Test arguments in key=value format (can be used multiple times)
    #[arg(short = 'a', long = "arg", value_parser = parse_key_val)]
    arguments: Vec<(String, String)>,
    /// Description of the source
    #[arg(short, long)]
    description: Option<String>,
    /// Interval in seconds (default: 1)
    #[arg(short, long, default_value = "1")]
    interval: i32,
    /// Whether the source is active (default: true)
    #[arg(long, default_value = "true")]
    active: bool,
    /// Site ID that this source belongs to
    #[arg(long)]
    site_id: Option<i32>,
    /// Company ID that this source belongs to
    #[arg(long)]
    company_id: Option<i32>,
}

/// Parse a single key=value pair
fn parse_key_val(s: &str) -> Result<(String, String), Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

#[derive(Args)]
struct EditArgs {
    /// Name of the source to edit
    name: String,
    /// New name for the source
    #[arg(long)]
    new_name: Option<String>,
    /// New test type (ping, charging_state, disk_space)
    #[arg(short = 't', long)]
    test_type: Option<String>,
    /// New test arguments in key=value format (can be used multiple times)
    #[arg(short = 'a', long = "arg", value_parser = parse_key_val)]
    arguments: Vec<(String, String)>,
    /// New description for the source
    #[arg(short, long)]
    description: Option<String>,
    /// New interval in seconds
    #[arg(short, long)]
    interval: Option<i32>,
    /// Set whether the source is active
    #[arg(long)]
    active: Option<bool>,
    /// Clear the description (set to null)
    #[arg(long)]
    clear_description: bool,
    /// Clear all arguments (set to empty)
    #[arg(long)]
    clear_arguments: bool,
    /// New site ID
    #[arg(long)]
    site_id: Option<i32>,
    /// New company ID
    #[arg(long)]
    company_id: Option<i32>,
    /// Clear the site ID (set to null)
    #[arg(long)]
    clear_site_id: bool,
    /// Clear the company ID (set to null)
    #[arg(long)]
    clear_company_id: bool,
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
                println!("{:<4} {:<20} {:<15} {:<15} {:<8} {:<8} {:<8} {:<20} {}",
                    "ID", "Name", "Test Type", "Arguments", "Active", "Site", "Company", "Last Run", "Description");
                println!("{}", "-".repeat(120));
                for source in sources {
                    let last_run = source.last_run
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string());

                    let test_type = source.test_type.as_deref().unwrap_or("(legacy)");
                    let arguments = match &source.arguments {
                        Some(args_json) => {
                            match serde_json::from_str::<std::collections::HashMap<String, String>>(args_json) {
                                Ok(args) if args.is_empty() => "{}".to_string(),
                                Ok(args) => {
                                    let formatted: Vec<String> = args.iter()
                                        .map(|(k, v)| format!("{}={}", k, v))
                                        .collect();
                                    formatted.join(",")
                                }
                                Err(_) => "(invalid)".to_string(),
                            }
                        }
                        None => "(none)".to_string(),
                    };

                    // Truncate arguments if too long for display
                    let args_display = if arguments.len() > 13 {
                        format!("{}...", &arguments[..10])
                    } else {
                        arguments
                    };

                    println!("{:<4} {:<20} {:<15} {:<15} {:<8} {:<8} {:<8} {:<20} {}",
                        source.id.unwrap_or(0),
                        source.name,
                        test_type,
                        args_display,
                        source.active,
                        source.site_id.map(|id| id.to_string()).unwrap_or_else(|| "-".to_string()),
                        source.company_id.map(|id| id.to_string()).unwrap_or_else(|| "-".to_string()),
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
                    println!("  Test Type: {}", source.test_type.as_deref().unwrap_or("(legacy)"));

                    // Display arguments nicely
                    match &source.arguments {
                        Some(args_json) => {
                            match serde_json::from_str::<std::collections::HashMap<String, String>>(args_json) {
                                Ok(args) if args.is_empty() => println!("  Arguments: (none)"),
                                Ok(args) => {
                                    println!("  Arguments:");
                                    for (key, value) in &args {
                                        println!("    {}: {}", key, value);
                                    }
                                }
                                Err(_) => println!("  Arguments: (invalid JSON)"),
                            }
                        }
                        None => println!("  Arguments: (none)"),
                    }

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
                    println!("  Site ID: {}", source.site_id.map(|id| id.to_string()).unwrap_or_else(|| "(none)".to_string()));
                    println!("  Company ID: {}", source.company_id.map(|id| id.to_string()).unwrap_or_else(|| "(none)".to_string()));
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

            // Validate test type
            use neems_data::collectors::TestType;
            let _test_type = TestType::from_str(&args.test_type)
                .map_err(|e| format!("Invalid test type '{}': {}", args.test_type, e))?;

            // Convert arguments Vec to HashMap
            let mut arguments = std::collections::HashMap::new();
            for (key, value) in args.arguments {
                arguments.insert(key, value);
            }

            // Use environment variables for defaults if not provided
            let site_id = args.site_id.or_else(|| {
                env::var("NEEMS_DEFAULT_SITE").ok().and_then(|s| s.parse().ok())
            });
            let company_id = args.company_id.or_else(|| {
                env::var("NEEMS_DEFAULT_COMPANY").ok().and_then(|s| s.parse().ok())
            });

            let test_type_str = args.test_type.clone();
            let new_source = NewSource {
                name: args.name.clone(),
                description: args.description,
                active: Some(args.active),
                interval_seconds: Some(args.interval),
                test_type: Some(args.test_type),
                arguments: Some(serde_json::to_string(&arguments)?),
                site_id,
                company_id,
            };

            let created = create_source(&mut connection, new_source)?;
            println!("Created source '{}' (ID: {})", created.name, created.id.unwrap_or(0));
            println!("  Test Type: {}", test_type_str);
            if !arguments.is_empty() {
                println!("  Arguments:");
                for (key, value) in &arguments {
                    println!("    {}: {}", key, value);
                }
            }
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

            // Handle test_type validation if provided
            if let Some(ref test_type) = args.test_type {
                use neems_data::collectors::TestType;
                TestType::from_str(test_type)
                    .map_err(|e| format!("Invalid test type '{}': {}", test_type, e))?;
            }

            // Handle arguments updates
            let arguments = if args.clear_arguments {
                Some("{}".to_string())
            } else if !args.arguments.is_empty() {
                // Merge with existing arguments if no clear flag
                let mut current_args = match &existing.arguments {
                    Some(args_json) => {
                        serde_json::from_str::<std::collections::HashMap<String, String>>(args_json)
                            .unwrap_or_default()
                    }
                    None => std::collections::HashMap::new(),
                };

                // Add/update new arguments
                for (key, value) in args.arguments {
                    current_args.insert(key, value);
                }

                Some(serde_json::to_string(&current_args)?)
            } else {
                None
            };

            // Handle site_id updates
            let site_id = if args.clear_site_id {
                Some(None)
            } else if args.site_id.is_some() {
                Some(args.site_id)
            } else {
                None
            };

            // Handle company_id updates
            let company_id = if args.clear_company_id {
                Some(None)
            } else if args.company_id.is_some() {
                Some(args.company_id)
            } else {
                None
            };

            let updates = UpdateSource {
                name: args.new_name,
                description,
                active: args.active,
                interval_seconds: args.interval,
                last_run: None, // Don't modify last_run via CLI
                test_type: args.test_type,
                arguments,
                site_id,
                company_id,
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
