use std::{env, error::Error};

use chrono::{Duration, NaiveDateTime, Utc};
use clap::{Args, Parser, Subcommand};
use dotenvy::dotenv;
use neems_data::{
    DataAggregator, NewReading, NewSource, UpdateSource,
    collectors::data_sources::charging_state_with_level, create_source, delete_source,
    get_source_by_name, insert_readings_batch, list_sources, rtac::state::AlarmFlags,
    update_source,
};
use serde_json::json;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(name = "neems-data")]
#[command(about = "Data aggregation service and source management for NEEMS")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show extended version information
    #[arg(long, action = clap::ArgAction::SetTrue)]
    version_info: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the data monitoring and aggregation service
    Monitor {
        #[arg(
            short,
            long,
            help = "Enable verbose output showing data source polling"
        )]
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
        name: String,
    },
    /// Show details of a specific source
    Show {
        /// Name of the source to show
        name: String,
    },
    /// Seed plausible past SoC history for a site (demo data).
    ///
    /// Creates a `charging_state` source for the site if one doesn't
    /// already exist, then writes one reading per `interval-minutes`
    /// slot over the trailing `days`. Existing timestamps are skipped,
    /// so re-running is safe.
    SeedSocHistory(SeedSocHistoryArgs),
    /// Seed plausible past alarm history for a site (demo data).
    ///
    /// Creates an `alarm_status` source for the site if one doesn't
    /// already exist, then writes one reading per `interval-minutes`
    /// slot over the trailing `days`, each carrying an `alarm_registers`
    /// bitfield. A deterministic pattern toggles a handful of
    /// representative alarms on and off so the FDNY page shows real
    /// transitions. Existing timestamps are skipped, so re-running is
    /// safe.
    SeedAlarmHistory(SeedAlarmHistoryArgs),
}

#[derive(Args)]
struct SeedSocHistoryArgs {
    /// Site ID to seed.
    #[arg(long)]
    site_id: i32,
    /// How many days of history to backfill (default 14).
    #[arg(long, default_value = "14")]
    days: u32,
    /// Cadence in minutes between samples (default 6).
    #[arg(long, default_value = "6")]
    interval_minutes: u32,
}

#[derive(Args)]
struct SeedAlarmHistoryArgs {
    /// Site ID to seed.
    #[arg(long)]
    site_id: i32,
    /// How many days of history to backfill (default 14).
    #[arg(long, default_value = "14")]
    days: u32,
    /// Cadence in minutes between samples (default 6).
    #[arg(long, default_value = "6")]
    interval_minutes: u32,
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
    let pos = s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
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

