use neems_data::read_aggregated_data;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_path =
        env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string());

    println!("Reading aggregated data from: {}", database_path);

    match read_aggregated_data(Some(&database_path)) {
        Ok(data) => {
            println!("Found {} sources with data:", data.len());
            for (source, readings) in data {
                println!(
                    "\nSource: {} ({})",
                    source.name,
                    source.description.as_deref().unwrap_or("No description")
                );
                println!("  Active: {}", source.active);
                println!("  Recent readings ({}):", readings.len());

                for reading in readings.iter().take(3) {
                    match reading.parse_data() {
                        Ok(json_data) => {
                            println!("    {}: {}", reading.timestamp, json_data);
                        }
                        Err(e) => {
                            println!("    {}: Error parsing data: {}", reading.timestamp, e);
                        }
                    }
                }

                if readings.len() > 3 {
                    println!("    ... and {} more readings", readings.len() - 3);
                }
            }
        }
        Err(e) => {
            println!("Error reading aggregated data: {}", e);
        }
    }

    Ok(())
}
