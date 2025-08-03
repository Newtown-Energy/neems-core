use clap::Subcommand;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use neems_api::orm::company::{
    delete_company, get_all_companies, get_company_by_id, insert_company,
};
use neems_api::orm::site::{delete_site, get_sites_by_company};
use neems_api::orm::user::{delete_user_with_cleanup, get_users_by_company};
use regex::Regex;
use std::io::{self, Write};

#[derive(Subcommand)]
pub enum CompanyAction {
    #[command(about = "List companies, optionally filtered by search term")]
    Ls {
        #[arg(help = "Search term (regex by default, use -F for fixed string)")]
        search_term: Option<String>,
        #[arg(
            short = 'F',
            long = "fixed-string",
            help = "Treat search term as fixed string instead of regex"
        )]
        fixed_string: bool,
    },
    #[command(about = "Add a new company")]
    Add {
        #[arg(short, long, help = "Company name")]
        name: String,
    },
    #[command(about = "Remove companies matching search term")]
    Rm {
        #[arg(
            help = "Search term to match companies for removal (regex by default, use -F for fixed string)"
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
    },
    #[command(about = "Edit company fields")]
    Edit {
        #[arg(short, long, help = "Company ID to edit")]
        id: i32,
        #[arg(long, help = "New company name")]
        name: Option<String>,
    },
}

pub fn handle_company_command_with_conn(
    conn: &mut SqliteConnection,
    action: CompanyAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        CompanyAction::Ls {
            search_term,
            fixed_string,
        } => {
            company_ls_impl(conn, search_term, fixed_string)?;
        }
        CompanyAction::Add { name } => {
            company_add_impl(conn, name)?;
        }
        CompanyAction::Rm {
            search_term,
            fixed_string,
            yes,
        } => {
            company_rm_impl(conn, search_term, fixed_string, yes)?;
        }
        CompanyAction::Edit { id, name } => {
            company_edit_impl(conn, id, name)?;
        }
    }
    Ok(())
}

pub fn company_ls_impl(
    conn: &mut SqliteConnection,
    search_term: Option<String>,
    fixed_string: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let companies = get_all_companies(conn)?;

    let filtered_companies = if let Some(term) = search_term {
        if fixed_string {
            companies
                .into_iter()
                .filter(|company| company.name.contains(&term))
                .collect::<Vec<_>>()
        } else {
            let regex = Regex::new(&term)
                .map_err(|e| format!("Invalid regex pattern '{}': {}", term, e))?;
            companies
                .into_iter()
                .filter(|company| regex.is_match(&company.name))
                .collect::<Vec<_>>()
        }
    } else {
        companies
    };

    if filtered_companies.is_empty() {
        println!("No companies found.");
    } else {
        println!("Companies:");
        for company in filtered_companies {
            println!(
                "  ID: {}, Name: {}, Created: {}",
                company.id, company.name, company.created_at
            );
        }
    }

    Ok(())
}

pub fn company_add_impl(
    conn: &mut SqliteConnection,
    name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let created_company = insert_company(conn, name)?;

    println!("Company created successfully!");
    println!("ID: {}", created_company.id);
    println!("Name: {}", created_company.name);
    println!("Created: {}", created_company.created_at);

    Ok(())
}

pub fn company_rm_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let companies = get_all_companies(conn)?;

    let matching_companies = if fixed_string {
        companies
            .into_iter()
            .filter(|company| company.name.contains(&search_term))
            .collect::<Vec<_>>()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", search_term, e))?;
        companies
            .into_iter()
            .filter(|company| regex.is_match(&company.name))
            .collect::<Vec<_>>()
    };

    if matching_companies.is_empty() {
        println!("No companies found matching the search term.");
        return Ok(());
    }

    println!(
        "Found {} company(ies) matching the search term:",
        matching_companies.len()
    );
    for company in &matching_companies {
        // Get associated users and sites counts
        let users = get_users_by_company(conn, company.id)?;
        let sites = get_sites_by_company(conn, company.id)?;

        println!(
            "  ID: {}, Name: {}, Users: {}, Sites: {}",
            company.id,
            company.name,
            users.len(),
            sites.len()
        );
    }

    if !yes {
        print!(
            "Are you sure you want to delete these {} company(ies) and all associated users and sites? [y/N]: ",
            matching_companies.len()
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

    for company in matching_companies {
        match delete_company_with_cascade(conn, company.id) {
            Ok(success) => {
                if success {
                    deleted_count += 1;
                    println!("Deleted company: {} (ID: {})", company.name, company.id);
                }
            }
            Err(e) => {
                errors.push(format!(
                    "Failed to delete company {} (ID: {}): {}",
                    company.name, company.id, e
                ));
            }
        }
    }

    println!("Successfully deleted {} company(ies).", deleted_count);

    if !errors.is_empty() {
        println!("Errors encountered:");
        for error in errors {
            println!("  {}", error);
        }
        return Err("Some deletions failed".into());
    }

    Ok(())
}

fn delete_company_with_cascade(
    conn: &mut SqliteConnection,
    company_id: i32,
) -> Result<bool, Box<dyn std::error::Error>> {
    // First delete all users in the company
    let users = get_users_by_company(conn, company_id)?;
    for user in users {
        delete_user_with_cleanup(conn, user.id)?;
    }

    // Then delete all sites in the company
    let sites = get_sites_by_company(conn, company_id)?;
    for site in sites {
        delete_site(conn, site.id)?;
    }

    // Finally delete the company itself
    let deleted = delete_company(conn, company_id)?;
    Ok(deleted)
}

pub fn company_edit_impl(
    conn: &mut SqliteConnection,
    company_id: i32,
    new_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if company exists
    let company = get_company_by_id(conn, company_id)?;
    if company.is_none() {
        return Err(format!("Company with ID {} does not exist", company_id).into());
    }
    let company = company.unwrap();

    // Check if any fields need updating
    if new_name.is_none() {
        println!("No fields specified for update. Use --name.");
        return Ok(());
    }

    update_company(conn, company_id, new_name.clone())?;

    println!("Company updated successfully!");
    println!("ID: {}", company.id);
    println!("Name: {}", new_name.unwrap_or(company.name));

    Ok(())
}

fn update_company(
    conn: &mut SqliteConnection,
    company_id: i32,
    new_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use neems_api::schema::companies::dsl::*;

    if let Some(name_val) = new_name {
        let now = chrono::Utc::now().naive_utc();

        diesel::update(companies.filter(id.eq(company_id)))
            .set((name.eq(name_val), updated_at.eq(now)))
            .execute(conn)?;
    }

    Ok(())
}
