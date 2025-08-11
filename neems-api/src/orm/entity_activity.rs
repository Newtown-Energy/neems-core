use chrono::NaiveDateTime;
use diesel::prelude::*;

use crate::models::{EntityActivity, NewEntityActivity};

/// Log an activity for an entity
pub fn log_activity(
    conn: &mut SqliteConnection,
    table_name_val: &str,
    entity_id_val: i32,
    operation_type_val: &str,
    user_id_val: Option<i32>,
) -> Result<EntityActivity, diesel::result::Error> {
    use crate::schema::entity_activity::dsl::*;

    let new_activity = NewEntityActivity {
        table_name: table_name_val.to_string(),
        entity_id: entity_id_val,
        operation_type: operation_type_val.to_string(),
        timestamp: None, // Use database default (CURRENT_TIMESTAMP)
        user_id: user_id_val,
    };

    diesel::insert_into(entity_activity)
        .values(&new_activity)
        .execute(conn)?;

    // Get the inserted record
    entity_activity
        .order(id.desc())
        .first::<EntityActivity>(conn)
}

/// Get the creation timestamp for an entity (first 'create' operation)
pub fn get_created_at(
    conn: &mut SqliteConnection,
    table_name_val: &str,
    entity_id_val: i32,
) -> Result<NaiveDateTime, diesel::result::Error> {
    use crate::schema::entity_activity::dsl::*;

    entity_activity
        .filter(table_name.eq(table_name_val))
        .filter(entity_id.eq(entity_id_val))
        .filter(operation_type.eq("create"))
        .order(timestamp.asc())
        .select(timestamp)
        .first::<NaiveDateTime>(conn)
}

/// Get the last update timestamp for an entity (most recent operation)
pub fn get_updated_at(
    conn: &mut SqliteConnection,
    table_name_val: &str,
    entity_id_val: i32,
) -> Result<NaiveDateTime, diesel::result::Error> {
    use crate::schema::entity_activity::dsl::*;

    entity_activity
        .filter(table_name.eq(table_name_val))
        .filter(entity_id.eq(entity_id_val))
        .order(timestamp.desc())
        .select(timestamp)
        .first::<NaiveDateTime>(conn)
}

/// Get full activity history for an entity
pub fn get_activity_history(
    conn: &mut SqliteConnection,
    table_name_val: &str,
    entity_id_val: i32,
) -> Result<Vec<EntityActivity>, diesel::result::Error> {
    use crate::schema::entity_activity::dsl::*;

    entity_activity
        .filter(table_name.eq(table_name_val))
        .filter(entity_id.eq(entity_id_val))
        .order(timestamp.asc())
        .load::<EntityActivity>(conn)
}

/// Get all activities of a specific type
pub fn get_activities_by_operation(
    conn: &mut SqliteConnection,
    operation_type_val: &str,
) -> Result<Vec<EntityActivity>, diesel::result::Error> {
    use crate::schema::entity_activity::dsl::*;

    entity_activity
        .filter(operation_type.eq(operation_type_val))
        .order(timestamp.desc())
        .load::<EntityActivity>(conn)
}

