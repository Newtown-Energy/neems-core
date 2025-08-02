use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use neems_data::{create_source, NewSource};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    let database_path = env::var("SITE_DATABASE_URL")
        .unwrap_or_else(|_| "site-data.sqlite".to_string());
    let database_url = format!("sqlite://{}", database_path);
    
    let mut connection = SqliteConnection::establish(&database_url)?;
    
    println!("Setting up sample data sources...");
    
    let sample_sources = vec![
        NewSource {
            name: "temperature_sensor_01".to_string(),
            description: Some("Main building temperature sensor".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "humidity_sensor_01".to_string(),
            description: Some("Main building humidity sensor".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "power_meter_01".to_string(),
            description: Some("Main electrical panel power meter".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "pressure_sensor_01".to_string(),
            description: Some("HVAC system pressure sensor".to_string()),
            active: Some(true),
        },
        NewSource {
            name: "flow_meter_01".to_string(),
            description: Some("Water flow meter - main line".to_string()),
            active: Some(true),
        },
    ];
    
    for source in sample_sources {
        match create_source(&mut connection, source.clone()) {
            Ok(created) => println!("Created source: {} (ID: {:?})", created.name, created.id),
            Err(e) => println!("Error creating source {}: {}", source.name, e),
        }
    }
    
    println!("Sample sources setup complete!");
    
    Ok(())
}