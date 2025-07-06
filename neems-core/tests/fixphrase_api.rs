use rocket::local::blocking::Client;
use rocket::http::{Status};
use neems_core::api::{FixPhraseResponse, FixPhraseError}; 
use neems_core::rocket;

#[test]
fn test_encode_fixphrase_success() {
    // 1. Set up test client
    let client = Client::tracked(rocket()).expect("valid rocket instance");

    // 2. Send request to API
    let response = client
        .get("/api/1/fixphrase/encode/42.3601/-71.0589")
        .dispatch();

    // 3. Verify response
    assert_eq!(response.status(), Status::Ok);
    let body: FixPhraseResponse = response.into_json().unwrap();
    
    // 4. Check accuracy (same logic as unit tests)
    let expected_lat = 42.3601;
    let expected_lon = -71.0589;
    assert!((body.latitude - expected_lat).abs() < body.accuracy);
    assert!((body.longitude - expected_lon).abs() < body.accuracy);
    assert!(!body.phrase.is_empty());
}

#[test]
fn test_encode_fixphrase_invalid_coords() {
    let client = Client::tracked(rocket()).unwrap();

    // Test invalid latitude
    let response = client
        .get("/api/1/fixphrase/encode/91.0/0.0")
        .dispatch();
    assert_eq!(response.status(), Status::BadRequest);
    let error: FixPhraseError = response.into_json().unwrap();
    assert!(matches!(error, FixPhraseError::InvalidLatitude));

    // Test invalid longitude
    let response = client
        .get("/api/1/fixphrase/encode/0.0/181.0")
        .dispatch();
    let error: FixPhraseError = response.into_json().unwrap();
    assert!(matches!(error, FixPhraseError::InvalidLongitude));
}

#[test]
fn test_api_response_structure() {
    let client = Client::tracked(rocket()).unwrap();
    let response = client
        .get("/api/1/fixphrase/encode/42.1409/-76.8518")
        .dispatch();

    let body: FixPhraseResponse = response.into_json().unwrap();
    
    // Verify exact phrase for known coordinates
    assert_eq!(body.phrase, "corrode ground slacks washbasin");
    assert!((body.latitude - 42.1409).abs() < body.accuracy);
    assert!((body.longitude - (-76.8518)).abs() < body.accuracy);
}
