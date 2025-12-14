//! API endpoints for managing application rules and schedule resolution.

use std::collections::HashMap;

use rocket::{Route, http::Status, response::status, serde::json::Json};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    logged_json::LoggedJson,
    models::{
        ApplicationRule, CalendarDaySchedule, CalendarDayScheduleMatches,
        CreateApplicationRuleRequest, EffectiveScheduleResponse,
    },
    orm::{
        DbConn,
        application_rule::{
            create_application_rule, delete_application_rule, get_application_rules_for_site,
            get_application_rules_for_template, get_calendar_schedules,
            get_calendar_schedules_with_matches, get_effective_schedule,
        },
        schedule_library::get_library_item,
        site::get_site_by_id,
    },
    session_guards::AuthenticatedUser,
};

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

// Helper function to check if user can manage schedules for a site
fn can_manage_schedule(
    user: &AuthenticatedUser,
    site_id: i32,
    conn: &mut diesel::SqliteConnection,
) -> bool {
    // newtown-admin and newtown-staff can manage any schedule
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Company admins can manage schedules for their company's sites
    if user.has_role("admin") {
        if let Ok(Some(site_data)) = get_site_by_id(conn, site_id) {
            return site_data.company_id == user.user.company_id;
        }
    }

    false
}

// Helper function to check if user can view schedules for a site
fn can_view_schedule(
    user: &AuthenticatedUser,
    site_id: i32,
    conn: &mut diesel::SqliteConnection,
) -> bool {
    // newtown-admin and newtown-staff can view any schedule
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Any user can view schedules for their company's sites
    if let Ok(Some(site_data)) = get_site_by_id(conn, site_id) {
        return site_data.company_id == user.user.company_id;
    }

    false
}

/// Get all application rules for a library item
#[get("/1/ScheduleLibraryItems/<id>/ApplicationRules")]
pub async fn get_rules_for_library_item(
    db: DbConn,
    id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<ApplicationRule>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Get the library item to check authorization
        let item = match get_library_item(conn, id) {
            Ok(item) => item,
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Library item not found".to_string(),
                });
                return Err(status::Custom(Status::NotFound, err));
            }
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        // Check authorization
        if !can_view_schedule(&auth_user, item.site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        get_application_rules_for_template(conn, id).map(Json).map_err(|e| {
            eprintln!("Error getting application rules: {:?}", e);
            let err = Json(ErrorResponse {
                error: "Internal server error".to_string(),
            });
            status::Custom(Status::InternalServerError, err)
        })
    })
    .await
}

/// Get all application rules for a site
#[get("/1/Sites/<site_id>/ApplicationRules")]
pub async fn get_rules_for_site(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<ApplicationRule>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Check authorization
        if !can_view_schedule(&auth_user, site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        get_application_rules_for_site(conn, site_id).map(Json).map_err(|e| {
            eprintln!("Error getting application rules: {:?}", e);
            let err = Json(ErrorResponse {
                error: "Internal server error".to_string(),
            });
            status::Custom(Status::InternalServerError, err)
        })
    })
    .await
}

