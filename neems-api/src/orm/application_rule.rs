use std::collections::HashMap;

use diesel::{prelude::*, sql_types::BigInt};

use crate::models::{
    ApplicationRule, ApplicationRuleDb, CalendarDaySchedule, CalendarDayScheduleMatches,
    CreateApplicationRuleRequest, EffectiveScheduleResponse, NewApplicationRule, RuleType,
};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

/// Creates a new application rule
/// If creating a default rule, deletes existing default for the site
pub fn create_application_rule(
    conn: &mut SqliteConnection,
    template_id: i32,
    request: CreateApplicationRuleRequest,
    acting_user_id: Option<i32>,
) -> Result<ApplicationRule, diesel::result::Error> {
    use crate::schema::{application_rules, schedule_templates};

    conn.transaction(|conn| {
        // If creating default, remove existing defaults for this site
        if request.rule_type == RuleType::Default {
            let template = schedule_templates::table
                .find(template_id)
                .first::<crate::models::ScheduleTemplate>(conn)?;

            // Get all templates for this site
            let site_template_ids: Vec<i32> = schedule_templates::table
                .filter(schedule_templates::site_id.eq(template.site_id))
                .select(schedule_templates::id)
                .load(conn)?;

            // Delete existing default rules for this site
            diesel::delete(
                application_rules::table
                    .filter(application_rules::template_id.eq_any(&site_template_ids))
                    .filter(application_rules::rule_type.eq("default")),
            )
            .execute(conn)?;
        }

        // Prepare JSON fields
        let days_of_week_json =
            request.days_of_week.as_ref().map(|d| serde_json::to_string(d).unwrap());
        let specific_dates_json =
            request.specific_dates.as_ref().map(|d| serde_json::to_string(d).unwrap());

        // Insert new rule
        let new_rule = NewApplicationRule {
            template_id,
            rule_type: request.rule_type.as_str().to_string(),
            days_of_week: days_of_week_json,
            specific_dates: specific_dates_json,
            override_reason: request.override_reason.clone(),
        };

        diesel::insert_into(application_rules::table).values(&new_rule).execute(conn)?;

        let rule_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
            .get_result::<LastInsertRowId>(conn)?
            .last_insert_rowid as i32;

        // Update activity log
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ =
                update_latest_activity_user(conn, "application_rules", rule_id, "create", user_id);
        }

        // Return created rule
        let rule_db = application_rules::table.find(rule_id).first::<ApplicationRuleDb>(conn)?;

        rule_db.to_api_model().map_err(|e| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })
    })
}

/// Gets all application rules for a template
pub fn get_application_rules_for_template(
    conn: &mut SqliteConnection,
    template_id: i32,
) -> Result<Vec<ApplicationRule>, diesel::result::Error> {
    use crate::schema::application_rules;

    let rules_db: Vec<ApplicationRuleDb> = application_rules::table
        .filter(application_rules::template_id.eq(template_id))
        .order_by(application_rules::created_at.desc())
        .load(conn)?;

    rules_db
        .into_iter()
        .map(|r| {
            r.to_api_model().map_err(|e| {
                diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })
        })
        .collect()
}

/// Checks if a library item has a default rule
pub fn has_default_rule(
    conn: &mut SqliteConnection,
    template_id: i32,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::application_rules;

    let count: i64 = application_rules::table
        .filter(application_rules::template_id.eq(template_id))
        .filter(application_rules::rule_type.eq("default"))
        .count()
        .get_result(conn)?;

    Ok(count > 0)
}

/// Gets all application rules for a site
pub fn get_application_rules_for_site(
    conn: &mut SqliteConnection,
    site_id: i32,
) -> Result<Vec<ApplicationRule>, diesel::result::Error> {
    use crate::schema::{application_rules, schedule_templates};

    // Get all template IDs for this site
    let template_ids: Vec<i32> = schedule_templates::table
        .filter(schedule_templates::site_id.eq(site_id))
        .select(schedule_templates::id)
        .load(conn)?;

    // Get all rules for these templates
    let rules_db: Vec<ApplicationRuleDb> = application_rules::table
        .filter(application_rules::template_id.eq_any(&template_ids))
        .order_by(application_rules::created_at.desc())
        .load(conn)?;

    rules_db
        .into_iter()
        .map(|r| {
            r.to_api_model().map_err(|e| {
                diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })
        })
        .collect()
}

/// Deletes an application rule
pub fn delete_application_rule(
    conn: &mut SqliteConnection,
    rule_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::application_rules;

    let result = diesel::delete(application_rules::table.filter(application_rules::id.eq(rule_id)))
        .execute(conn)?;

    if result > 0 {
        if let Some(user_id) = acting_user_id {
            use crate::orm::entity_activity::update_latest_activity_user;
            let _ =
                update_latest_activity_user(conn, "application_rules", rule_id, "delete", user_id);
        }
    }

    Ok(result)
}

