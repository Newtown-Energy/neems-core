use dotenvy::dotenv;
use neems_data::{create_source, DataAggregator, NewSource};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();

    let database_path =
        env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string());

    println!("Setting up data sources...");
    println!("Database path: {}", database_path);

    let aggregator = DataAggregator::new(Some(&database_path));
    let mut connection = aggregator.establish_connection()?;

    // Define the data sources we want to collect from
    let sources = vec![
        NewSource {
            name: "current_time".to_string(),
            description: Some("Current UTC timestamp and unix timestamp".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "ping_localhost".to_string(),
            description: Some("Average ping time for 3 round trips to localhost".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "random_digits".to_string(),
            description: Some("Random integers, floats, and bytes".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "database_modtime".to_string(),
            description: Some("Modification time of the database file".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "database_sha1".to_string(),
            description: Some("SHA1 hash of the database file".to_string()),
            active: Some(true),
        },
    ];

    for new_source in sources {
        match neems_data::get_source_by_name(&mut connection, &new_source.name)? {
            Some(existing) => {
                println!("Source '{}' already exists (ID: {:?})", new_source.name, existing.id);
            }
            None => {
                let created = create_source(&mut connection, new_source.clone())?;
                println!("Created source '{}' (ID: {:?})", created.name, created.id);
            }
        }
    }

    println!("Data source setup complete!");
    Ok(())
}