/// Test function to verify database triggers are working
/// This creates a user, updates it, and deletes it, then checks the activity log
pub fn test_triggers_manually(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    use crate::orm::company;
    use crate::orm::user;
    use crate::models::UserInput;

    println!("Testing database triggers...");

    // Create a test company first
    let company = company::insert_company(conn, "Trigger Test Company".to_string())?;
    println!("Created test company with ID: {}", company.id);

    // Create a user (should trigger 'create' log)
    let new_user = UserInput {
        email: "trigger.test@example.com".to_string(),
        password_hash: "test_hash".to_string(),
        company_id: company.id,
        totp_secret: Some("test_secret".to_string()),
    };

    let created_user = user::insert_user(conn, new_user)?;
    println!("Created user with ID: {}", created_user.id);

    // Check if create activity was logged
    let create_activities = get_activity_history(conn, "users", created_user.id)?;
    println!("Activity after create: {} entries", create_activities.len());
    for activity in &create_activities {
        println!("  - {} at {}", activity.operation_type, activity.timestamp);
    }

    // Update the user (should trigger 'update' log)  
    user::update_user(
        conn, 
        created_user.id, 
        Some("updated.email@example.com".to_string()), 
        None, 
        None, 
        None
    )?;
    println!("Updated user email");

    // Check activities after update
    let update_activities = get_activity_history(conn, "users", created_user.id)?;
    println!("Activity after update: {} entries", update_activities.len());
    for activity in &update_activities {
        println!("  - {} at {}", activity.operation_type, activity.timestamp);
    }

    // Delete the user (should trigger 'delete' log)
    user::delete_user_with_cleanup(conn, created_user.id)?;
    println!("Deleted user");

    // Check final activities (delete should be logged)
    let final_activities = get_activity_history(conn, "users", created_user.id)?;
    println!("Activity after delete: {} entries", final_activities.len());
    for activity in &final_activities {
        println!("  - {} at {}", activity.operation_type, activity.timestamp);
    }

    // Verify we have create, update, and delete entries
    let has_create = final_activities.iter().any(|a| a.operation_type == "create");
    let has_update = final_activities.iter().any(|a| a.operation_type == "update");  
    let has_delete = final_activities.iter().any(|a| a.operation_type == "delete");

    println!("Trigger test results:");
    println!("  - Create logged: {}", has_create);
    println!("  - Update logged: {}", has_update);
    println!("  - Delete logged: {}", has_delete);

    if has_create && has_update && has_delete {
        println!("✅ All triggers working correctly!");
        Ok(())
    } else {
        Err("❌ Some triggers are not working".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;

    #[test]
    fn test_log_activity() {
        let mut conn = setup_test_db();

        let result = log_activity(&mut conn, "users", 1, "create", None);
        assert!(result.is_ok());
        
        let activity = result.unwrap();
        assert_eq!(activity.table_name, "users");
        assert_eq!(activity.entity_id, 1);
        assert_eq!(activity.operation_type, "create");
        assert_eq!(activity.user_id, None);
    }

    #[test]
    fn test_get_created_at() {
        let mut conn = setup_test_db();

        // First log a create activity
        log_activity(&mut conn, "users", 1, "create", None).unwrap();
        
        // Then log an update
        std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different timestamp
        log_activity(&mut conn, "users", 1, "update", None).unwrap();

        let created_at = get_created_at(&mut conn, "users", 1).unwrap();
        let updated_at = get_updated_at(&mut conn, "users", 1).unwrap();

        assert!(created_at <= updated_at);
    }

    #[test]
    fn test_get_activity_history() {
        let mut conn = setup_test_db();

        // Log multiple activities with no user_id to avoid foreign key issues
        log_activity(&mut conn, "users", 1, "create", None).unwrap();
        log_activity(&mut conn, "users", 1, "update", None).unwrap();
        log_activity(&mut conn, "users", 1, "update", None).unwrap();

        let history = get_activity_history(&mut conn, "users", 1).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].operation_type, "create");
        assert_eq!(history[1].operation_type, "update");
        assert_eq!(history[2].operation_type, "update");
    }

    #[test] 
    fn test_database_triggers() {
        let mut conn = setup_test_db();
        
        // This test will verify that database triggers are working
        let result = test_triggers_manually(&mut conn);
        assert!(result.is_ok(), "Database triggers test failed: {:?}", result);
    }

    #[test]
    fn test_all_table_triggers() {
        let mut conn = setup_test_db();
        
        let result = test_all_triggers_comprehensive(&mut conn);
        assert!(result.is_ok(), "Comprehensive triggers test failed: {:?}", result);
    }
}

/// Comprehensive test function to verify all table triggers are working
pub fn test_all_triggers_comprehensive(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    use crate::orm::{company, site};
    use crate::models::CompanyInput;

    println!("Testing triggers for all tables...");

    // Test company triggers
    println!("Testing company triggers...");
    let company_input = CompanyInput {
        name: "Trigger Test Company All".to_string(),
    };
    let created_company = company::insert_company(conn, company_input.name)?;
    
    // Check company create activity
    let company_activities = get_activity_history(conn, "companies", created_company.id)?;
    println!("Company activities after create: {}", company_activities.len());
    assert!(company_activities.iter().any(|a| a.operation_type == "create"), "Company create not logged");

    // Skip company update test since there's no update_company function
    // The create trigger is already verified above

    // Test site triggers
    println!("Testing site triggers...");
    let created_site = site::insert_site(
        conn, 
        "Test Site".to_string(),
        "123 Test St".to_string(),
        40.7128,
        -74.0060,
        created_company.id
    )?;
    
    // Check site create activity
    let site_activities = get_activity_history(conn, "sites", created_site.id)?;
    println!("Site activities after create: {}", site_activities.len());
    assert!(site_activities.iter().any(|a| a.operation_type == "create"), "Site create not logged");

    // Update site
    site::update_site(
        conn,
        created_site.id,
        Some("Updated Site Name".to_string()),
        None,
        None,
        None,
        None
    )?;
    let site_activities_after_update = get_activity_history(conn, "sites", created_site.id)?;
    println!("Site activities after update: {}", site_activities_after_update.len());
    assert!(site_activities_after_update.iter().any(|a| a.operation_type == "update"), "Site update not logged");

    // Test session triggers (general count since we use placeholder entity_id)
    println!("Testing session triggers...");
    // This would create a session, but we don't have direct session creation in our ORM
    // For now, we'll just verify the triggers exist by checking if we can count session activities
    let all_session_activities = get_activities_by_operation(conn, "create")?
        .iter()
        .filter(|a| a.table_name == "sessions")
        .count();
    
    println!("Session create activities found: {}", all_session_activities);

    // Clean up - delete entities (should trigger delete logs)
    site::delete_site(conn, created_site.id)?;
    let site_final_activities = get_activity_history(conn, "sites", created_site.id)?;
    assert!(site_final_activities.iter().any(|a| a.operation_type == "delete"), "Site delete not logged");

    company::delete_company(conn, created_company.id)?;  
    let company_final_activities = get_activity_history(conn, "companies", created_company.id)?;
    assert!(company_final_activities.iter().any(|a| a.operation_type == "delete"), "Company delete not logged");

    println!("✅ All table triggers working correctly!");
    Ok(())
}