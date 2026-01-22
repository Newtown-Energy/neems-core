mod client;
mod config;
mod server;

use clap::{Parser, Subcommand};
use config::SiteConfig;

#[derive(Parser)]
#[command(name = "modbus-poc")]
#[command(about = "Modbus TCP proof of concept for SEL-RTAC communication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a mock Modbus TCP server for testing
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "5502")]
        port: u16,
    },

    // =========================================================================
    // Raw register operations (address-based)
    // =========================================================================
    /// Read holding registers from a Modbus server (raw address mode)
    Read {
        /// Modbus server hostname or IP
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Modbus server port
        #[arg(short, long, default_value = "11502")]
        port: u16,

        /// Modbus unit ID (slave address)
        #[arg(short, long, default_value = "1")]
        unit: u8,

        /// Starting register address
        #[arg(short, long)]
        address: u16,

        /// Number of registers to read
        #[arg(short, long, default_value = "1")]
        count: u16,
    },

    /// Write a single holding register (raw address mode)
    Write {
        /// Modbus server hostname or IP
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Modbus server port
        #[arg(short, long, default_value = "11502")]
        port: u16,

        /// Modbus unit ID (slave address)
        #[arg(short, long, default_value = "1")]
        unit: u8,

        /// Register address to write
        #[arg(short, long)]
        address: u16,

        /// Value to write (0-65535)
        #[arg(short, long)]
        value: u16,
    },

    /// Write multiple holding registers (raw address mode)
    WriteMulti {
        /// Modbus server hostname or IP
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Modbus server port
        #[arg(short, long, default_value = "11502")]
        port: u16,

        /// Modbus unit ID (slave address)
        #[arg(short, long, default_value = "1")]
        unit: u8,

        /// Starting register address
        #[arg(short, long)]
        address: u16,

        /// Comma-separated values to write (e.g., "100,200,300")
        #[arg(short, long)]
        values: String,
    },

    /// Scan a range of registers to discover which ones are populated
    Scan {
        /// Modbus server hostname or IP
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Modbus server port
        #[arg(short, long, default_value = "11502")]
        port: u16,

        /// Modbus unit ID (slave address)
        #[arg(short, long, default_value = "1")]
        unit: u8,

        /// Starting register address
        #[arg(short, long, default_value = "0")]
        start: u16,

        /// Ending register address (inclusive)
        #[arg(short, long, default_value = "999")]
        end: u16,

        /// Number of registers to read per request
        #[arg(short, long, default_value = "100")]
        batch_size: u16,

        /// Register type to scan: "holding" or "input"
        #[arg(short, long, default_value = "holding")]
        register_type: String,
    },

    // =========================================================================
    // Configuration-driven operations (tag-based)
    // =========================================================================
    /// List all tags defined in the configuration
    ListTags {
        /// Path to the site configuration file (TOML)
        #[arg(short, long)]
        config: String,
    },

    /// Read a tag by name using configuration
    ReadTag {
        /// Path to the site configuration file (TOML)
        #[arg(short, long)]
        config: String,

        /// Tag name to read
        #[arg(short, long)]
        tag: String,
    },

    /// Write a tag by name using configuration (engineering value)
    WriteTag {
        /// Path to the site configuration file (TOML)
        #[arg(short, long)]
        config: String,

        /// Tag name to write
        #[arg(short, long)]
        tag: String,

        /// Engineering value to write
        #[arg(short, long)]
        value: f64,
    },

    /// Read all tags for an equipment ID
    ReadEquipment {
        /// Path to the site configuration file (TOML)
        #[arg(short, long)]
        config: String,

        /// Equipment ID to read tags for
        #[arg(short, long)]
        equipment: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { port } => {
            server::run_server(port).await?;
        }

        // Raw register operations
        Commands::Read {
            host,
            port,
            unit,
            address,
            count,
        } => {
            client::read_registers(&host, port, unit, address, count).await?;
        }
        Commands::Write {
            host,
            port,
            unit,
            address,
            value,
        } => {
            client::write_register(&host, port, unit, address, value).await?;
        }
        Commands::WriteMulti {
            host,
            port,
            unit,
            address,
            values,
        } => {
            let values: Vec<u16> = values
                .split(',')
                .map(|s| s.trim().parse::<u16>())
                .collect::<Result<Vec<_>, _>>()?;
            client::write_registers(&host, port, unit, address, &values).await?;
        }

        Commands::Scan {
            host,
            port,
            unit,
            start,
            end,
            batch_size,
            register_type,
        } => {
            client::scan_registers(&host, port, unit, start, end, batch_size, &register_type)
                .await?;
        }

        // Configuration-driven operations
        Commands::ListTags { config } => {
            let site_config = SiteConfig::load(&config)?;
            client::list_tags(&site_config);
        }
        Commands::ReadTag { config, tag } => {
            let site_config = SiteConfig::load(&config)?;
            client::read_tag(&site_config, &tag).await?;
        }
        Commands::WriteTag { config, tag, value } => {
            let site_config = SiteConfig::load(&config)?;
            client::write_tag(&site_config, &tag, value).await?;
        }
        Commands::ReadEquipment { config, equipment } => {
            let site_config = SiteConfig::load(&config)?;
            let results = client::read_equipment_tags(&site_config, &equipment).await?;

            println!();
            println!("Equipment '{}' readings:", equipment);
            println!("{:-<60}", "");
            for result in results {
                println!(
                    "  {}: {}{}",
                    result.tag_name,
                    result.engineering_value,
                    result.units.as_deref().map(|u| format!(" {}", u)).unwrap_or_default()
                );
            }
        }
    }

    Ok(())
}
