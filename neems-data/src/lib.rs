use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::env;
use std::error::Error;
use tokio::task;

pub mod collectors;
pub mod models;
pub mod schema;

pub use models::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub struct DataAggregator {
    database_url: String,
}

impl DataAggregator {
    pub fn new(database_path: Option<&str>) -> Self {
        let database_path = match database_path {
            Some(path) => path.to_string(),
            None => {
                env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string())
            }
        };
        let database_url = format!("sqlite://{}", database_path);

        Self { database_url }
    }

    pub fn establish_connection(&self) -> Result<SqliteConnection, Box<dyn Error>> {
        let mut connection = SqliteConnection::establish(&self.database_url)?;
        connection
            .run_pending_migrations(MIGRATIONS)
            .map_err(|e| format!("Error running migrations: {}", e))?;
        Ok(connection)
    }

    pub async fn start_aggregation(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let database_url = self.database_url.clone();

        task::spawn(async move {
            loop {
                match Self::collect_data(&database_url).await {
                    Ok(_) => println!("Data collection cycle completed"),
                    Err(e) => eprintln!("Error during data collection: {}", e),
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        })
        .await?;

        Ok(())
    }

    async fn collect_data(database_url: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        task::spawn_blocking({
            let database_url = database_url.to_string();
            move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut connection = SqliteConnection::establish(&database_url)?;

                // Example: Insert sample readings for testing
                // In practice, this would collect from actual data sources
                Self::collect_from_sources(&mut connection)?;

                Ok(())
            }
        })
        .await??;

        Ok(())
    }

    fn collect_from_sources(
        connection: &mut SqliteConnection,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        use schema::sources::dsl::*;

        // Get all active sources
        let active_sources: Vec<Source> = sources
            .filter(active.eq(true))
            .select(Source::as_select())
            .load(connection)?;

        for source in active_sources {
            if let Some(source_id) = source.id {
                // This is where you'd implement actual data collection per source
                // For now, just create a placeholder reading
                let sample_data = serde_json::json!({
                    "placeholder": true,
                    "source": source.name
                });

                let new_reading = NewReading::with_json_data(source_id, &sample_data)?;
                insert_reading(connection, new_reading)?;
            }
        }

        Ok(())
    }
}

/// Insert a single reading
pub fn insert_reading(
    connection: &mut SqliteConnection,
    reading: NewReading,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::readings;

    diesel::insert_into(readings::table)
        .values(&reading)
        .execute(connection)?;

    Ok(())
}

/// Insert multiple readings in a batch for better performance
pub fn insert_readings_batch(
    connection: &mut SqliteConnection,
    readings: Vec<NewReading>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::readings;

    diesel::insert_into(readings::table)
        .values(&readings)
        .execute(connection)?;

    Ok(())
}

/// Source Management Functions

/// Create a new data source
pub fn create_source(
    connection: &mut SqliteConnection,
    new_source: NewSource,
) -> Result<Source, Box<dyn Error>> {
    use schema::sources;

    diesel::insert_into(sources::table)
        .values(&new_source)
        .execute(connection)?;

    // Get the inserted source
    let source: Source = sources::table
        .order(sources::id.desc())
        .select(Source::as_select())
        .first(connection)?;

    Ok(source)
}

/// List all sources
pub fn list_sources(connection: &mut SqliteConnection) -> Result<Vec<Source>, Box<dyn Error>> {
    use schema::sources::dsl::*;

    let source_list = sources.select(Source::as_select()).load(connection)?;

    Ok(source_list)
}

/// Get a source by name
pub fn get_source_by_name(
    connection: &mut SqliteConnection,
    source_name: &str,
) -> Result<Option<Source>, Box<dyn Error>> {
    use schema::sources::dsl::*;

    let source = sources
        .filter(name.eq(source_name))
        .select(Source::as_select())
        .first(connection)
        .optional()?;

    Ok(source)
}

/// Update a source
pub fn update_source(
    connection: &mut SqliteConnection,
    source_id: i32,
    updates: UpdateSource,
) -> Result<Source, Box<dyn Error>> {
    use schema::sources::dsl::*;

    diesel::update(sources.filter(id.eq(source_id)))
        .set(&updates)
        .execute(connection)?;

    let updated_source = sources
        .filter(id.eq(source_id))
        .select(Source::as_select())
        .first(connection)?;

    Ok(updated_source)
}

/// Get recent readings for a source
pub fn get_recent_readings(
    connection: &mut SqliteConnection,
    src_id: i32,
    limit: i64,
) -> Result<Vec<Reading>, Box<dyn Error>> {
    use schema::readings::dsl::*;

    let recent_readings = readings
        .filter(source_id.eq(src_id))
        .order(timestamp.desc())
        .limit(limit)
        .select(Reading::as_select())
        .load(connection)?;

    Ok(recent_readings)
}

/// Read aggregated data - main interface for neems-api
pub fn read_aggregated_data(
    database_path: Option<&str>,
) -> Result<Vec<(Source, Vec<Reading>)>, Box<dyn Error>> {
    let aggregator = DataAggregator::new(database_path);
    let mut connection = aggregator.establish_connection()?;

    let sources = list_sources(&mut connection)?;
    let mut result = Vec::new();

    for source in sources {
        if let Some(source_id) = source.id {
            let readings = get_recent_readings(&mut connection, source_id, 10)?; // Last 10 readings
            result.push((source, readings));
        }
    }

    Ok(result)
}