/// Gets the effective schedule for a specific date
/// Applies precedence rules: specific_date > day_of_week > default
pub fn get_effective_schedule(
    conn: &mut SqliteConnection,
    site_id: i32,
    date: chrono::NaiveDate,
) -> Result<EffectiveScheduleResponse, diesel::result::Error> {
    use crate::{orm::schedule_library::get_library_items_for_site, schema::application_rules};

    // 1. Get all library items for site
    let items = get_library_items_for_site(conn, site_id)?;
    if items.is_empty() {
        return Err(diesel::result::Error::NotFound);
    }

    let item_ids: Vec<i32> = items.iter().map(|i| i.id).collect();

    // 2. Get all application rules for these items
    let rules_db: Vec<ApplicationRuleDb> = application_rules::table
        .filter(application_rules::template_id.eq_any(&item_ids))
        .load(conn)?;

    // 3. Convert to API models
    let rules: Vec<ApplicationRule> =
        rules_db.into_iter().filter_map(|r| r.to_api_model().ok()).collect();

    // 4. Match rules to date
    let date_string = date.format("%Y-%m-%d").to_string();
    let day_of_week = chrono::Datelike::weekday(&date).num_days_from_sunday() as i32; // 0=Sunday

    let mut matching_rules: Vec<(ApplicationRule, i32)> = rules
        .into_iter()
        .filter_map(|rule| {
            let specificity = match &rule.rule_type {
                RuleType::SpecificDate => {
                    if let Some(ref dates) = rule.specific_dates {
                        if dates.contains(&date_string) {
                            Some(2)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                RuleType::DayOfWeek => {
                    if let Some(ref days) = rule.days_of_week {
                        if days.contains(&day_of_week) {
                            Some(1)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                RuleType::Default => Some(0),
            };
            specificity.map(|s| (rule, s))
        })
        .collect();

    // 5. Sort by specificity DESC, then created_at DESC
    matching_rules.sort_by(|(rule_a, spec_a), (rule_b, spec_b)| {
        spec_b.cmp(spec_a).then_with(|| rule_b.created_at.cmp(&rule_a.created_at))
    });

    // 6. Get winning rule
    let (winning_rule, specificity) =
        matching_rules.first().ok_or(diesel::result::Error::NotFound)?;

    // 7. Get library item for winning rule
    let library_item = items
        .into_iter()
        .find(|item| item.id == winning_rule.library_item_id)
        .ok_or(diesel::result::Error::NotFound)?;

    Ok(EffectiveScheduleResponse {
        library_item,
        specificity: *specificity,
        rule: winning_rule.clone(),
    })
}

/// Gets ALL matching schedules for a specific date (not just the winning one)
/// Returns the winning match and all other matches with lower priority
pub fn get_all_matching_schedules(
    conn: &mut SqliteConnection,
    site_id: i32,
    date: chrono::NaiveDate,
) -> Result<CalendarDayScheduleMatches, diesel::result::Error> {
    use crate::{
        models::application_rule::{CalendarDayScheduleMatches, CalendarScheduleMatch},
        orm::schedule_library::get_library_items_for_site,
        schema::application_rules,
    };

    // 1. Get all library items for site
    let items = get_library_items_for_site(conn, site_id)?;
    if items.is_empty() {
        return Err(diesel::result::Error::NotFound);
    }

    let item_ids: Vec<i32> = items.iter().map(|i| i.id).collect();

    // 2. Get all application rules for these items
    let rules_db: Vec<ApplicationRuleDb> = application_rules::table
        .filter(application_rules::template_id.eq_any(&item_ids))
        .load(conn)?;

    // 3. Convert to API models
    let rules: Vec<ApplicationRule> =
        rules_db.into_iter().filter_map(|r| r.to_api_model().ok()).collect();

    // 4. Match rules to date
    let date_string = date.format("%Y-%m-%d").to_string();
    let day_of_week = chrono::Datelike::weekday(&date).num_days_from_sunday() as i32; // 0=Sunday

    let mut matching_rules: Vec<(ApplicationRule, i32)> = rules
        .into_iter()
        .filter_map(|rule| {
            let specificity = match &rule.rule_type {
                RuleType::SpecificDate => {
                    if let Some(ref dates) = rule.specific_dates {
                        if dates.contains(&date_string) {
                            Some(2)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                RuleType::DayOfWeek => {
                    if let Some(ref days) = rule.days_of_week {
                        if days.contains(&day_of_week) {
                            Some(1)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                RuleType::Default => Some(0),
            };
            specificity.map(|s| (rule, s))
        })
        .collect();

    if matching_rules.is_empty() {
        return Err(diesel::result::Error::NotFound);
    }

    // 5. Sort by specificity DESC, then created_at DESC
    matching_rules.sort_by(|(rule_a, spec_a), (rule_b, spec_b)| {
        spec_b.cmp(spec_a).then_with(|| rule_b.created_at.cmp(&rule_a.created_at))
    });

    // 6. Convert all matches to CalendarScheduleMatch format
    // Deduplicate by library_item_id - if a schedule has multiple rules applying
    // to this date (e.g., default + specific date), only include the highest
    // priority one
    let mut seen_library_item_ids = std::collections::HashSet::new();
    let all_matches: Vec<CalendarScheduleMatch> = matching_rules
        .into_iter()
        .filter_map(|(rule, specificity)| {
            // Skip if we've already seen this library item (higher priority rule already
            // processed)
            if !seen_library_item_ids.insert(rule.library_item_id) {
                return None;
            }

            // Find the library item for this rule
            items.iter().find(|item| item.id == rule.library_item_id).map(|library_item| {
                CalendarScheduleMatch {
                    library_item_id: library_item.id,
                    library_item_name: library_item.name.clone(),
                    specificity,
                    rule_id: rule.id,
                    rule_type: rule.rule_type.clone(),
                    override_reason: rule.override_reason.clone(),
                }
            })
        })
        .collect();

    if all_matches.is_empty() {
        return Err(diesel::result::Error::NotFound);
    }

    // 7. Separate winning match from other matches
    let mut matches_iter = all_matches.into_iter();
    let winning_match = matches_iter.next().ok_or(diesel::result::Error::NotFound)?;
    let other_matches: Vec<CalendarScheduleMatch> = matches_iter.collect();

    Ok(CalendarDayScheduleMatches { winning_match, other_matches })
}

/// Gets calendar schedules for a month
/// Returns a HashMap of date strings to schedule assignments
pub fn get_calendar_schedules(
    conn: &mut SqliteConnection,
    site_id: i32,
    year: i32,
    month: u32,
) -> Result<HashMap<String, CalendarDaySchedule>, diesel::result::Error> {
    use chrono::NaiveDate;

    // Ensure default schedule exists for this site
    use super::schedule_library::ensure_default_schedule_exists;
    let _ = ensure_default_schedule_exists(conn, site_id, None);

    // Validate month
    if !(1..=12).contains(&month) {
        return Err(diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid month",
        ))));
    }

    // Get the first and last day of the month
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid date",
        )))
    })?;

    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_month_year = if month == 12 { year + 1 } else { year };
    let last_day = NaiveDate::from_ymd_opt(next_month_year, next_month, 1)
        .ok_or_else(|| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid date",
            )))
        })?
        .pred_opt()
        .ok_or_else(|| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid date",
            )))
        })?;

    let mut result = HashMap::new();
    let mut current_day = first_day;

    while current_day <= last_day {
        match get_effective_schedule(conn, site_id, current_day) {
            Ok(schedule) => {
                result.insert(
                    current_day.format("%Y-%m-%d").to_string(),
                    CalendarDaySchedule {
                        library_item_id: schedule.library_item.id,
                        library_item_name: schedule.library_item.name,
                        specificity: schedule.specificity,
                        rule_id: schedule.rule.id,
                    },
                );
            }
            Err(_) => {
                // No schedule for this day, skip
            }
        }

        current_day = current_day.succ_opt().ok_or_else(|| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid date",
            )))
        })?;
    }

    Ok(result)
}

