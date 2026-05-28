use std::io::{self, Write};

use clap::Subcommand;
use diesel::sqlite::SqliteConnection;
use neems_api::orm::{
    company::get_company_by_id,
    site::{
        SiteUpdate, delete_site, get_all_sites, get_site_by_company_and_name, get_site_by_id,
        get_sites_by_company, insert_site, update_site,
    },
};
use regex::Regex;

use crate::admin_cli::utils::resolve_company_id;

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
        #[arg(short = 'c', long = "company", help = "Filter by company ID or name")]
        company_id: Option<String>,
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
        #[arg(short, long, help = "Company ID or name")]
        company_id: String,
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
        #[arg(short = 'c', long = "company", help = "Filter by company ID or name")]
        company_id: Option<String>,
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
        #[arg(long, help = "New company ID or name")]
        company_id: Option<String>,
    },
}

pub fn handle_site_command_with_conn(
    conn: &mut SqliteConnection,
    action: SiteAction,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SiteAction::Ls { search_term, fixed_string, company_id } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            site_ls_impl(conn, search_term, fixed_string, resolved_company_id)?;
        }
        SiteAction::Add {
            name,
            address,
            latitude,
            longitude,
            company_id,
        } => {
            let resolved_company_id = resolve_company_id(conn, &company_id)?;
            site_add_impl(
                conn,
                name,
                address,
                latitude,
                longitude,
                resolved_company_id,
                admin_user_id,
            )?;
        }
        SiteAction::Rm {
            search_term,
            fixed_string,
            yes,
            company_id,
        } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            site_rm_impl(conn, search_term, fixed_string, yes, resolved_company_id, admin_user_id)?;
        }
        SiteAction::Edit {
            id,
            name,
            address,
            latitude,
            longitude,
            company_id,
        } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            site_edit_impl(
                conn,
                id,
                name,
                address,
                latitude,
                longitude,
                resolved_company_id,
                admin_user_id,
            )?;
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
            sites.into_iter().filter(|site| site.name.contains(&term)).collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            sites.into_iter().filter(|site| regex.is_match(&site.name)).collect::<Vec<_>>()
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
        println!("Coordinates: ({}, {})", existing_site.latitude, existing_site.longitude);
        return Ok(());
    }

    let created_site = insert_site(
        conn,
        name,
        address,
        latitude,
        longitude,
        company_id,
        120, // Default ramp duration
        Some(admin_user_id),
    )?;

    println!("Site created successfully!");
    println!("ID: {}", created_site.id);
    println!("Name: {}", created_site.name);
    println!("Address: {}", created_site.address);
    println!("Company ID: {}", created_site.company_id);
    println!("Coordinates: ({}, {})", created_site.latitude, created_site.longitude);

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
        sites.into_iter().filter(|site| regex.is_match(&site.name)).collect::<Vec<_>>()
    };

    if matching_sites.is_empty() {
        println!("No sites found matching the search term.");
        return Ok(());
    }

    println!("Found {} site(s) matching the search term:", matching_sites.len());
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
                errors
                    .push(format!("Failed to delete site {} (ID: {}): {}", site.name, site.id, e));
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
        SiteUpdate {
            name: new_name,
            address: new_address,
            latitude: new_latitude,
            longitude: new_longitude,
            company_id: new_company_id,
            ..Default::default()
        },
        Some(admin_user_id),
    )?;

    println!("Site updated successfully!");
    println!("ID: {}", updated_site.id);
    println!("Name: {}", updated_site.name);
    println!("Address: {}", updated_site.address);
    println!("Company ID: {}", updated_site.company_id);
    println!("Coordinates: ({}, {})", updated_site.latitude, updated_site.longitude);

    Ok(())
}

#[cfg(all(test, feature = "test-staging"))]
#[allow(unused_imports)]
mod tests {
    use neems_api::orm::{
        company::insert_company,
        site::{get_all_sites, get_site_by_id, get_sites_by_company, insert_site},
        testing::setup_test_db,
    };

    use super::*;

