use std::io::{self, Write};

use clap::Subcommand;
use diesel::sqlite::SqliteConnection;
use neems_api::{
    models::DeviceInput,
    orm::{
        company::get_company_by_id,
        device::{
            delete_device, get_all_devices, get_device_by_id, get_device_by_site_and_name,
            get_devices_by_company, get_devices_by_site, insert_device, update_device,
        },
        site::get_site_by_id,
    },
};
use regex::Regex;

use crate::admin_cli::utils::resolve_company_id;

#[derive(Subcommand)]
pub enum DeviceAction {
    #[command(about = "List devices, optionally filtered by search term")]
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
        #[arg(short = 's', long = "site", help = "Filter by site ID")]
        site_id: Option<i32>,
    },
    #[command(about = "Add a new device")]
    Add {
        #[arg(short, long, help = "Device name (defaults to type if not provided)")]
        name: Option<String>,
        #[arg(short = 'd', long, help = "Device description")]
        description: Option<String>,
        #[arg(short = 't', long = "type", help = "Device type")]
        type_: String,
        #[arg(short = 'm', long, help = "Device model")]
        model: String,
        #[arg(long, help = "Serial number")]
        serial: Option<String>,
        #[arg(short = 'i', long = "ip", help = "IP address")]
        ip_address: Option<String>,
        #[arg(long, help = "Install date (YYYY-MM-DD HH:MM:SS)")]
        install_date: Option<String>,
        #[arg(short = 'c', long = "company", help = "Company ID or name")]
        company_id: String,
        #[arg(short = 's', long = "site", help = "Site ID")]
        site_id: i32,
    },
    #[command(about = "Remove devices matching search term")]
    Rm {
        #[arg(
            help = "Search term to match devices for removal (regex by default, use -F for fixed string)"
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
        #[arg(short = 's', long = "site", help = "Filter by site ID")]
        site_id: Option<i32>,
    },
    #[command(about = "Edit device fields")]
    Edit {
        #[arg(short, long, help = "Device ID to edit")]
        id: i32,
        #[arg(long, help = "New device name")]
        name: Option<String>,
        #[arg(long, help = "New device description (use empty string to clear)")]
        description: Option<String>,
        #[arg(long = "type", help = "New device type")]
        type_: Option<String>,
        #[arg(long, help = "New device model")]
        model: Option<String>,
        #[arg(long, help = "New serial number (use empty string to clear)")]
        serial: Option<String>,
        #[arg(long = "ip", help = "New IP address (use empty string to clear)")]
        ip_address: Option<String>,
        #[arg(
            long,
            help = "New install date (YYYY-MM-DD HH:MM:SS or empty to clear)"
        )]
        install_date: Option<String>,
        #[arg(long = "company", help = "New company ID or name")]
        company_id: Option<String>,
        #[arg(long = "site", help = "New site ID")]
        site_id: Option<i32>,
    },
}

pub fn handle_device_command_with_conn(
    conn: &mut SqliteConnection,
    action: DeviceAction,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DeviceAction::Ls {
            search_term,
            fixed_string,
            company_id,
            site_id,
        } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            device_ls_impl(conn, search_term, fixed_string, resolved_company_id, site_id)?;
        }
        DeviceAction::Add {
            name,
            description,
            type_,
            model,
            serial,
            ip_address,
            install_date,
            company_id,
            site_id,
        } => {
            let install_date_parsed = if let Some(date_str) = install_date {
                if date_str.is_empty() {
                    None
                } else {
                    Some(
                        chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S")
                            .map_err(|e| {
                                format!("Invalid date format: {}. Use YYYY-MM-DD HH:MM:SS", e)
                            })?,
                    )
                }
            } else {
                None
            };

            let resolved_company_id = resolve_company_id(conn, &company_id)?;
            let device_input = DeviceInput {
                name,
                description,
                type_,
                model,
                serial,
                ip_address,
                install_date: install_date_parsed,
                company_id: resolved_company_id,
                site_id,
            };
            device_add_impl(conn, device_input, admin_user_id)?;
        }
        DeviceAction::Rm {
            search_term,
            fixed_string,
            yes,
            company_id,
            site_id,
        } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            device_rm_impl(
                conn,
                search_term,
                fixed_string,
                yes,
                resolved_company_id,
                site_id,
                admin_user_id,
            )?;
        }
        DeviceAction::Edit {
            id,
            name,
            description,
            type_,
            model,
            serial,
            ip_address,
            install_date,
            company_id,
            site_id,
        } => {
            let resolved_company_id = if let Some(company_str) = company_id {
                Some(resolve_company_id(conn, &company_str)?)
            } else {
                None
            };
            device_edit_impl(
                conn,
                id,
                name,
                description,
                type_,
                model,
                serial,
                ip_address,
                install_date,
                resolved_company_id,
                site_id,
                admin_user_id,
            )?;
        }
    }
    Ok(())
}

