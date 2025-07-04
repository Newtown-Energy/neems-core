/*
API version 1
*/

use rocket::serde::{Serialize, Deserialize};
use rocket::serde::json::Json;
use rocket::response::status as rocket_status;

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
pub struct Status {
    status: &'static str,
}

#[rocket::get("/1/status")]
pub fn status() -> Json<Status> {
    Json(Status { status: "running" })
}
