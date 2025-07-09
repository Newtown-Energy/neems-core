/*
API version 1
*/

use serde::{Deserialize, Serialize};
use rocket::http::Status as HttpStatus;
use rocket::serde::json::Json;
use rocket::response::status as rocket_status;
use rocket::Route;

pub use fixphrase::{FixPhrase, FixPhraseError};


#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct HealthStatus {
    status: &'static str,
}

#[rocket::get("/1/status")]
pub fn health_status() -> Json<HealthStatus> {
    Json(HealthStatus { status: "running" })
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct FixPhraseResponse {
    pub phrase: String,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: f64,
}

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

pub fn routes() -> Vec<Route> {
    routes![health_status, encode_fixphrase, ]
}
