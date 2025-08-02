use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::schema::readings;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = readings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Reading {
    pub id: Option<i32>,
    pub source_id: i32,
    pub timestamp: NaiveDateTime,
    pub data: String, // JSON string
    pub quality_flags: i32,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = readings)]
pub struct NewReading {
    pub source_id: i32,
    pub timestamp: Option<NaiveDateTime>,
    pub data: String, // JSON string
    pub quality_flags: Option<i32>,
}

impl Reading {
    /// Parse the JSON data field into a serde_json::Value
    pub fn parse_data(&self) -> Result<JsonValue, serde_json::Error> {
        serde_json::from_str(&self.data)
    }
}

impl NewReading {
    /// Create a new reading with JSON data
    pub fn with_json_data(source_id: i32, data: &JsonValue) -> Result<Self, serde_json::Error> {
        Ok(Self {
            source_id,
            timestamp: None, // Will use database default
            data: serde_json::to_string(data)?,
            quality_flags: None, // Will use database default (0)
        })
    }

    /// Create a new reading with quality flags
    pub fn with_quality(
        source_id: i32,
        data: &JsonValue,
        quality_flags: i32,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            source_id,
            timestamp: None,
            data: serde_json::to_string(data)?,
            quality_flags: Some(quality_flags),
        })
    }
}