    // Initialize tracing so the RTAC collector / worker / control logic logs are
    // visible. Honors RUST_LOG; defaults to info.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let database_path =
        env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string());
    // DataAggregator::new prepends `sqlite://`, so strip any leading
    // scheme from the env var to avoid a doubled prefix
    // (`sqlite://sqlite:///path/...`) on hosts that ship the URL with
    // the scheme already attached.
    let database_path = database_path
        .strip_prefix("sqlite://")
        .map(|s| s.to_string())
        .unwrap_or(database_path);

    let aggregator = DataAggregator::new(Some(&database_path));
    let mut connection = aggregator
        .establish_connection()
        .map_err(|e| format!("Failed to establish database connection: {}", e))?;

    let cli = Cli::parse();

    // Handle --version-info flag
    if cli.version_info {
        println!("neems-data {}", built_info::PKG_VERSION);
        println!("Built: {}", built_info::BUILT_TIME_UTC);
        if let Some(commit) = built_info::GIT_COMMIT_HASH {
            println!("Git commit: {}", commit);
        }
        return Ok(());
    }

    match cli.command {
        Some(Commands::Monitor { verbose }) => {
            println!("Starting neems-data aggregator v{}", built_info::PKG_VERSION);
            println!("Built: {}", built_info::BUILT_TIME_UTC);
            if let Some(commit) = built_info::GIT_COMMIT_HASH {
                println!("Git commit: {}", commit);
            }
            println!("Database path: {}", database_path);
            if verbose {
                println!("Verbose mode enabled - will show data source polling details");
            }

            println!("Starting data aggregation process...");
            aggregator.start_aggregation(verbose).await?;
        }
        Some(Commands::List) => {
            let sources = list_sources(&mut connection)?;
            if sources.is_empty() {
                println!("No sources found.");
            } else {
                println!(
                    "{:<4} {:<20} {:<15} {:<15} {:<8} {:<8} {:<8} {:<20} Description",
                    "ID", "Name", "Test Type", "Arguments", "Active", "Site", "Company", "Last Run"
                );
                println!("{}", "-".repeat(120));
                for source in sources {
                    let last_run = source
                        .last_run
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string());

                    let test_type = source.test_type.as_deref().unwrap_or("(legacy)");
                    let arguments = match &source.arguments {
                        Some(args_json) => {
                            match serde_json::from_str::<std::collections::HashMap<String, String>>(
                                args_json,
                            ) {
                                Ok(args) if args.is_empty() => "{}".to_string(),
                                Ok(args) => {
                                    let formatted: Vec<String> =
                                        args.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
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

                    println!(
                        "{:<4} {:<20} {:<15} {:<15} {:<8} {:<8} {:<8} {:<20} {}",
                        source.id.unwrap_or(0),
                        source.name,
                        test_type,
                        args_display,
                        source.active,
                        source.site_id.map(|id| id.to_string()).unwrap_or_else(|| "-".to_string()),
                        source
                            .company_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        last_run,
                        source.description.unwrap_or_else(|| "".to_string())
                    );
                }
            }
        }
        Some(Commands::Show { name }) => {
            match get_source_by_name(&mut connection, &name)? {
                Some(source) => {
                    println!("Source Details:");
                    println!("  ID: {}", source.id.unwrap_or(0));
                    println!("  Name: {}", source.name);
                    println!("  Test Type: {}", source.test_type.as_deref().unwrap_or("(legacy)"));

                    // Display arguments nicely
                    match &source.arguments {
                        Some(args_json) => {
                            match serde_json::from_str::<std::collections::HashMap<String, String>>(
                                args_json,
                            ) {
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

                    println!(
                        "  Description: {}",
                        source.description.unwrap_or_else(|| "(none)".to_string())
                    );
                    println!("  Active: {}", source.active);
                    println!("  Interval: {} seconds", source.interval_seconds);
                    println!("  Created: {}", source.created_at.format("%Y-%m-%d %H:%M:%S"));
                    println!("  Updated: {}", source.updated_at.format("%Y-%m-%d %H:%M:%S"));
                    println!(
                        "  Last Run: {}",
                        source
                            .last_run
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "Never".to_string())
                    );
                    println!(
                        "  Site ID: {}",
                        source
                            .site_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "(none)".to_string())
                    );
                    println!(
                        "  Company ID: {}",
                        source
                            .company_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "(none)".to_string())
                    );
                }
                None => {
                    eprintln!("Error: Source '{}' not found.", name);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Add(args)) => {
            // Check if source already exists
            if get_source_by_name(&mut connection, &args.name)?.is_some() {
                eprintln!("Error: Source '{}' already exists.", args.name);
                std::process::exit(1);
            }

            // Validate test type
            use std::str::FromStr;

            use neems_data::collectors::TestType;
            let _test_type = TestType::from_str(&args.test_type)
                .map_err(|e| format!("Invalid test type '{}': {}", args.test_type, e))?;

            // Convert arguments Vec to HashMap
            let mut arguments = std::collections::HashMap::new();
            for (key, value) in args.arguments {
                arguments.insert(key, value);
            }

            // Use environment variables for defaults if not provided
            let site_id = args
                .site_id
                .or_else(|| env::var("NEEMS_DEFAULT_SITE").ok().and_then(|s| s.parse().ok()));
            let company_id = args
                .company_id
                .or_else(|| env::var("NEEMS_DEFAULT_COMPANY").ok().and_then(|s| s.parse().ok()));

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
        Some(Commands::Edit(args)) => {
            // Check if source exists
            let existing = match get_source_by_name(&mut connection, &args.name)? {
                Some(source) => source,
                None => {
                    eprintln!("Error: Source '{}' not found.", args.name);
                    std::process::exit(1);
                }
            };

            let source_id = existing.id.expect("source loaded from database is missing its id");

            let description = if args.clear_description {
                Some(None)
            } else if args.description.is_some() {
                Some(args.description)
            } else {
                None
            };

            // Handle test_type validation if provided
            if let Some(ref test_type) = args.test_type {
                use std::str::FromStr;

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
        Some(Commands::Remove { name }) => {
            // Check if source exists
            let existing = match get_source_by_name(&mut connection, &name)? {
                Some(source) => source,
                None => {
                    eprintln!("Error: Source '{}' not found.", name);
                    std::process::exit(1);
                }
            };

            let source_id = existing.id.expect("source loaded from database is missing its id");

            // Delete the source
            let deleted_count = delete_source(&mut connection, source_id)?;

            if deleted_count > 0 {
                println!("Removed source '{}'", name);
            } else {
                eprintln!("Error: Failed to remove source '{}'", name);
                std::process::exit(1);
            }
        }
        Some(Commands::SeedSocHistory(args)) => {
            seed_soc_history(&mut connection, args)?;
        }
        Some(Commands::SeedAlarmHistory(args)) => {
            seed_alarm_history(&mut connection, args)?;
        }
        None => {
            eprintln!("No command provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Backfill plausible past SoC readings for the given site.
///
/// Idempotent: collects existing reading timestamps for the source
/// up-front and only writes slots that aren't already present.
fn seed_soc_history(
    conn: &mut diesel::SqliteConnection,
    args: SeedSocHistoryArgs,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use diesel::prelude::*;
    use neems_data::schema::{readings, sources};

    if args.interval_minutes == 0 {
        return Err("--interval-minutes must be > 0".into());
    }

    // Ensure a charging_state source exists for this site. Reuse the
    // existing one if present; otherwise create a deterministic name so
    // re-runs find the same row.
    let existing_source: Option<(Option<i32>, String)> = sources::table
        .filter(sources::site_id.eq(args.site_id))
        .filter(sources::test_type.eq("charging_state"))
        .select((sources::id, sources::name))
        .first(conn)
        .optional()?;

    let (source_id, source_name) = match existing_source {
        Some((Some(id), name)) => (id, name),
        Some((None, name)) => {
            return Err(format!("source '{}' has NULL id (corrupt row?)", name).into());
        }
        None => {
            let name = format!("soc_history_site_{}", args.site_id);
            let new_source = NewSource {
                name: name.clone(),
                description: Some(format!("Demo SoC history for site {} (seeded)", args.site_id)),
                active: Some(false), // seed-only; not polled live
                interval_seconds: Some((args.interval_minutes as i32) * 60),
                test_type: Some("charging_state".to_string()),
                arguments: Some("{}".to_string()),
                site_id: Some(args.site_id),
                company_id: None,
            };
            let created = create_source(conn, new_source)?;
            let id = created.id.ok_or("create_source returned a row with no id")?;
            println!("Created source '{}' (ID {})", created.name, id);
            (id, created.name)
        }
    };

    // Build the slot grid (oldest → newest, top of the minute).
    let interval = Duration::minutes(args.interval_minutes as i64);
    let end = {
        let now = Utc::now().naive_utc();
        // Snap to the most recent slot boundary so re-runs hit the same
        // timestamps each time.
        let secs = now.and_utc().timestamp();
        let slot = interval.num_seconds();
        let snapped = secs - (secs % slot);
        chrono::DateTime::from_timestamp(snapped, 0)
            .ok_or("failed to snap end timestamp")?
            .naive_utc()
    };
    let start = end - Duration::days(args.days as i64);
    // Loop below is inclusive on both endpoints, so the slot count is
    // span/interval + 1 (e.g. a 6-min span at 6-min cadence yields 2
    // slots, not 1). Without the +1 the "already present" math at the
    // end can underflow when the seeder fully populates the window.
    let total_slots = ((end - start).num_seconds() / interval.num_seconds()) as usize + 1;

    // Idempotency: pull existing timestamps in the window and skip them.
    let existing: std::collections::HashSet<NaiveDateTime> = readings::table
        .filter(readings::source_id.eq(source_id))
        .filter(readings::timestamp.ge(start))
        .filter(readings::timestamp.le(end))
        .select(readings::timestamp)
        .load::<NaiveDateTime>(conn)?
        .into_iter()
        .collect();

    let mut batch: Vec<NewReading> = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        if !existing.contains(&cursor) {
            let utc = cursor.and_utc();
            let (state, level) = charging_state_with_level(utc, "default");
            let blob = json!({
                "source_id": source_id,
                "battery_id": "default",
                "state": state,
                "level": level,
                "timestamp_utc": utc.to_rfc3339(),
                "seeded": true,
            })
            .to_string();
            batch.push(NewReading {
                source_id,
                timestamp: Some(cursor),
                data: blob,
                quality_flags: Some(0),
            });
        }
        cursor += interval;
    }

    let to_write = batch.len();
    if to_write == 0 {
        println!(
            "Source '{}' (ID {}) already has all {} slots seeded — nothing to do.",
            source_name, source_id, total_slots
        );
        return Ok(());
    }

    // Insert in chunks so SQLite doesn't choke on a giant single statement.
    for chunk in batch.chunks(500) {
        insert_readings_batch(conn, chunk.to_vec())?;
    }

    println!(
        "Seeded {} new SoC readings ({} already present) into source '{}' (ID {}) for site {}.",
        to_write,
        total_slots - to_write,
        source_name,
        source_id,
        args.site_id
    );
    Ok(())
}

/// Deterministic demo alarm state for a given instant.
///
/// Each tuple is `(alarm_num, period_minutes, active_minutes, phase_minutes)`:
/// the alarm is active when the time-of-window position is within the first
/// `active_minutes` of each `period_minutes` cycle. The chosen alarms span
/// several zones and severities so the FDNY timeline has variety, and the
/// long periods keep transitions sparse (a handful per alarm per week) rather
/// than flapping every sample.
fn seeded_alarm_flags(utc: chrono::DateTime<Utc>) -> AlarmFlags {
    const PATTERN: &[(u16, i64, i64, i64)] = &[
        (1, 1440, 90, 0),       // loss_fiber (L3) — ~daily, 90 min
        (203, 2880, 180, 600),  // meter_loss_of_comms (L5) — every 2 days, 3 h
        (301, 720, 60, 200),    // t1_temp_alarm (L4) — twice daily, 1 h
        (104, 4320, 240, 1000), // estop (L2, critical) — every 3 days, 4 h
        (7, 5760, 30, 2500),    // intruder_detected (L5) — every 4 days, 30 min
    ];
    let t_min = utc.timestamp() / 60;
    let mut flags = AlarmFlags::default();
    for &(num, period, active, phase) in PATTERN {
        let pos = ((t_min - phase) % period + period) % period;
        if pos < active {
            flags.set_alarm_num(num, true);
        }
    }
    flags
}

/// Backfill plausible past alarm readings for the given site.
///
/// Idempotent in the same way as [`seed_soc_history`]: existing reading
/// timestamps for the source are skipped, so re-running only fills gaps.
fn seed_alarm_history(
    conn: &mut diesel::SqliteConnection,
    args: SeedAlarmHistoryArgs,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use diesel::prelude::*;
    use neems_data::schema::{readings, sources};

    if args.interval_minutes == 0 {
        return Err("--interval-minutes must be > 0".into());
    }

    // Ensure an alarm_status source exists for this site. Reuse the existing
    // one if present; otherwise create a deterministic name so re-runs find
    // the same row.
    let existing_source: Option<(Option<i32>, String)> = sources::table
        .filter(sources::site_id.eq(args.site_id))
        .filter(sources::test_type.eq("alarm_status"))
        .select((sources::id, sources::name))
        .first(conn)
        .optional()?;

    let (source_id, source_name) = match existing_source {
        Some((Some(id), name)) => (id, name),
        Some((None, name)) => {
            return Err(format!("source '{}' has NULL id (corrupt row?)", name).into());
        }
        None => {
            let name = format!("alarm_history_site_{}", args.site_id);
            let new_source = NewSource {
                name: name.clone(),
                description: Some(format!("Demo alarm history for site {} (seeded)", args.site_id)),
                active: Some(false), // seed-only; not polled live
                interval_seconds: Some((args.interval_minutes as i32) * 60),
                test_type: Some("alarm_status".to_string()),
                arguments: Some("{}".to_string()),
                site_id: Some(args.site_id),
                company_id: None,
            };
            let created = create_source(conn, new_source)?;
            let id = created.id.ok_or("create_source returned a row with no id")?;
            println!("Created source '{}' (ID {})", created.name, id);
            (id, created.name)
        }
    };

    // Build the slot grid (oldest → newest, top of the minute), snapping the
    // end to the most recent slot boundary so re-runs hit the same timestamps.
    let interval = Duration::minutes(args.interval_minutes as i64);
    let end = {
        let now = Utc::now().naive_utc();
        let secs = now.and_utc().timestamp();
        let slot = interval.num_seconds();
        let snapped = secs - (secs % slot);
        chrono::DateTime::from_timestamp(snapped, 0)
            .ok_or("failed to snap end timestamp")?
            .naive_utc()
    };
    let start = end - Duration::days(args.days as i64);
    let total_slots = ((end - start).num_seconds() / interval.num_seconds()) as usize + 1;

    // Idempotency: pull existing timestamps in the window and skip them.
    let existing: std::collections::HashSet<NaiveDateTime> = readings::table
        .filter(readings::source_id.eq(source_id))
        .filter(readings::timestamp.ge(start))
        .filter(readings::timestamp.le(end))
        .select(readings::timestamp)
        .load::<NaiveDateTime>(conn)?
        .into_iter()
        .collect();

    let mut batch: Vec<NewReading> = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        if !existing.contains(&cursor) {
            let utc = cursor.and_utc();
            let registers = seeded_alarm_flags(utc).to_registers();
            let blob = json!({
                "source_id": source_id,
                "alarm_registers": registers.to_vec(),
                "timestamp_utc": utc.to_rfc3339(),
                "seeded": true,
            })
            .to_string();
            batch.push(NewReading {
                source_id,
                timestamp: Some(cursor),
                data: blob,
                quality_flags: Some(0),
            });
        }
        cursor += interval;
    }

    let to_write = batch.len();
    if to_write == 0 {
        println!(
            "Source '{}' (ID {}) already has all {} slots seeded — nothing to do.",
            source_name, source_id, total_slots
        );
        return Ok(());
    }

    // Insert in chunks so SQLite doesn't choke on a giant single statement.
    for chunk in batch.chunks(500) {
        insert_readings_batch(conn, chunk.to_vec())?;
    }

    println!(
        "Seeded {} new alarm readings ({} already present) into source '{}' (ID {}) for site {}.",
        to_write,
        total_slots - to_write,
        source_name,
        source_id,
        args.site_id
    );
    Ok(())
}
