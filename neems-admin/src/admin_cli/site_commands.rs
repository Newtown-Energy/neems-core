use clap::Subcommand;
use diesel::sqlite::SqliteConnection;
use neems_api::orm::company::get_company_by_id;
use neems_api::orm::site::{
    delete_site, get_all_sites, get_site_by_company_and_name, get_site_by_id, get_sites_by_company, insert_site, update_site,
};
use regex::Regex;
use std::io::{self, Write};

#[derive(Subcommand)]
pub enum SiteAction {
    #[command(about = "List sites, optionally filtered by search term")]
    Ls {
        #[arg(help = "Search term (regex by default, use -F for fixed string)")]
        search_term: Option<String>,
        #[arg(
            short = 'F',
            long = "fixed-string",
            help = "Treat search term as fixed string instead of regex"
        )]
        fixed_string: bool,
        #[arg(short = 'c', long = "company", help = "Filter by company ID")]
        company_id: Option<i32>,
    },
    #[command(about = "Add a new site")]
    Add {
        #[arg(short, long, help = "Site name")]
        name: String,
        #[arg(short, long, help = "Site address")]
        address: String,
        #[arg(long, help = "Latitude coordinate")]
        latitude: f64,
        #[arg(long, help = "Longitude coordinate")]
        longitude: f64,
        #[arg(short, long, help = "Company ID")]
        company_id: i32,
    },
    #[command(about = "Remove sites matching search term")]
    Rm {
        #[arg(
            help = "Search term to match sites for removal (regex by default, use -F for fixed string)"
        )]
        search_term: String,
        #[arg(
            short = 'F',
            long = "fixed-string",
            help = "Treat search term as fixed string instead of regex"
        )]
        fixed_string: bool,
        #[arg(short = 'y', long = "yes", help = "Skip confirmation prompt")]
        yes: bool,
        #[arg(short = 'c', long = "company", help = "Filter by company ID")]
        company_id: Option<i32>,
    },
    #[command(about = "Edit site fields")]
    Edit {
        #[arg(short, long, help = "Site ID to edit")]
        id: i32,
        #[arg(long, help = "New site name")]
        name: Option<String>,
        #[arg(long, help = "New site address")]
        address: Option<String>,
        #[arg(long, help = "New latitude coordinate")]
        latitude: Option<f64>,
        #[arg(long, help = "New longitude coordinate")]
        longitude: Option<f64>,
        #[arg(long, help = "New company ID")]
        company_id: Option<i32>,
    },
}

pub fn handle_site_command_with_conn(
    conn: &mut SqliteConnection,
    action: SiteAction,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SiteAction::Ls {
            search_term,
            fixed_string,
            company_id,
        } => {
            site_ls_impl(conn, search_term, fixed_string, company_id)?;
        }
        SiteAction::Add {
            name,
            address,
            latitude,
            longitude,
            company_id,
        } => {
            site_add_impl(conn, name, address, latitude, longitude, company_id, admin_user_id)?;
        }
        SiteAction::Rm {
            search_term,
            fixed_string,
            yes,
            company_id,
        } => {
            site_rm_impl(conn, search_term, fixed_string, yes, company_id, admin_user_id)?;
        }
        SiteAction::Edit {
            id,
            name,
            address,
            latitude,
            longitude,
            company_id,
        } => {
            site_edit_impl(conn, id, name, address, latitude, longitude, company_id, admin_user_id)?;
        }
    }
    Ok(())
}

pub fn site_ls_impl(
    conn: &mut SqliteConnection,
    search_term: Option<String>,
    fixed_string: bool,
    company_id: Option<i32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sites = if let Some(comp_id) = company_id {
        get_sites_by_company(conn, comp_id)?
    } else {
        get_all_sites(conn)?
    };

    let filtered_sites = if let Some(term) = search_term {
        if fixed_string {
            sites
                .into_iter()
                .filter(|site| site.name.contains(&term))
                .collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            sites
                .into_iter()
                .filter(|site| regex.is_match(&site.name))
                .collect::<Vec<_>>()
        }
    } else {
        sites
    };

    if filtered_sites.is_empty() {
        println!("No sites found.");
    } else {
        println!("Sites:");
        for site in filtered_sites {
            println!(
                "  ID: {}, Name: {}, Address: {}, Company ID: {}, Coords: ({}, {})",
                site.id, site.name, site.address, site.company_id, site.latitude, site.longitude
            );
        }
    }

    Ok(())
}