    #[test]
    fn test_handle_site_command_with_conn_ls() {
        let mut conn = setup_test_db();

        let action = SiteAction::Ls {
            search_term: None,
            fixed_string: false,
            company_id: None,
        };
        let result = handle_site_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_site_command_with_conn_add() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        let action = SiteAction::Add {
            name: "CLI Test Site".to_string(),
            address: "CLI Test Address".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            company_id: company.id.to_string(),
        };
        let result = handle_site_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());

        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found = sites.iter().any(|s| s.name == "CLI Test Site");
        assert!(found);
    }

    #[test]
    fn test_handle_site_command_with_conn_rm() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        insert_site(
            &mut conn,
            "Remove This Site".to_string(),
            "Address".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site");

        let action = SiteAction::Rm {
            search_term: "Remove This".to_string(),
            fixed_string: true,
            yes: true,
            company_id: None,
        };
        let result = handle_site_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());

        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found = sites.iter().any(|s| s.name == "Remove This Site");
        assert!(!found);
    }

    #[test]
    fn test_site_ls_impl_all() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        insert_site(
            &mut conn,
            "Site 1".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site 1");
        insert_site(
            &mut conn,
            "Site 2".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site 2");

        let result = site_ls_impl(&mut conn, None, false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_ls_impl_with_search() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        insert_site(
            &mut conn,
            "Main Office".to_string(),
            "123 Main St".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site 1");
        insert_site(
            &mut conn,
            "Branch Office".to_string(),
            "456 Branch Ave".to_string(),
            41.0,
            -75.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site 2");

        let result = site_ls_impl(&mut conn, Some("Main".to_string()), true, None);
        assert!(result.is_ok());

        let result = site_ls_impl(&mut conn, Some("^Branch".to_string()), false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_ls_impl_with_company_filter() {
        let mut conn = setup_test_db();

        let company1 = insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to create company 2");

        insert_site(
            &mut conn,
            "Site A".to_string(),
            "Address A".to_string(),
            40.0,
            -74.0,
            company1.id,
            120,
            Some(1),
        )
        .expect("Failed to create site A");
        insert_site(
            &mut conn,
            "Site B".to_string(),
            "Address B".to_string(),
            41.0,
            -75.0,
            company2.id,
            120,
            Some(1),
        )
        .expect("Failed to create site B");

        let result = site_ls_impl(&mut conn, None, false, Some(company1.id));
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_add_impl() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        let result = site_add_impl(
            &mut conn,
            "New Site".to_string(),
            "123 New St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            1,
        );
        assert!(result.is_ok());

        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found = sites.iter().any(|s| s.name == "New Site");
        assert!(found);
    }

    #[test]
    fn test_site_add_impl_duplicate_name_same_company() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        // Create first site
        let result = site_add_impl(
            &mut conn,
            "Duplicate Site".to_string(),
            "123 Original St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            1,
        );
        assert!(result.is_ok());

        // Try to create second site with same name in same company - should succeed
        // gracefully
        let result = site_add_impl(
            &mut conn,
            "Duplicate Site".to_string(),
            "456 Different St".to_string(),
            41.0,
            -75.0,
            company.id,
            1,
        );
        assert!(result.is_ok()); // Now handles duplicates gracefully

        // Verify there's still only one site with this name for this company
        let sites = get_sites_by_company(&mut conn, company.id).expect("Failed to get sites");
        let count = sites.iter().filter(|s| s.name == "Duplicate Site").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_site_rm_impl() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        insert_site(
            &mut conn,
            "Delete Me Site".to_string(),
            "Address".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site");
        insert_site(
            &mut conn,
            "Keep Me Site".to_string(),
            "Address".to_string(),
            41.0,
            -75.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site");

        let result = site_rm_impl(&mut conn, "Delete Me".to_string(), true, true, None, 1);
        assert!(result.is_ok());

        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        let found_deleted = sites.iter().any(|s| s.name == "Delete Me Site");
        assert!(!found_deleted);

        let found_kept = sites.iter().any(|s| s.name == "Keep Me Site");
        assert!(found_kept);
    }

    #[test]
    fn test_site_rm_impl_with_company_filter() {
        let mut conn = setup_test_db();

        let company1 = insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to create company 2");

        insert_site(
            &mut conn,
            "Test Site".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company1.id,
            120,
            Some(1),
        )
        .expect("Failed to create site 1");
        insert_site(
            &mut conn,
            "Test Site".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company2.id,
            120,
            Some(1),
        )
        .expect("Failed to create site 2");

        // Delete only from company1
        let result =
            site_rm_impl(&mut conn, "Test Site".to_string(), true, true, Some(company1.id), 1);
        assert!(result.is_ok());

        let sites = get_all_sites(&mut conn).expect("Failed to get sites");
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].company_id, company2.id);
    }

    #[test]
    fn test_site_edit_impl() {
        let mut conn = setup_test_db();

        let company1 = insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to create company 2");

        let site = insert_site(
            &mut conn,
            "Original Site".to_string(),
            "Original Address".to_string(),
            40.0,
            -74.0,
            company1.id,
            120,
            Some(1),
        )
        .expect("Failed to create site");

        // Edit name and address
        let result = site_edit_impl(
            &mut conn,
            site.id,
            Some("Updated Site".to_string()),
            Some("Updated Address".to_string()),
            None,
            None,
            None,
            1,
        );
        assert!(result.is_ok());

        let updated_site = get_site_by_id(&mut conn, site.id)
            .expect("Failed to get updated site")
            .expect("Site should exist");
        assert_eq!(updated_site.name, "Updated Site");
        assert_eq!(updated_site.address, "Updated Address");
        assert_eq!(updated_site.latitude, 40.0);
        assert_eq!(updated_site.longitude, -74.0);

        // Edit coordinates and company
        let result = site_edit_impl(
            &mut conn,
            site.id,
            None,
            None,
            Some(41.0),
            Some(-75.0),
            Some(company2.id),
            1,
        );
        assert!(result.is_ok());

        let updated_site = get_site_by_id(&mut conn, site.id)
            .expect("Failed to get updated site")
            .expect("Site should exist");
        assert_eq!(updated_site.latitude, 41.0);
        assert_eq!(updated_site.longitude, -75.0);
        assert_eq!(updated_site.company_id, company2.id);
    }

    #[test]
    fn test_site_edit_impl_nonexistent_site() {
        let mut conn = setup_test_db();

        let result = site_edit_impl(
            &mut conn,
            99999,
            Some("New Name".to_string()),
            None,
            None,
            None,
            None,
            1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_site_edit_impl_nonexistent_company() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "Address".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            Some(1),
        )
        .expect("Failed to create site");

        let result = site_edit_impl(&mut conn, site.id, None, None, None, None, Some(99999), 1);
        assert!(result.is_err());
    }
}
