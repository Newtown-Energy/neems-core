use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::schema::sources;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = sources)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Source {
    pub id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub interval_seconds: i32,
    pub last_run: Option<NaiveDateTime>,
    pub test_type: Option<String>,
    pub arguments: Option<String>, // JSON string
    pub site_id: Option<i32>,
    pub company_id: Option<i32>,
}

impl Source {
    /// Parse the arguments JSON string into a HashMap
    pub fn get_arguments(&self) -> Result<HashMap<String, String>, serde_json::Error> {
        match &self.arguments {
            Some(args) => serde_json::from_str(args),
            None => Ok(HashMap::new()),
        }
    }

    /// Set arguments from a HashMap, serializing to JSON
    pub fn set_arguments(&mut self, args: &HashMap<String, String>) -> Result<(), serde_json::Error> {
        self.arguments = Some(serde_json::to_string(args)?);
        Ok(())
    }
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = sources)]
pub struct NewSource {
    pub name: String,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub interval_seconds: Option<i32>,
    pub test_type: Option<String>,
    pub arguments: Option<String>, // JSON string
    pub site_id: Option<i32>,
    pub company_id: Option<i32>,
}

impl NewSource {
    /// Create a NewSource with arguments from a HashMap
    pub fn with_arguments(
        name: String,
        test_type: String,
        arguments: &HashMap<String, String>,
        description: Option<String>,
        active: Option<bool>,
        interval_seconds: Option<i32>,
        site_id: Option<i32>,
        company_id: Option<i32>,
    ) -> Result<Self, serde_json::Error> {
        Ok(NewSource {
            name,
            description,
            active,
            interval_seconds,
            test_type: Some(test_type),
            arguments: Some(serde_json::to_string(arguments)?),
            site_id,
            company_id,
        })
    }
}

#[derive(Debug, Clone, AsChangeset, Serialize, Deserialize)]
#[diesel(table_name = sources)]
pub struct UpdateSource {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub active: Option<bool>,
    pub interval_seconds: Option<i32>,
    pub last_run: Option<Option<NaiveDateTime>>,
    pub test_type: Option<String>,
    pub arguments: Option<String>, // JSON string
    pub site_id: Option<Option<i32>>,
    pub company_id: Option<Option<i32>>,
}

impl UpdateSource {
    /// Set arguments from a HashMap
    pub fn with_arguments(mut self, args: &HashMap<String, String>) -> Result<Self, serde_json::Error> {
        self.arguments = Some(serde_json::to_string(args)?);
        Ok(self)
    }
}
