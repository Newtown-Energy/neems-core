use dotenvy::dotenv;
use neems_data::{DataAggregator, NewSource, create_source};
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenv().ok();

    let database_path =
        env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string());

    println!("Setting up data sources...");
    println!("Database path: {}", database_path);

    let aggregator = DataAggregator::new(Some(&database_path));
    let mut connection = aggregator
        .establish_connection()
        .map_err(|e| e.to_string())?;

    // Define the data sources we want to collect from
    let mut sources = vec![
        NewSource {
            name: "current_time".to_string(),
            description: Some("Current UTC timestamp and unix timestamp".to_string()),
            active: Some(true),
            interval_seconds: Some(1), // Every second
        },
        NewSource {
            name: "ping_localhost".to_string(),
            description: Some("Ping statistics for localhost (127.0.0.1)".to_string()),
            active: Some(true),
            interval_seconds: Some(5), // Every 5 seconds
        },
        NewSource {
            name: "random_digits".to_string(),
            description: Some("Random integers, floats, and bytes".to_string()),
            active: Some(true),
            interval_seconds: Some(2), // Every 2 seconds
        },
        NewSource {
            name: "database_modtime".to_string(),
            description: Some("Modification time of the database file".to_string()),
            active: Some(true),
            interval_seconds: Some(10), // Every 10 seconds
        },
        NewSource {
            name: "database_sha1".to_string(),
            description: Some("SHA1 hash of the database file".to_string()),
            active: Some(true),
            interval_seconds: Some(30), // Every 30 seconds (expensive operation)
        },
        NewSource {
            name: "charging_state".to_string(),
            description: Some(
                "Calculates the current charging state (charging, discharging, hold)".to_string(),
            ),
            active: Some(true),
            interval_seconds: Some(1), // Every second
        },
        NewSource {
            name: "time_sleep_3".to_string(),
            description: Some(
                "Times how long it takes to run 'time sleep 3' command - slow 3+ second task".to_string(),
            ),
            active: Some(true),
            interval_seconds: Some(30), // Every 30 seconds (very slow task)
        },
    ];

    // Add battery-specific charging state sources
    let batteries = vec!["battery_1", "battery_2", "battery_3"];
    for battery_id in batteries {
        sources.push(NewSource {
            name: format!("charging_state_{}", battery_id),
            description: Some(format!(
                "Calculates the current charging state (charging, discharging, hold) for {}",
                battery_id
            )),
            active: Some(true),
            interval_seconds: Some(1), // Every second
        });
    }

    // Examples of additional sources you can add:
    // 
    // Additional ping sources (target extracted from name after "ping_"):
    // sources.push(NewSource {
    //     name: "ping_google.com".to_string(),
    //     description: Some("Ping statistics for Google".to_string()),
    //     active: Some(true),
    //     interval_seconds: Some(10), // Every 10 seconds
    // });
    //
    // sources.push(NewSource {
    //     name: "ping_8.8.8.8".to_string(), 
    //     description: Some("Ping statistics for Google DNS".to_string()),
    //     active: Some(true),
    //     interval_seconds: Some(5), // Every 5 seconds
    // });

    for new_source in sources {
        match neems_data::get_source_by_name(&mut connection, &new_source.name)
            .map_err(|e| e.to_string())?
        {
            Some(existing) => {
                println!(
                    "Source '{}' already exists (ID: {:?})",
                    new_source.name, existing.id
                );
            }
            None => {
                let created = create_source(&mut connection, new_source.clone())
                    .map_err(|e| e.to_string())?;
                println!("Created source '{}' (ID: {:?})", created.name, created.id);
            }
        }
    }

    println!("Data source setup complete!");
    Ok(())
}