pub fn site_add_impl(
    conn: &mut SqliteConnection,
    name: String,
    address: String,
    latitude: f64,
    longitude: f64,
    company_id: i32,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if site already exists for this company
    if let Some(existing_site) = get_site_by_company_and_name(conn, company_id, &name)? {
        println!("Site already exists!");
        println!("ID: {}", existing_site.id);
        println!("Name: {}", existing_site.name);
        println!("Address: {}", existing_site.address);
        println!("Company ID: {}", existing_site.company_id);
        println!(
            "Coordinates: ({}, {})",
            existing_site.latitude, existing_site.longitude
        );
        return Ok(());
    }

    let created_site = insert_site(conn, name, address, latitude, longitude, company_id, Some(admin_user_id))?;

    println!("Site created successfully!");
    println!("ID: {}", created_site.id);
    println!("Name: {}", created_site.name);
    println!("Address: {}", created_site.address);
    println!("Company ID: {}", created_site.company_id);
    println!(
        "Coordinates: ({}, {})",
        created_site.latitude, created_site.longitude
    );

    Ok(())
}

pub fn site_rm_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
    company_id: Option<i32>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let sites = if let Some(comp_id) = company_id {
        get_sites_by_company(conn, comp_id)?
    } else {
        get_all_sites(conn)?
    };

    let matching_sites = if fixed_string {
        sites
            .into_iter()
            .filter(|site| site.name.contains(&search_term))
            .collect::<Vec<_>>()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", search_term, e))?;
        sites
            .into_iter()
            .filter(|site| regex.is_match(&site.name))
            .collect::<Vec<_>>()
    };

    if matching_sites.is_empty() {
        println!("No sites found matching the search term.");
        return Ok(());
    }

    println!(
        "Found {} site(s) matching the search term:",
        matching_sites.len()
    );
    for site in &matching_sites {
        println!(
            "  ID: {}, Name: {}, Address: {}, Company ID: {}",
            site.id, site.name, site.address, site.company_id
        );
    }

    if !yes {
        print!(
            "Are you sure you want to delete these {} site(s)? [y/N]: ",
            matching_sites.len()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    let mut deleted_count = 0;
    let mut errors = Vec::new();

    for site in matching_sites {
        match delete_site(conn, site.id, Some(admin_user_id)) {
            Ok(rows_affected) => {
                if rows_affected > 0 {
                    deleted_count += 1;
                    println!("Deleted site: {} (ID: {})", site.name, site.id);
                }
            }
            Err(e) => {
                errors.push(format!(
                    "Failed to delete site {} (ID: {}): {}",
                    site.name, site.id, e
                ));
            }
        }
    }

    println!("Successfully deleted {} site(s).", deleted_count);

    if !errors.is_empty() {
        println!("Errors encountered:");
        for error in errors {
            println!("  {}", error);
        }
        return Err("Some deletions failed".into());
    }

    Ok(())
}

pub fn site_edit_impl(
    conn: &mut SqliteConnection,
    site_id: i32,
    new_name: Option<String>,
    new_address: Option<String>,
    new_latitude: Option<f64>,
    new_longitude: Option<f64>,
    new_company_id: Option<i32>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if site exists
    let site = get_site_by_id(conn, site_id)?;
    if site.is_none() {
        return Err(format!("Site with ID {} does not exist", site_id).into());
    }

    // Check if any fields need updating
    if new_name.is_none()
        && new_address.is_none()
        && new_latitude.is_none()
        && new_longitude.is_none()
        && new_company_id.is_none()
    {
        println!(
            "No fields specified for update. Use --name, --address, --latitude, --longitude, or --company-id."
        );
        return Ok(());
    }

    // Validate company exists if specified
    if let Some(comp_id) = new_company_id {
        if get_company_by_id(conn, comp_id)?.is_none() {
            return Err(format!("Company with ID {} does not exist", comp_id).into());
        }
    }

    let updated_site = update_site(
        conn,
        site_id,
        new_name,
        new_address,
        new_latitude,
        new_longitude,
        new_company_id,
        Some(admin_user_id),
    )?;

    println!("Site updated successfully!");
    println!("ID: {}", updated_site.id);
    println!("Name: {}", updated_site.name);
    println!("Address: {}", updated_site.address);
    println!("Company ID: {}", updated_site.company_id);
    println!(
        "Coordinates: ({}, {})",
        updated_site.latitude, updated_site.longitude
    );

    Ok(())
}
