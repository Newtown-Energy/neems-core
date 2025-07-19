//! API version 1 - FixPhrase endpoints
//!
//! This module provides HTTP endpoints for encoding and decoding FixPhrase location data.
//! FixPhrase is a location encoding system that converts latitude/longitude coordinates
//! into human-readable phrases.

use serde::{Deserialize, Serialize};
use rocket::http::Status as HttpStatus;
use rocket::serde::json::Json;
use rocket::response::status as rocket_status;
use rocket::Route;

pub use fixphrase::{FixPhrase, FixPhraseError};

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct FixPhraseResponse {
    pub phrase: String,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: f64,
}

/// FixPhrase Encoding endpoint.
///
/// - **URL:** `/api/1/fixphrase/encode/<lat>/<lon>`
/// - **Method:** `GET`
/// - **Purpose:** Encodes latitude/longitude coordinates into a FixPhrase string
/// - **Authentication:** None required
///
/// This endpoint takes latitude and longitude coordinates and returns a FixPhrase
/// representation along with the decoded coordinates and accuracy information.
///
/// # Parameters
///
/// - `lat` - Latitude coordinate (between -90 and 90)
/// - `lon` - Longitude coordinate (between -180 and 180)
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "phrase": "example.fixphrase.string",
///   "latitude": 40.7128,
///   "longitude": -74.0060,
///   "accuracy": 10.0
/// }
/// ```
///
/// **Failure (HTTP 400 Bad Request):**
/// ```json
/// {
///   "error": "Invalid coordinates"
/// }
/// ```
///
/// # Arguments
/// * `lat` - The latitude coordinate (must be between -90 and 90)
/// * `lon` - The longitude coordinate (must be between -180 and 180)
///
/// # Returns
/// * `Ok(Json<FixPhraseResponse>)` - Successfully encoded FixPhrase with decoded verification
/// * `Err(rocket_status::Custom<Json<FixPhraseError>>)` - Error during encoding or decoding
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/fixphrase/encode/40.7128/-74.0060');
/// ```
#[rocket::get("/1/fixphrase/encode/<lat>/<lon>")]
pub fn encode_fixphrase(
    lat: f64, 
    lon: f64
) -> Result<Json<FixPhraseResponse>, rocket_status::Custom<Json<FixPhraseError>>> {
    match FixPhrase::encode(lat, lon) {
        Ok(phrase) => {
            match FixPhrase::decode(&phrase) {
                Ok((decoded_lat, decoded_lon, accuracy, _)) => {
                    Ok(Json(FixPhraseResponse {
                        phrase,
                        latitude: decoded_lat,
                        longitude: decoded_lon,
                        accuracy,
                    }))
                }
                Err(e) => Err(rocket_status::Custom(HttpStatus::BadRequest, Json(e))),
            }
        }
        Err(e) => Err(rocket_status::Custom(HttpStatus::BadRequest, Json(e))),
    }
}

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for FixPhrase endpoints
pub fn routes() -> Vec<Route> {
    routes![encode_fixphrase]
}