pub fn device_ls_impl(
    conn: &mut SqliteConnection,
    search_term: Option<String>,
    fixed_string: bool,
    company_id: Option<i32>,
    site_id: Option<i32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let devices = if let Some(site) = site_id {
        get_devices_by_site(conn, site)?
    } else if let Some(comp) = company_id {
        get_devices_by_company(conn, comp)?
    } else {
        get_all_devices(conn)?
    };

    let filtered_devices = if let Some(term) = search_term {
        if fixed_string {
            devices
                .into_iter()
                .filter(|device| {
                    device.name.contains(&term)
                        || device.type_.contains(&term)
                        || device.model.contains(&term)
                })
                .collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            devices
                .into_iter()
                .filter(|device| {
                    regex.is_match(&device.name)
                        || regex.is_match(&device.type_)
                        || regex.is_match(&device.model)
                })
                .collect::<Vec<_>>()
        }
    } else {
        devices
    };

    if filtered_devices.is_empty() {
        println!("No devices found.");
    } else {
        println!("Devices:");
        for device in filtered_devices {
            println!(
                "  ID: {}, Name: {}, Type: {}, Model: {}, Company ID: {}, Site ID: {}",
                device.id,
                device.name,
                device.type_,
                device.model,
                device.company_id,
                device.site_id
            );
            if let Some(desc) = &device.description {
                println!("    Description: {}", desc);
            }
            if let Some(serial) = &device.serial {
                println!("    Serial: {}", serial);
            }
            if let Some(ip) = &device.ip_address {
                println!("    IP Address: {}", ip);
            }
            if let Some(date) = &device.install_date {
                println!("    Install Date: {}", date);
            }
        }
    }

    Ok(())
}

pub fn device_add_impl(
    conn: &mut SqliteConnection,
    device_input: DeviceInput,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate company exists
    if get_company_by_id(conn, device_input.company_id)?.is_none() {
        return Err(format!("Company with ID {} does not exist", device_input.company_id).into());
    }

    // Validate site exists
    if get_site_by_id(conn, device_input.site_id)?.is_none() {
        return Err(format!("Site with ID {} does not exist", device_input.site_id).into());
    }

    // Check if device already exists for this site
    let device_name = device_input.name.clone().unwrap_or_else(|| device_input.type_.clone());
    if let Some(existing_device) =
        get_device_by_site_and_name(conn, device_input.site_id, &device_name)?
    {
        println!("Device already exists!");
        println!("ID: {}", existing_device.id);
        println!("Name: {}", existing_device.name);
        println!("Type: {}", existing_device.type_);
        println!("Model: {}", existing_device.model);
        println!("Company ID: {}", existing_device.company_id);
        println!("Site ID: {}", existing_device.site_id);
        return Ok(());
    }

    let created_device = insert_device(conn, device_input, Some(admin_user_id))?;

    println!("Device created successfully!");
    println!("ID: {}", created_device.id);
    println!("Name: {}", created_device.name);
    println!("Type: {}", created_device.type_);
    println!("Model: {}", created_device.model);
    println!("Company ID: {}", created_device.company_id);
    println!("Site ID: {}", created_device.site_id);
    if let Some(desc) = &created_device.description {
        println!("Description: {}", desc);
    }
    if let Some(serial) = &created_device.serial {
        println!("Serial: {}", serial);
    }
    if let Some(ip) = &created_device.ip_address {
        println!("IP Address: {}", ip);
    }
    if let Some(date) = &created_device.install_date {
        println!("Install Date: {}", date);
    }

    Ok(())
}

