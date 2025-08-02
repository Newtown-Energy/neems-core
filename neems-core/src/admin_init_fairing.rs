use diesel::prelude::*;
use dotenvy::dotenv;
use rocket::Rocket;
use rocket::fairing::AdHoc;

use crate::models::{CompanyNoTime, NewRole, NewUserRole, Role, User, UserNoTime};
use crate::orm::DbConn;
use crate::orm::company::{get_company_by_name, insert_company};
use crate::orm::login::hash_password;
use crate::orm::role::insert_role;
use crate::orm::user::insert_user;
use crate::schema::roles::dsl::*;
use crate::schema::user_roles;
use crate::schema::users::dsl::*;

/// Add default admin user and inst if needed.
///
/// Set the default admin email/pass based on envars NEEMS_DEFAULT_EMAIL and NEEMS_DEFAULT_PASSWORD
pub fn admin_init_fairing() -> AdHoc {
    AdHoc::try_on_ignite("Admin User Initialization", |rocket| async {
        dotenv().ok();

        let conn = match get_db_connection(&rocket).await {
            Some(conn) => conn,
            None => return Err(rocket),
        };

        let company = match setup_company(&conn).await {
            Ok(inst) => inst,
            Err(rocket) => return Err(rocket),
        };

        match setup_admin_user(&conn, company).await {
            Ok(()) => Ok(rocket),
            Err(rocket) => Err(rocket),
        }
    })
}

async fn get_db_connection(rocket: &Rocket<rocket::Build>) -> Option<DbConn> {
    match DbConn::get_one(rocket).await {
        Some(conn) => Some(conn),
        None => {
            error!("[admin-init] ERROR: Could not get DB connection.");
            None
        }
    }
}

async fn setup_company(
    conn: &DbConn,
) -> Result<crate::models::Company, rocket::Rocket<rocket::Build>> {
    conn.run(|c| find_or_create_company(c))
        .await
        .map_err(|_| rocket::build())
}

fn find_or_create_company(
    c: &mut SqliteConnection,
) -> Result<crate::models::Company, diesel::result::Error> {
    let candidate_names = [
        "Newtown Energy",
        "Newtown Energy, Inc",
        "Newtown Energy, Inc.",
    ];

    for cand in candidate_names {
        let comp_no_time = CompanyNoTime {
            name: cand.to_string(),
        };
        match get_company_by_name(c, &comp_no_time) {
            Ok(Some(found)) => {
                info!("[admin-init] Matched company: '{}'", cand);
                return Ok(found);
            }
            Ok(None) => continue,
            Err(e) => {
                error!("[admin-init] ERROR querying company '{}': {:?}", cand, e);
                return Err(e);
            }
        }
    }

    println!("[admin-init] No matching company found. Creating 'Newtown Energy'.");
    match insert_company(c, "Newtown Energy".to_string()) {
        Ok(inst) => Ok(inst),
        Err(e) => {
            error!("[admin-init] ERROR creating company: {:?}", e);
            Err(e)
        }
    }
}

async fn setup_admin_user(
    conn: &DbConn,
    company: crate::models::Company,
) -> Result<(), rocket::Rocket<rocket::Build>> {
    let admin_email = get_admin_email();

    conn.run(move |c| create_admin_user_if_needed(c, &admin_email, &company))
        .await
        .map_err(|e| {
            error!("[admin-init] FATAL: Admin user creation failed: {:?}", e);
            rocket::build()
        })
}

fn get_admin_email() -> String {
    std::env::var("NEEMS_DEFAULT_EMAIL").unwrap_or_else(|_| "superadmin@example.com".to_string())
}

fn create_admin_user_if_needed(
    c: &mut SqliteConnection,
    admin_email: &str,
    company: &crate::models::Company,
) -> Result<(), diesel::result::Error> {
    if admin_user_exists(c, admin_email)? {
        info!("[admin-init] Admin user '{}' already exists", admin_email);
        return Ok(());
    }

    let user = create_admin_user(c, admin_email, company)?;
    assign_admin_role(c, &user, admin_email)?;

    Ok(())
}

fn admin_user_exists(
    c: &mut SqliteConnection,
    admin_email: &str,
) -> Result<bool, diesel::result::Error> {
    let existing_user = users
        .filter(email.eq(admin_email))
        .first::<User>(c)
        .optional()?;

    Ok(existing_user.is_some())
}

fn create_admin_user(
    c: &mut SqliteConnection,
    admin_email: &str,
    company: &crate::models::Company,
) -> Result<crate::models::User, diesel::result::Error> {
    let admin_password = get_admin_password();
    let passhash = hash_password(&admin_password);

    let admin_user = UserNoTime {
        email: admin_email.to_string(),
        password_hash: passhash,
        company_id: company.id,
        totp_secret: None,
    };

    match insert_user(c, admin_user) {
        Ok(user) => {
            info!("[admin-init] Created admin user: '{}'", admin_email);
            Ok(user)
        }
        Err(e) => {
            error!("[admin-init] ERROR creating admin user: {:?}", e);
            Err(e)
        }
    }
}

fn get_admin_password() -> String {
    std::env::var("NEEMS_DEFAULT_PASSWORD").unwrap_or_else(|_| "admin".to_string())
}

fn assign_admin_role(
    c: &mut SqliteConnection,
    user: &crate::models::User,
    admin_email: &str,
) -> Result<(), diesel::result::Error> {
    let role_name = "newtown-admin";
    let role = find_or_create_admin_role(c, role_name)?;
    create_user_role_association(c, user, &role, role_name, admin_email)?;

    Ok(())
}

fn find_or_create_admin_role(
    c: &mut SqliteConnection,
    role_name: &str,
) -> Result<Role, diesel::result::Error> {
    let existing_role = roles
        .filter(name.eq(role_name))
        .first::<Role>(c)
        .optional()?;

    match existing_role {
        Some(r) => Ok(r),
        None => {
            info!("[admin-init] Creating role: '{}'", role_name);
            let new_role = NewRole {
                name: role_name.to_string(),
                description: Some("Administrator for Newtown".to_string()),
            };
            match insert_role(c, new_role) {
                Ok(r) => Ok(r),
                Err(e) => {
                    error!("[admin-init] ERROR creating role: {:?}", e);
                    Err(e)
                }
            }
        }
    }
}

fn create_user_role_association(
    c: &mut SqliteConnection,
    user: &crate::models::User,
    role: &Role,
    role_name: &str,
    admin_email: &str,
) -> Result<(), diesel::result::Error> {
    let new_user_role = NewUserRole {
        user_id: user.id,
        role_id: role.id,
    };

    match diesel::insert_into(user_roles::table)
        .values(&new_user_role)
        .execute(c)
    {
        Ok(_) => {
            println!(
                "[admin-init] Assigned role '{}' to user '{}'",
                role_name, admin_email
            );
            Ok(())
        }
        Err(e) => {
            error!("[admin-init] ERROR assigning role: {:?}", e);
            Err(e)
        }
    }
}
