use clap::Subcommand;
use diesel::sqlite::SqliteConnection;
use neems_api::models::{NewRole, Role};
use neems_api::orm::role::{delete_role, get_all_roles, get_role, insert_role, update_role};
use regex::Regex;
use std::io::{self, Write};

#[derive(Subcommand)]
pub enum RoleAction {
    #[command(about = "List roles, optionally filtered by search term")]
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
    #[command(about = "Add a new role")]
    Add {
        #[arg(short, long, help = "Role name")]
        name: String,
        #[arg(short, long, help = "Role description")]
        description: Option<String>,
    },
    #[command(about = "Remove roles matching search term")]
    Rm {
        #[arg(
            help = "Search term to match roles for removal (regex by default, use -F for fixed string)"
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
    #[command(about = "Edit role fields")]
    Edit {
        #[arg(help = "Role ID to edit")]
        role_id: i32,
        #[arg(short, long, help = "New role name")]
        name: Option<String>,
        #[arg(short, long, help = "New role description")]
        description: Option<String>,
    },
}

pub fn handle_role_command_with_conn(
    conn: &mut SqliteConnection,
    action: RoleAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        RoleAction::Ls {
            search_term,
            fixed_string,
        } => {
            role_ls_impl(conn, search_term, fixed_string)?;
        }
        RoleAction::Add { name, description } => {
            role_add_impl(conn, name, description)?;
        }
        RoleAction::Rm {
            search_term,
            fixed_string,
            yes,
        } => {
            role_rm_impl(conn, search_term, fixed_string, yes)?;
        }
        RoleAction::Edit {
            role_id,
            name,
            description,
        } => {
            role_edit_impl(conn, role_id, name, description)?;
        }
    }
    Ok(())
}

pub fn role_ls_impl(
    conn: &mut SqliteConnection,
    search_term: Option<String>,
    fixed_string: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let roles = get_all_roles(conn)?;

    let filtered_roles: Vec<Role> = if let Some(term) = search_term {
        if fixed_string {
            roles
                .into_iter()
                .filter(|role| role.name.contains(&term))
                .collect()
        } else {
            let regex =
                Regex::new(&term).map_err(|e| format!("Invalid regex '{}': {}", term, e))?;
            roles
                .into_iter()
                .filter(|role| regex.is_match(&role.name))
                .collect()
        }
    } else {
        roles
    };

    if filtered_roles.is_empty() {
        println!("No roles found.");
    } else {
        println!("Roles:");
        for role in filtered_roles {
            let desc = role.description.as_deref().unwrap_or("(no description)");
            println!(
                "  ID: {}, Name: {}, Description: {}",
                role.id, role.name, desc
            );
        }
    }

    Ok(())
}

pub fn role_add_impl(
    conn: &mut SqliteConnection,
    name: String,
    description: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_role = NewRole {
        name: name.clone(),
        description,
    };
    let created_role = insert_role(conn, new_role)?;

    println!("Successfully added role:");
    println!("  ID: {}", created_role.id);
    println!("  Name: {}", created_role.name);
    if let Some(desc) = &created_role.description {
        println!("  Description: {}", desc);
    }

    Ok(())
}

pub fn role_rm_impl(
    conn: &mut SqliteConnection,
    search_term: String,
    fixed_string: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let roles = get_all_roles(conn)?;

    let matching_roles: Vec<Role> = if fixed_string {
        roles
            .into_iter()
            .filter(|role| role.name.contains(&search_term))
            .collect()
    } else {
        let regex = Regex::new(&search_term)
            .map_err(|e| format!("Invalid regex '{}': {}", search_term, e))?;
        roles
            .into_iter()
            .filter(|role| regex.is_match(&role.name))
            .collect()
    };

    if matching_roles.is_empty() {
        println!("No roles found matching '{}'", search_term);
        return Ok(());
    }

    println!("Roles to be removed:");
    for role in &matching_roles {
        let desc = role.description.as_deref().unwrap_or("(no description)");
        println!(
            "  ID: {}, Name: {}, Description: {}",
            role.id, role.name, desc
        );
    }

    if !yes {
        print!(
            "Are you sure you want to remove {} role(s)? [y/N]: ",
            matching_roles.len()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().to_lowercase().starts_with('y') {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let mut removed_count = 0;
    for role in matching_roles {
        match delete_role(conn, role.id) {
            Ok(rows_affected) => {
                if rows_affected > 0 {
                    println!("Removed role: {}", role.name);
                    removed_count += 1;
                } else {
                    println!(
                        "Warning: Role {} was not found (may have been removed already)",
                        role.name
                    );
                }
            }
            Err(e) => {
                eprintln!("Error removing role {}: {}", role.name, e);
            }
        }
    }

    println!("Successfully removed {} role(s).", removed_count);
    Ok(())
}

pub fn role_edit_impl(
    conn: &mut SqliteConnection,
    role_id: i32,
    new_name: Option<String>,
    new_description: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if role exists
    let _role =
        get_role(conn, role_id).map_err(|_| format!("Role with ID {} not found", role_id))?;

    if new_name.is_none() && new_description.is_none() {
        println!("No changes specified. Use --name or --description to specify changes.");
        return Ok(());
    }

    // Convert description option for the update function
    let description_update = new_description.map(Some);

    let updated_role = update_role(conn, role_id, new_name, description_update)?;

    println!("Successfully updated role:");
    println!("  ID: {}", updated_role.id);
    println!("  Name: {}", updated_role.name);
    if let Some(desc) = &updated_role.description {
        println!("  Description: {}", desc);
    } else {
        println!("  Description: (no description)");
    }

    Ok(())
}
