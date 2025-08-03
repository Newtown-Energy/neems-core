//! Logged JSON request guard that captures and logs request bodies.
//!
//! This module provides a wrapper around Rocket's Json type that automatically
//! logs the parsed JSON data for debugging purposes. It's a drop-in replacement
//! for Json<T> in your API endpoints.

use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::{
    Data, Request,
    data::{self, FromData},
};

/// A wrapper around Rocket's Json that logs the request data.
///
/// This is a drop-in replacement for Json<T> that automatically logs
/// the parsed JSON data to help with debugging API requests.
pub struct LoggedJson<T>(pub T);

impl<T> LoggedJson<T> {
    /// Extract the inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for LoggedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for LoggedJson<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[rocket::async_trait]
impl<'r, T: Deserialize<'r> + Serialize> FromData<'r> for LoggedJson<T> {
    type Error = rocket::serde::json::Error<'r>;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        // Use Rocket's built-in Json parser first
        match Json::<T>::from_data(req, data).await {
            data::Outcome::Success(json_data) => {
                // Log the parsed and re-serialized data
                match serde_json::to_string(&json_data.0) {
                    Ok(json_string) => {
                        let message = format!(
                            "API Request Body: {} {} | Data: {}",
                            req.method().as_str(),
                            req.uri().path(),
                            json_string
                        );
                        info!("{}", message);
                    }
                    Err(_) => {
                        info!(
                            "API Request Body: {} {} | Data: <failed to serialize>",
                            req.method().as_str(),
                            req.uri().path()
                        );
                    }
                }
                data::Outcome::Success(LoggedJson(json_data.into_inner()))
            }
            data::Outcome::Error(e) => data::Outcome::Error(e),
            data::Outcome::Forward(f) => data::Outcome::Forward(f),
        }
    }
}

// Implement common traits for convenience
impl<T: Serialize> Serialize for LoggedJson<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<T: Clone> Clone for LoggedJson<T> {
    fn clone(&self) -> Self {
        LoggedJson(self.0.clone())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for LoggedJson<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
