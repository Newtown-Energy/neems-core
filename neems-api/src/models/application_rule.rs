use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::application_rules;

/// Type of application rule
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Default,
    DayOfWeek,
    SpecificDate,
}

/// Database model for application rules
#[derive(
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    QueryableByName,
    Debug,
    Clone,
    Serialize,
    Deserialize,
)]
#[diesel(belongs_to(super::schedule_library::ScheduleTemplate, foreign_key = template_id))]
#[diesel(table_name = application_rules)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ApplicationRuleDb {
    pub id: i32,
    pub template_id: i32,
    pub rule_type: String,
    pub days_of_week: Option<String>,
    pub specific_dates: Option<String>,
    pub override_reason: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

/// Insertable struct for creating new application rules
#[derive(Insertable, Debug)]
#[diesel(table_name = application_rules)]
pub struct NewApplicationRule {
    pub template_id: i32,
    pub rule_type: String,
    pub days_of_week: Option<String>,
    pub specific_dates: Option<String>,
    pub override_reason: Option<String>,
}

// ============================================================================
// API Models (exported to TypeScript)
// ============================================================================

/// Application rule determining when a schedule applies (API model)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApplicationRule {
    pub id: i32,
    pub library_item_id: i32, // Maps to template_id in DB
    pub rule_type: RuleType,
    pub days_of_week: Option<Vec<i32>>, // 0=Sunday, 6=Saturday
    pub specific_dates: Option<Vec<String>>, // ISO date strings
    pub override_reason: Option<String>,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
}

/// Request to create an application rule
#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateApplicationRuleRequest {
    pub rule_type: RuleType,
    pub days_of_week: Option<Vec<i32>>,
    pub specific_dates: Option<Vec<String>>,
    pub override_reason: Option<String>,
}

/// Response with effective schedule for a date
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EffectiveScheduleResponse {
    pub library_item: super::schedule_library::ScheduleLibraryItem,
    pub specificity: i32, // 0=default, 1=day_of_week, 2=specific_date
    pub rule: ApplicationRule,
}

/// Calendar day schedule assignment
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CalendarDaySchedule {
    pub library_item_id: i32,
    pub library_item_name: String,
    pub specificity: i32,
    pub rule_id: i32,
}

/// Individual schedule match with full rule information
#[derive(Debug, Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct CalendarScheduleMatch {
    pub library_item_id: i32,
    pub library_item_name: String,
    pub specificity: i32,
    pub rule_id: i32,
    pub rule_type: RuleType,
    pub override_reason: Option<String>,
}

/// All matching schedules for a calendar day
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CalendarDayScheduleMatches {
    pub winning_match: CalendarScheduleMatch,
    pub other_matches: Vec<CalendarScheduleMatch>,
}

// Helper functions for RuleType
impl RuleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleType::Default => "default",
            RuleType::DayOfWeek => "day_of_week",
            RuleType::SpecificDate => "specific_date",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "default" => Ok(RuleType::Default),
            "day_of_week" => Ok(RuleType::DayOfWeek),
            "specific_date" => Ok(RuleType::SpecificDate),
            _ => Err(format!("Unknown rule type: {}", s)),
        }
    }
}

// Conversion from database model to API model
impl ApplicationRuleDb {
    pub fn to_api_model(&self) -> Result<ApplicationRule, String> {
        let rule_type = RuleType::from_str(&self.rule_type)?;

        let days_of_week = if let Some(ref json_str) = self.days_of_week {
            Some(serde_json::from_str(json_str).map_err(|e| e.to_string())?)
        } else {
            None
        };

        let specific_dates = if let Some(ref json_str) = self.specific_dates {
            Some(serde_json::from_str(json_str).map_err(|e| e.to_string())?)
        } else {
            None
        };

        Ok(ApplicationRule {
            id: self.id,
            library_item_id: self.template_id,
            rule_type,
            days_of_week,
            specific_dates,
            override_reason: self.override_reason.clone(),
            created_at: self.created_at,
        })
    }
}
