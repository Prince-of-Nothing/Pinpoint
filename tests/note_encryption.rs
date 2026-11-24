//! Private notes are encrypted before they reach the store/Postgres, while the
//! API keeps returning plaintext to the owner.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use pinpoint_desktop::models::PlaceNote;
use pinpoint_desktop::security::encryption::{AesGcmEncryption, EncryptionKey};
use pinpoint_desktop::server::{router, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

fn cipher() -> Arc<AesGcmEncryption> {
    let key = EncryptionKey::from_bytes(&[7u8; 32]).expect("32-byte key");
    Arc::new(AesGcmEncryption::new(key))
}

async fn register(app: &Router, email: &str) -> (String, String) {
    let body = json!({
        "email": email,
        "password": "StrongPass!2026",
        "full_name": "Note Tester"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let csrf = body["csrf_token"].as_str().expect("csrf").to_string();
    (cookie, csrf)
}

async fn create_note(app: &Router, cookie: &str, csrf: &str) -> Value {
    let body = json!({
        "latitude": 47.01,
        "longitude": 28.86,
        "label": "Secret cafe",
        "message": "private message body",
        "photo": "data:image/png;base64,AAAA"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/locations/notes")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn list_notes(app: &Router, cookie: &str) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/locations/notes")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn notes_are_stored_encrypted_and_returned_in_plaintext() {
    let mut state = AppState::new();
    state.data_cipher = Some(cipher());
    let app = router(state.clone());
    let (cookie, csrf) = register(&app, "cipher@example.invalid").await;

    let created = create_note(&app, &cookie, &csrf).await;
    assert_eq!(created["label"], "Secret cafe");
    assert_eq!(created["message"], "private message body");

    let stored = {
        let store = state.store.lock().unwrap();
        store.place_notes[0].clone()
    };
    assert!(stored.label.starts_with("enc:v1:"));
    assert!(stored.message.starts_with("enc:v1:"));
    assert!(stored
        .photo
        .as_deref()
        .is_some_and(|photo| photo.starts_with("enc:v1:")));
    assert!(!stored.message.contains("private message body"));

    let listed = list_notes(&app, &cookie).await;
    assert_eq!(listed[0]["label"], "Secret cafe");
    assert_eq!(listed[0]["message"], "private message body");
    assert_eq!(listed[0]["photo"], "data:image/png;base64,AAAA");
}

#[tokio::test]
async fn legacy_plaintext_notes_stay_readable() {
    let mut state = AppState::new();
    state.data_cipher = Some(cipher());
    let app = router(state.clone());
    let (cookie, _csrf) = register(&app, "legacy@example.invalid").await;
    {
        let mut store = state.store.lock().unwrap();
        store.place_notes.push(PlaceNote {
            id: 99,
            user_id: 1,
            latitude: 47.0,
            longitude: 28.8,
            label: "Old note".to_string(),
            message: "written before encryption".to_string(),
            photo: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        });
    }
    let listed = list_notes(&app, &cookie).await;
    assert_eq!(listed[0]["label"], "Old note");
    assert_eq!(listed[0]["message"], "written before encryption");
}

#[tokio::test]
async fn without_a_key_notes_stay_plaintext_for_development() {
    let state = AppState::new();
    assert!(state.data_cipher.is_none());
    let app = router(state.clone());
    let (cookie, csrf) = register(&app, "dev@example.invalid").await;
    create_note(&app, &cookie, &csrf).await;
    let stored = {
        let store = state.store.lock().unwrap();
        store.place_notes[0].clone()
    };
    assert_eq!(stored.message, "private message body");
}
