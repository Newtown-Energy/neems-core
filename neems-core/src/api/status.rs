/*
API version 1 - Status endpoints
*/

use serde::Serialize;
use rocket::serde::json::Json;
use rocket::Route;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct HealthStatus {
    status: &'static str,
}

#[rocket::get("/1/status")]
pub fn health_status() -> Json<HealthStatus> {
    Json(HealthStatus { status: "running" })
}

pub fn routes() -> Vec<Route> {
    routes![health_status]
}