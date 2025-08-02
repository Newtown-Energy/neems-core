#[cfg(feature = "fixphrase")]
use neems_core::api::fixphrase::{FixPhraseError, FixPhraseResponse};
#[cfg(feature = "fixphrase")]
use neems_core::orm::testing::test_rocket_no_db;
#[cfg(feature = "fixphrase")]
use rocket::http::Status;
#[cfg(feature = "fixphrase")]
use rocket::local::asynchronous::Client;

#[cfg(feature = "fixphrase")]
#[rocket::async_test]
async fn test_encode_fixphrase_success() {
    // 1. Set up test client
    let client = Client::tracked(test_rocket_no_db())
        .await
        .expect("valid rocket instance");

    // 2. Send request to API
    let response = client
        .get("/api/1/fixphrase/encode/42.3601/-71.0589")
        .dispatch()
        .await;

    // 3. Verify response
    assert_eq!(response.status(), Status::Ok);
    let body: FixPhraseResponse = response.into_json().await.unwrap();

    // 4. Check accuracy (same logic as unit tests)
    let expected_lat = 42.3601;
    let expected_lon = -71.0589;
    assert!((body.latitude - expected_lat).abs() < body.accuracy);
    assert!((body.longitude - expected_lon).abs() < body.accuracy);
    assert!(!body.phrase.is_empty());
}

#[cfg(feature = "fixphrase")]
#[rocket::async_test]
async fn test_encode_fixphrase_invalid_coords() {
    let client = Client::tracked(test_rocket_no_db())
        .await
        .expect("valid rocket instance");

    // Test invalid latitude
    let response = client
        .get("/api/1/fixphrase/encode/91.0/0.0")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
    let error: FixPhraseError = response.into_json().await.unwrap();
    assert!(matches!(error, FixPhraseError::InvalidLatitude));

    // Test invalid longitude
    let response = client
        .get("/api/1/fixphrase/encode/0.0/181.0")
        .dispatch()
        .await;
    let error: FixPhraseError = response.into_json().await.unwrap();
    assert!(matches!(error, FixPhraseError::InvalidLongitude));
}

#[cfg(feature = "fixphrase")]
#[rocket::async_test]
async fn test_api_response_structure() {
    let client = Client::tracked(test_rocket_no_db())
        .await
        .expect("valid rocket instance");
    let response = client
        .get("/api/1/fixphrase/encode/42.1409/-76.8518")
        .dispatch()
        .await;

    let body: FixPhraseResponse = response.into_json().await.unwrap();

    // Verify exact phrase for known coordinates
    assert_eq!(body.phrase, "corrode ground slacks washbasin");
    assert!((body.latitude - 42.1409).abs() < body.accuracy);
    assert!((body.longitude - (-76.8518)).abs() < body.accuracy);
}