/// Gets calendar schedules with ALL matches for a month
/// Returns a HashMap of date strings to all matching schedule assignments
pub fn get_calendar_schedules_with_matches(
    conn: &mut SqliteConnection,
    site_id: i32,
    year: i32,
    month: u32,
) -> Result<HashMap<String, CalendarDayScheduleMatches>, diesel::result::Error> {
    use chrono::NaiveDate;

    // Ensure default schedule exists for this site
    use super::schedule_library::ensure_default_schedule_exists;
    let _ = ensure_default_schedule_exists(conn, site_id, None);

    // Validate month
    if !(1..=12).contains(&month) {
        return Err(diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid month",
        ))));
    }

    // Get the first and last day of the month
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid date",
        )))
    })?;

    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_month_year = if month == 12 { year + 1 } else { year };
    let last_day = NaiveDate::from_ymd_opt(next_month_year, next_month, 1)
        .ok_or_else(|| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid date",
            )))
        })?
        .pred_opt()
        .ok_or_else(|| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid date",
            )))
        })?;

    let mut result = HashMap::new();
    let mut current_day = first_day;

    while current_day <= last_day {
        match get_all_matching_schedules(conn, site_id, current_day) {
            Ok(matches) => {
                result.insert(current_day.format("%Y-%m-%d").to_string(), matches);
            }
            Err(_) => {
                // No schedule for this day, skip
            }
        }

        current_day = current_day.succ_opt().ok_or_else(|| {
            diesel::result::Error::DeserializationError(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid date",
            )))
        })?;
    }

    Ok(result)
}
