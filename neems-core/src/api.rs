/*
API version 1
*/

use serde::{Deserialize, Serialize};
use rocket::http::Status as HttpStatus;
use rocket::serde::json::Json;
use rocket::response::status as rocket_status;

pub use fixphrase::{FixPhrase, FixPhraseError};

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Client {
    name: String,  // Changed from &'static str to String for POST payload
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Clients {
    clients: Vec<Client>,
}

// GET all clients
#[rocket::get("/1/clients")]
pub fn get_clients() -> Json<Clients> {
    Json(Clients {
        clients: vec![
            Client { name: "Doctor Voltage".into() },
            Client { name: "Corona Solar".into() },
            Client { name: "South Bump Energy".into() },
        ]
    })
}

// POST a new client
#[rocket::post("/1/clients", data = "<client>")]
pub fn create_client(client: Json<Client>) -> rocket_status::Created<Json<Client>> {
    let response = client.into_inner();

    rocket_status::Created::new("/1/clients/")  // Location header
        .body(Json(response))           // Response body
}

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

// #[rocket::get("/1/fixphrase/encode/<lat>/<lon>")]
// pub fn encode_fixphrase(lat: f64, lon: f64) -> Result<Json<FixPhraseResponse>, Json<FixPhraseError>> {
//     match FixPhrase::encode(lat, lon) {
//         Ok(phrase) => {
//             // Decode to get accuracy information
//             match FixPhrase::decode(&phrase) {
//                 Ok((decoded_lat, decoded_lon, accuracy, _)) => {
//                     Ok(Json(FixPhraseResponse {
//                         phrase,
//                         latitude: decoded_lat,
//                         longitude: decoded_lon,
//                         accuracy,
//                     }))
//                 }
//                 Err(e) => Err(Json(e)),
//             }
//         }
//         Err(e) => Err(Json(e)),
//     }
// }
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
