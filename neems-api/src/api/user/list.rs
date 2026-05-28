//! List users endpoint with OData filtering, sort, and expand.

use rocket::{http::Status, serde::json::Json};

use crate::{
    odata_query::{ODataCollectionResponse, ODataQuery, apply_select, build_context_url},
    orm::{
        DbConn,
        user::{get_users_by_company_with_roles, list_all_users_with_roles},
    },
    session_guards::AuthenticatedUser,
};

/// List Users endpoint.
///
/// - **URL:** `/api/1/users`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all users in the system
/// - **Authentication:** Required
///
/// This endpoint retrieves all users from the database and returns them
/// as a JSON array. This includes all user information including timestamps
/// and associated company IDs.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "email": "user1@example.com",
///     "password_hash": "hashed_password",
///     "company_id": 1,
///     "totp_secret": null,
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   },
///   {
///     "id": 2,
///     "email": "user2@example.com",
///     "password_hash": "hashed_password",
///     "company_id": 2,
///     "totp_secret": "secret",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// # Arguments
/// * `db` - Database connection pool
///
/// # Returns
/// * `Ok(Json<Vec<User>>)` - List of all users
/// * `Err(Status)` - Error during retrieval (typically InternalServerError)
#[get("/1/Users?<query..>")]
pub async fn list_users(
    db: DbConn,
    auth_user: AuthenticatedUser,
    query: ODataQuery,
) -> Result<Json<serde_json::Value>, Status> {
    // Validate query options
    query.validate().map_err(|_| Status::BadRequest)?;

    // Authorization: determine which users this user can see
    let users = if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        // newtown-admin and newtown-staff can see all users
        db.run(|conn| {
            list_all_users_with_roles(conn).map_err(|e| {
                eprintln!("Error listing all users: {:?}", e);
                Status::InternalServerError
            })
        })
        .await?
    } else if auth_user.has_role("admin") {
        // admin can only see users from their own company
        let company_id = auth_user.user.company_id;
        db.run(move |conn| {
            get_users_by_company_with_roles(conn, company_id).map_err(|e| {
                eprintln!("Error listing company users: {:?}", e);
                Status::InternalServerError
            })
        })
        .await?
    } else {
        // Regular users cannot list users
        return Err(Status::Forbidden);
    };

    // Apply filtering if specified
    let mut filtered_users = users;
    if let Some(filter_expr) = query.parse_filter() {
        // Basic filtering implementation - this could be expanded
        filtered_users.retain(|user| {
            // Simple implementation - could be much more sophisticated
            match &filter_expr.property.as_str() {
                &"email" => match &filter_expr.value {
                    crate::odata_query::FilterValue::String(s) => match filter_expr.operator {
                        crate::odata_query::FilterOperator::Eq => user.email == *s,
                        crate::odata_query::FilterOperator::Ne => user.email != *s,
                        crate::odata_query::FilterOperator::Contains => user.email.contains(s),
                        _ => true,
                    },
                    _ => true,
                },
                _ => true, // Unknown property, don't filter
            }
        });
    }

    // Apply sorting if specified
    if let Some(orderby) = query.parse_orderby() {
        for (property, direction) in orderby.iter().rev() {
            match property.as_str() {
                "email" => {
                    filtered_users.sort_by(|a, b| {
                        let cmp = a.email.cmp(&b.email);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                "id" => {
                    filtered_users.sort_by(|a, b| {
                        let cmp = a.id.cmp(&b.id);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                _ => {} // Unknown property, don't sort
            }
        }
    }

    // Get count before applying top/skip
    let total_count = filtered_users.len() as i64;

    // Apply skip and top
    if let Some(skip) = query.skip {
        filtered_users = filtered_users.into_iter().skip(skip as usize).collect();
    }
    if let Some(top) = query.top {
        filtered_users = filtered_users.into_iter().take(top as usize).collect();
    }

    // Handle $expand and computed properties, then $select
    let expand_props = query.parse_expand();
    let select_props = query.parse_select();
    let mut expanded_users: Vec<serde_json::Value> = Vec::new();

    // Check if activity timestamps are requested in $select
    let needs_activity_timestamps = if let Some(ref select_fields) = select_props {
        select_fields
            .iter()
            .any(|field| field == "activity_created_at" || field == "activity_updated_at")
    } else {
        false // Default behavior doesn't include activity timestamps
    };

    for user in &filtered_users {
        let mut user_json = serde_json::to_value(user).map_err(|_| Status::InternalServerError)?;

        // Handle $expand=company
        if let Some(expansions) = &expand_props
            && expansions.iter().any(|e| e.eq_ignore_ascii_case("company"))
        {
            // Load company data for this user
            let company_id = user.company_id;
            let company = db
                .run(move |conn| {
                    use crate::orm::company::get_company_by_id;
                    get_company_by_id(conn, company_id)
                })
                .await
                .map_err(|_| Status::InternalServerError)?;

            if let Some(company) = company {
                user_json.as_object_mut().ok_or(Status::InternalServerError)?.insert(
                    "Company".to_string(),
                    serde_json::to_value(company).map_err(|_| Status::InternalServerError)?,
                );
            }
        }

        // Handle computed activity timestamps if requested
        if needs_activity_timestamps {
            let user_id = user.id;
            let timestamps = db
                .run(move |conn| {
                    use crate::orm::entity_activity::{get_created_at, get_updated_at};

                    let created_at = get_created_at(conn, "users", user_id).ok();
                    let updated_at = get_updated_at(conn, "users", user_id).ok();

                    (created_at, updated_at)
                })
                .await;

            // Add activity timestamps to user object
            let user_obj = user_json.as_object_mut().ok_or(Status::InternalServerError)?;
            if let Some(created_at) = timestamps.0 {
                user_obj.insert(
                    "activity_created_at".to_string(),
                    serde_json::Value::String(
                        created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    ),
                );
            } else {
                user_obj.insert("activity_created_at".to_string(), serde_json::Value::Null);
            }

            if let Some(updated_at) = timestamps.1 {
                user_obj.insert(
                    "activity_updated_at".to_string(),
                    serde_json::Value::String(
                        updated_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    ),
                );
            } else {
                user_obj.insert("activity_updated_at".to_string(), serde_json::Value::Null);
            }
        }

        expanded_users.push(user_json);
    }

    // Apply $select to each expanded user if specified
    let selected_users: Result<Vec<serde_json::Value>, _> = expanded_users
        .iter()
        .map(|user| apply_select(user, select_props.as_deref()))
        .collect();

    let selected_users = selected_users.map_err(|_| Status::InternalServerError)?;

    // Build OData response
    let context = build_context_url("http://localhost/api/1", "Users", select_props.as_deref());
    let mut response = ODataCollectionResponse::new(context, selected_users);

    // Add count if requested
    if query.count.unwrap_or(false) {
        response = response.with_count(total_count);
    }

    Ok(Json(serde_json::to_value(response).map_err(|_| Status::InternalServerError)?))
}