/// Create a new application rule
#[post("/1/ScheduleLibraryItems/<id>/ApplicationRules", data = "<request>")]
pub async fn create_application_rule_endpoint(
    db: DbConn,
    id: i32,
    request: LoggedJson<CreateApplicationRuleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<ApplicationRule>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Get the library item to check authorization
        let item = match get_library_item(conn, id) {
            Ok(item) => item,
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Library item not found".to_string(),
                });
                return Err(status::Custom(Status::NotFound, err));
            }
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        // Check authorization
        if !can_manage_schedule(&auth_user, item.site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match create_application_rule(conn, id, request.into_inner(), Some(auth_user.user.id)) {
            Ok(rule) => {
                let location = format!("/api/1/ApplicationRules/{}", rule.id);
                Ok(status::Created::new(location).body(Json(rule)))
            }
            Err(e) => {
                eprintln!("Error creating application rule: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Delete an application rule
#[delete("/1/ApplicationRules/<id>")]
pub async fn delete_application_rule_endpoint(
    db: DbConn,
    id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Get the rule to check authorization
        let rules = match get_application_rules_for_template(conn, id) {
            Ok(rules) => rules,
            Err(e) => {
                eprintln!("Error getting application rule: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Application rule not found".to_string(),
                });
                return Err(status::Custom(Status::NotFound, err));
            }
        };

        // Find the rule
        let rule = rules.into_iter().find(|r| r.id == id);
        if rule.is_none() {
            let err = Json(ErrorResponse {
                error: "Application rule not found".to_string(),
            });
            return Err(status::Custom(Status::NotFound, err));
        }

        let rule = rule.unwrap();

        // Get the library item to check site_id
        let item = match get_library_item(conn, rule.library_item_id) {
            Ok(item) => item,
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        // Check authorization
        if !can_manage_schedule(&auth_user, item.site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match delete_application_rule(conn, id, Some(auth_user.user.id)) {
            Ok(_) => Ok(Status::NoContent),
            Err(e) => {
                eprintln!("Error deleting application rule: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get the effective schedule for a specific date
#[get("/1/Sites/<site_id>/EffectiveSchedule?<date>")]
pub async fn get_effective_schedule_endpoint(
    db: DbConn,
    site_id: i32,
    date: String,
    auth_user: AuthenticatedUser,
) -> Result<Json<EffectiveScheduleResponse>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Check authorization
        if !can_view_schedule(&auth_user, site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        // Parse date
        let parsed_date = match chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                let err = Json(ErrorResponse {
                    error: "Invalid date format. Use YYYY-MM-DD".to_string(),
                });
                return Err(status::Custom(Status::BadRequest, err));
            }
        };

        match get_effective_schedule(conn, site_id, parsed_date) {
            Ok(schedule) => Ok(Json(schedule)),
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "No schedule found for this date".to_string(),
                });
                Err(status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error getting effective schedule: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get calendar schedules for a month
#[get("/1/Sites/<site_id>/CalendarSchedules?<year>&<month>")]
pub async fn get_calendar_schedules_endpoint(
    db: DbConn,
    site_id: i32,
    year: i32,
    month: u32,
    auth_user: AuthenticatedUser,
) -> Result<Json<HashMap<String, CalendarDaySchedule>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Check authorization
        if !can_view_schedule(&auth_user, site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match get_calendar_schedules(conn, site_id, year, month) {
            Ok(schedules) => Ok(Json(schedules)),
            Err(e) => {
                eprintln!("Error getting calendar schedules: {:?}", e);
                let err = Json(ErrorResponse {
                    error: format!("Error getting calendar schedules: {}", e),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get calendar schedules with ALL matches for a month
#[get("/1/Sites/<site_id>/CalendarSchedulesWithMatches?<year>&<month>")]
pub async fn get_calendar_schedules_with_matches_endpoint(
    db: DbConn,
    site_id: i32,
    year: i32,
    month: u32,
    auth_user: AuthenticatedUser,
) -> Result<Json<HashMap<String, CalendarDayScheduleMatches>>, status::Custom<Json<ErrorResponse>>>
{
    db.run(move |conn| {
        // Check authorization
        if !can_view_schedule(&auth_user, site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match get_calendar_schedules_with_matches(conn, site_id, year, month) {
            Ok(schedules) => Ok(Json(schedules)),
            Err(e) => {
                eprintln!("Error getting calendar schedules with matches: {:?}", e);
                let err = Json(ErrorResponse {
                    error: format!("Error getting calendar schedules with matches: {}", e),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

pub fn routes() -> Vec<Route> {
    routes![
        get_rules_for_library_item,
        get_rules_for_site,
        create_application_rule_endpoint,
        delete_application_rule_endpoint,
        get_effective_schedule_endpoint,
        get_calendar_schedules_endpoint,
        get_calendar_schedules_with_matches_endpoint,
    ]
}