pub fn device_rm_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
    company_id: Option<i32>,
    site_id: Option<i32>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let devices = if let Some(site) = site_id {
        get_devices_by_site(conn, site)?
    } else if let Some(comp) = company_id {
        get_devices_by_company(conn, comp)?
    } else {
        get_all_devices(conn)?
    };

    let matching_devices = if fixed_string {
        devices
            .into_iter()
            .filter(|device| {
                device.name.contains(&search_term)
                    || device.type_.contains(&search_term)
                    || device.model.contains(&search_term)
            })
            .collect::<Vec<_>>()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", search_term, e))?;
        devices
            .into_iter()
            .filter(|device| {
                regex.is_match(&device.name)
                    || regex.is_match(&device.type_)
                    || regex.is_match(&device.model)
            })
            .collect::<Vec<_>>()
    };

    if matching_devices.is_empty() {
        println!("No devices found matching the search term.");
        return Ok(());
    }

    println!("Found {} device(s) matching the search term:", matching_devices.len());
    for device in &matching_devices {
        println!(
            "  ID: {}, Name: {}, Type: {}, Model: {}, Company ID: {}, Site ID: {}",
            device.id, device.name, device.type_, device.model, device.company_id, device.site_id
        );
    }

    if !yes {
        print!(
            "Are you sure you want to delete these {} device(s)? [y/N]: ",
            matching_devices.len()
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

    for device in matching_devices {
        match delete_device(conn, device.id, Some(admin_user_id)) {
            Ok(rows_affected) => {
                if rows_affected > 0 {
                    deleted_count += 1;
                    println!("Deleted device: {} (ID: {})", device.name, device.id);
                }
            }
            Err(e) => {
                errors.push(format!(
                    "Failed to delete device {} (ID: {}): {}",
                    device.name, device.id, e
                ));
            }
        }
    }

    println!("Successfully deleted {} device(s).", deleted_count);

    if !errors.is_empty() {
        println!("Errors encountered:");
        for error in errors {
            println!("  {}", error);
        }
        return Err("Some deletions failed".into());
    }

    Ok(())
}

pub fn device_edit_impl(
    conn: &mut SqliteConnection,
    device_id: i32,
    new_name: Option<String>,
    new_description: Option<String>,
    new_type: Option<String>,
    new_model: Option<String>,
    new_serial: Option<String>,
    new_ip_address: Option<String>,
    new_install_date: Option<String>,
    new_company_id: Option<i32>,
    new_site_id: Option<i32>,
    admin_user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if device exists
    let device = get_device_by_id(conn, device_id)?;
    if device.is_none() {
        return Err(format!("Device with ID {} does not exist", device_id).into());
    }

    // Check if any fields need updating
    if new_name.is_none()
        && new_description.is_none()
        && new_type.is_none()
        && new_model.is_none()
        && new_serial.is_none()
        && new_ip_address.is_none()
        && new_install_date.is_none()
        && new_company_id.is_none()
        && new_site_id.is_none()
    {
        println!(
            "No fields specified for update. Use --name, --description, --type, --model, --serial, --ip, --install-date, --company, or --site."
        );
        return Ok(());
    }

    // Validate company exists if specified
    if let Some(comp_id) = new_company_id {
        if get_company_by_id(conn, comp_id)?.is_none() {
            return Err(format!("Company with ID {} does not exist", comp_id).into());
        }
    }

    // Validate site exists if specified
    if let Some(site_id) = new_site_id {
        if get_site_by_id(conn, site_id)?.is_none() {
            return Err(format!("Site with ID {} does not exist", site_id).into());
        }
    }

    // Parse optional fields that can be cleared
    let parsed_description = new_description.map(|s| if s.is_empty() { None } else { Some(s) });
    let parsed_serial = new_serial.map(|s| if s.is_empty() { None } else { Some(s) });
    let parsed_ip_address = new_ip_address.map(|s| if s.is_empty() { None } else { Some(s) });

    let parsed_install_date = if let Some(date_str) = new_install_date {
        if date_str.is_empty() {
            Some(None)
        } else {
            Some(Some(
                chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S")
                    .map_err(|e| format!("Invalid date format: {}. Use YYYY-MM-DD HH:MM:SS", e))?,
            ))
        }
    } else {
        None
    };

    let updated_device = update_device(
        conn,
        device_id,
        new_name,
        parsed_description,
        new_type,
        new_model,
        parsed_serial,
        parsed_ip_address,
        parsed_install_date,
        new_company_id,
        new_site_id,
        Some(admin_user_id),
    )?;

    println!("Device updated successfully!");
    println!("ID: {}", updated_device.id);
    println!("Name: {}", updated_device.name);
    println!("Type: {}", updated_device.type_);
    println!("Model: {}", updated_device.model);
    println!("Company ID: {}", updated_device.company_id);
    println!("Site ID: {}", updated_device.site_id);
    if let Some(desc) = &updated_device.description {
        println!("Description: {}", desc);
    }
    if let Some(serial) = &updated_device.serial {
        println!("Serial: {}", serial);
    }
    if let Some(ip) = &updated_device.ip_address {
        println!("IP Address: {}", ip);
    }
    if let Some(date) = &updated_device.install_date {
        println!("Install Date: {}", date);
    }

    Ok(())
}

#[cfg(all(test, feature = "test-staging"))]
#[allow(unused_imports)]
mod tests {
    use neems_api::{
        models::DeviceInput,
        orm::{
            company::insert_company,
            device::{get_all_devices, get_device_by_id, get_devices_by_site, insert_device},
            site::insert_site,
            testing::setup_test_db,
        },
    };

    use super::*;

    #[test]
    fn test_handle_device_command_with_conn_ls() {
        let mut conn = setup_test_db();

        let action = DeviceAction::Ls {
            search_term: None,
            fixed_string: false,
            company_id: None,
            site_id: None,
        };
        let result = handle_device_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_device_command_with_conn_add() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create test company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to create site");

        let action = DeviceAction::Add {
            name: Some("Test Device".to_string()),
            description: Some("Test Description".to_string()),
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: Some("SN123".to_string()),
            ip_address: Some("192.168.1.1".to_string()),
            install_date: Some("2024-01-15 10:30:00".to_string()),
            company_id: company.id.to_string(),
            site_id: site.id,
        };
        let result = handle_device_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());

        // Verify device was created
        let devices = get_all_devices(&mut conn).expect("Failed to get devices");
        let found = devices.iter().any(|d| d.name == "Test Device");
        assert!(found);
    }

    #[test]
    fn test_handle_device_command_with_conn_rm() {
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
            None,
        )
        .expect("Failed to create site");

        let device_input = DeviceInput {
            name: Some("Remove This Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input, Some(1)).expect("Failed to create device");

        let action = DeviceAction::Rm {
            search_term: "Remove This".to_string(),
            fixed_string: true,
            yes: true,
            company_id: None,
            site_id: None,
        };
        let result = handle_device_command_with_conn(&mut conn, action, 1);
        assert!(result.is_ok());

        let devices = get_all_devices(&mut conn).expect("Failed to get devices");
        let found = devices.iter().any(|d| d.name == "Remove This Device");
        assert!(!found);
    }

    #[test]
    fn test_device_ls_impl_all() {
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
            None,
        )
        .expect("Failed to create site");

        let device_input1 = DeviceInput {
            name: Some("Device 1".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input1, None).expect("Failed to create device 1");

        let device_input2 = DeviceInput {
            name: Some("Device 2".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input2, None).expect("Failed to create device 2");

        let result = device_ls_impl(&mut conn, None, false, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_device_ls_impl_with_search() {
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
            None,
        )
        .expect("Failed to create site");

        let device_input1 = DeviceInput {
            name: Some("Solar Inverter".to_string()),
            description: None,
            type_: "Inverter".to_string(),
            model: "SUN2000".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input1, None).expect("Failed to create device 1");

        let device_input2 = DeviceInput {
            name: Some("Battery System".to_string()),
            description: None,
            type_: "Battery".to_string(),
            model: "PowerWall".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input2, None).expect("Failed to create device 2");

        let result = device_ls_impl(&mut conn, Some("Solar".to_string()), true, None, None);
        assert!(result.is_ok());

        let result = device_ls_impl(&mut conn, Some("^Battery".to_string()), false, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_device_ls_impl_with_site_filter() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        let site1 = insert_site(
            &mut conn,
            "Site 1".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company.id,
            120,
            None,
        )
        .expect("Failed to create site 1");

        let site2 = insert_site(
            &mut conn,
            "Site 2".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company.id,
            120,
            None,
        )
        .expect("Failed to create site 2");

        let device_input1 = DeviceInput {
            name: Some("Device A".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site1.id,
        };
        insert_device(&mut conn, device_input1, None).expect("Failed to create device A");

        let device_input2 = DeviceInput {
            name: Some("Device B".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site2.id,
        };
        insert_device(&mut conn, device_input2, None).expect("Failed to create device B");

        let result = device_ls_impl(&mut conn, None, false, None, Some(site1.id));
        assert!(result.is_ok());
    }

    #[test]
    fn test_device_add_impl() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to create site");

        let device_input = DeviceInput {
            name: Some("New Device".to_string()),
            description: Some("Device Description".to_string()),
            type_: "Controller".to_string(),
            model: "Model X".to_string(),
            serial: Some("SN456".to_string()),
            ip_address: Some("192.168.1.100".to_string()),
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };

        let result = device_add_impl(&mut conn, device_input, 1);
        assert!(result.is_ok());

        let devices = get_all_devices(&mut conn).expect("Failed to get devices");
        let found = devices.iter().any(|d| d.name == "New Device");
        assert!(found);
    }

    #[test]
    fn test_device_add_impl_duplicate_name_same_site() {
        let mut conn = setup_test_db();

        let company = insert_company(&mut conn, "Test Company".to_string(), None)
            .expect("Failed to create company");

        let site = insert_site(
            &mut conn,
            "Test Site".to_string(),
            "123 Test St".to_string(),
            40.7128,
            -74.0060,
            company.id,
            120,
            None,
        )
        .expect("Failed to create site");

        // Create first device
        let device_input1 = DeviceInput {
            name: Some("Duplicate Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        let result = device_add_impl(&mut conn, device_input1, 1);
        assert!(result.is_ok());

        // Try to create second device with same name in same site - should succeed
        // gracefully
        let device_input2 = DeviceInput {
            name: Some("Duplicate Device".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        let result = device_add_impl(&mut conn, device_input2, 1);
        assert!(result.is_ok()); // Should handle duplicates gracefully

        // Verify there's still only one device with this name for this site
        let devices = get_devices_by_site(&mut conn, site.id).expect("Failed to get devices");
        let count = devices.iter().filter(|d| d.name == "Duplicate Device").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_device_rm_impl() {
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
            None,
        )
        .expect("Failed to create site");

        let device_input1 = DeviceInput {
            name: Some("Delete Me Device".to_string()),
            description: None,
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input1, Some(1)).expect("Failed to create device 1");

        let device_input2 = DeviceInput {
            name: Some("Keep Me Device".to_string()),
            description: None,
            type_: "Meter".to_string(),
            model: "Model B".to_string(),
            serial: None,
            ip_address: None,
            install_date: None,
            company_id: company.id,
            site_id: site.id,
        };
        insert_device(&mut conn, device_input2, Some(1)).expect("Failed to create device 2");

        let result = device_rm_impl(&mut conn, "Delete Me".to_string(), true, true, None, None, 1);
        assert!(result.is_ok());

        let devices = get_all_devices(&mut conn).expect("Failed to get devices");
        let found_deleted = devices.iter().any(|d| d.name == "Delete Me Device");
        assert!(!found_deleted);

        let found_kept = devices.iter().any(|d| d.name == "Keep Me Device");
        assert!(found_kept);
    }

    #[test]
    fn test_device_edit_impl() {
        let mut conn = setup_test_db();

        let company1 = insert_company(&mut conn, "Company 1".to_string(), None)
            .expect("Failed to create company 1");
        let company2 = insert_company(&mut conn, "Company 2".to_string(), None)
            .expect("Failed to create company 2");

        let site1 = insert_site(
            &mut conn,
            "Site 1".to_string(),
            "Address 1".to_string(),
            40.0,
            -74.0,
            company1.id,
            120,
            None,
        )
        .expect("Failed to create site 1");

        let site2 = insert_site(
            &mut conn,
            "Site 2".to_string(),
            "Address 2".to_string(),
            41.0,
            -75.0,
            company2.id,
            120,
            None,
        )
        .expect("Failed to create site 2");

        let device_input = DeviceInput {
            name: Some("Original Device".to_string()),
            description: Some("Original Description".to_string()),
            type_: "Sensor".to_string(),
            model: "Model A".to_string(),
            serial: Some("SN123".to_string()),
            ip_address: Some("192.168.1.1".to_string()),
            install_date: None,
            company_id: company1.id,
            site_id: site1.id,
        };
        let device =
            insert_device(&mut conn, device_input, Some(1)).expect("Failed to create device");

        // Edit name and description
        let result = device_edit_impl(
            &mut conn,
            device.id,
            Some("Updated Device".to_string()),
            Some("".to_string()), // Clear description
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            1,
        );
        assert!(result.is_ok());

        let updated_device = get_device_by_id(&mut conn, device.id)
            .expect("Failed to get updated device")
            .expect("Device should exist");
        assert_eq!(updated_device.name, "Updated Device");
        assert_eq!(updated_device.description, None);
        assert_eq!(updated_device.type_, "Sensor");
        assert_eq!(updated_device.model, "Model A");

        // Edit company and site
        let result = device_edit_impl(
            &mut conn,
            device.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(company2.id),
            Some(site2.id),
            1,
        );
        assert!(result.is_ok());

        let updated_device = get_device_by_id(&mut conn, device.id)
            .expect("Failed to get updated device")
            .expect("Device should exist");
        assert_eq!(updated_device.company_id, company2.id);
        assert_eq!(updated_device.site_id, site2.id);
    }

    #[test]
    fn test_device_edit_impl_nonexistent_device() {
        let mut conn = setup_test_db();

        let result = device_edit_impl(
            &mut conn,
            99999,
            Some("New Name".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            1,
        );
        assert!(result.is_err());
    }
}
