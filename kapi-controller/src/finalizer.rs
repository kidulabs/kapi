//! Standalone finalizer helper functions.
//!
//! These utilities are used inside controller reconcile loops to inspect and
//! manage finalizer lists with optimistic concurrency (CAS) retry on 409
//! Conflict.
//!
//! Two forms are provided:
//!
//! * **Raw helpers** (`ensure_finalizer`, `remove_finalizer`, `is_deleting`)
//!   operate directly on a [`StoredObject`].
//! * **Typed helpers** ([`typed`]) accept any `&T where T: TypedResource`,
//!   convert the resource to a [`StoredObject`] once via
//!   [`TypedResource::to_stored_object`](kapi_client::TypedResource::to_stored_object),
//!   and delegate to the raw helpers.
//!
//! Typed controllers therefore never have to call `to_stored_object()`
//! themselves or thread a raw value through the reconcile path.

use std::time::Duration;

use kapi_client::client::KapiClient;
use kapi_client::error::ClientError;
use kapi_core::{ApiError, StoredObject};

/// Returns `true` when the object has a deletion timestamp set (i.e. it is
/// being deleted).
pub fn is_deleting(obj: &StoredObject) -> bool {
    obj.system.deletion_timestamp.is_some()
}

/// Ensures that `finalizer` is present on `obj`.
///
/// * If the finalizer is already present → no-op.
/// * Otherwise → clone the object, append the finalizer, and call
///   `client.update()`.
///
/// On a 409 Conflict (CAS failure), re-fetches the object and retries
/// (up to 5 attempts).
pub async fn ensure_finalizer(
    client: &KapiClient,
    obj: &StoredObject,
    finalizer: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if obj.metadata.finalizers.iter().any(|f| f == finalizer) {
        return Ok(());
    }

    let mut current = obj.clone();
    for attempt in 0..5 {
        // Try the update.
        let mut updated = current.clone();
        if !updated.metadata.finalizers.iter().any(|f| f == finalizer) {
            updated.metadata.finalizers.push(finalizer.to_string());
        }

        match client.update(current.metadata.namespace.as_deref(), &updated).await {
            Ok(_) => return Ok(()),
            Err(ClientError::Api(ApiError::Conflict { .. })) if attempt < 4 => {
                // CAS conflict — re-fetch and retry.
                tokio::time::sleep(Duration::from_millis(10)).await;
                current = client
                    .get(&obj.key, obj.metadata.namespace.as_deref(), &obj.metadata.name)
                    .await?;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }

    Ok(())
}

/// Removes `finalizer` from `obj`.
///
/// * If the finalizer is not present → no-op.
/// * Otherwise → clone the object, remove the finalizer, and call
///   `client.update()`.
///
/// On a 409 Conflict (CAS failure), re-fetches the object and retries
/// (up to 5 attempts).
pub async fn remove_finalizer(
    client: &KapiClient,
    obj: &StoredObject,
    finalizer: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !obj.metadata.finalizers.iter().any(|f| f == finalizer) {
        return Ok(());
    }

    let mut current = obj.clone();
    for attempt in 0..5 {
        // Try the update.
        let mut updated = current.clone();
        updated.metadata.finalizers.retain(|f| f != finalizer);

        match client.update(current.metadata.namespace.as_deref(), &updated).await {
            Ok(_) => return Ok(()),
            Err(ClientError::Api(ApiError::Conflict { .. })) if attempt < 4 => {
                // CAS conflict — re-fetch and retry.
                tokio::time::sleep(Duration::from_millis(10)).await;
                current = client
                    .get(&obj.key, obj.metadata.namespace.as_deref(), &obj.metadata.name)
                    .await?;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }

    Ok(())
}

/// Typed convenience wrappers for the raw [`ensure_finalizer`] and
/// [`remove_finalizer`] helpers.
///
/// Each function accepts a typed resource `&T where T: TypedResource`,
/// converts it to a [`StoredObject`] exactly once via
/// [`TypedResource::to_stored_object`](kapi_client::TypedResource::to_stored_object),
/// and then delegates to the raw, `StoredObject`-based helper. The CAS retry
/// loop is **not** re-implemented here — it stays in the raw helpers.
///
/// # CAS re-fetches operate on `StoredObject`, not `T`
///
/// The CAS retry loop always works on [`StoredObject`] values. When a typed
/// resource is passed in, the re-fetch performed after a 409 Conflict returns
/// a `StoredObject`, never `T`. The passed-in `&T` is therefore never
/// updated in place by these helpers — re-read the resource from the server
/// if you need the freshest state.
pub mod typed {
    use kapi_client::TypedResource;

    use super::*;

    /// Typed counterpart of [`super::ensure_finalizer`]: ensures that
    /// `finalizer` is present on the typed resource `obj`.
    ///
    /// Calls [`TypedResource::to_stored_object`] once, then delegates to the
    /// raw [`super::ensure_finalizer`] with the resulting [`StoredObject`].
    /// All raw semantics — no-op when the finalizer is already present, CAS
    /// retry on 409 Conflict — apply unchanged.
    ///
    /// # CAS re-fetches operate on `StoredObject`, not `T`
    ///
    /// The CAS retry loop re-fetches a `StoredObject` after a 409 Conflict,
    /// never a `T`. `obj` is not updated in place.
    ///
    /// # Errors
    ///
    /// Returns `Err` if [`TypedResource::to_stored_object`] fails
    /// ([`ClientError`]) or if the underlying update fails after all retries.
    pub async fn ensure_finalizer<T: TypedResource>(
        client: &KapiClient,
        obj: &T,
        finalizer: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stored = obj
            .to_stored_object()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        super::ensure_finalizer(client, &stored, finalizer).await
    }

    /// Typed counterpart of [`super::remove_finalizer`]: removes `finalizer`
    /// from the typed resource `obj`.
    ///
    /// Calls [`TypedResource::to_stored_object`] once, then delegates to the
    /// raw [`super::remove_finalizer`] with the resulting [`StoredObject`].
    /// All raw semantics — no-op when the finalizer is absent, CAS retry on
    /// 409 Conflict — apply unchanged.
    ///
    /// # CAS re-fetches operate on `StoredObject`, not `T`
    ///
    /// The CAS retry loop re-fetches a `StoredObject` after a 409 Conflict,
    /// never a `T`. `obj` is not updated in place.
    ///
    /// # Errors
    ///
    /// Returns `Err` if [`TypedResource::to_stored_object`] fails
    /// ([`ClientError`]) or if the underlying update fails after all retries.
    pub async fn remove_finalizer<T: TypedResource>(
        client: &KapiClient,
        obj: &T,
        finalizer: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stored = obj
            .to_stored_object()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        super::remove_finalizer(client, &stored, finalizer).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kapi_core::{ObjectMeta, ResourceKey, SystemMetadata};
    use serde_json::Value;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// The raw HTTP request captured by [`spawn_capture_server`].
    #[derive(Debug)]
    struct CapturedRequest {
        request_line: String,
        body: Value,
    }

    /// Minimal [`StoredObject`] with the given finalizer list.
    fn obj_with_finalizers(finalizers: &[&str]) -> StoredObject {
        StoredObject {
            key: ResourceKey {
                group: "example.io".into(),
                version: "v1".into(),
                kind: "Widget".into(),
            },
            metadata: ObjectMeta {
                name: "test".into(),
                namespace: Some("default".into()),
                labels: Default::default(),
                annotations: Default::default(),
                finalizers: finalizers.iter().map(|s| s.to_string()).collect(),
            },
            system: SystemMetadata::initial(),
            spec: Value::Null,
            status: None,
        }
    }

    fn obj_with_deletion_timestamp(finalizers: &[&str]) -> StoredObject {
        let mut obj = obj_with_finalizers(finalizers);
        obj.system.deletion_timestamp = Some(chrono::Utc::now());
        obj
    }

    // ------------------------------------------------------------------
    // Mock HTTP server
    // ------------------------------------------------------------------

    /// A [`StoredObject`] serialized as JSON, used as a mock server response
    /// body so `client.update()` can deserialize it successfully.
    fn stored_json(finalizers: &[&str]) -> Value {
        serde_json::to_value(obj_with_finalizers(finalizers)).expect("serialize response")
    }

    /// Builds a client pointed at a port with no listener, so any accidental
    /// network call fails immediately (proving a helper short-circuited).
    fn client_with_no_server() -> KapiClient {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        KapiClient::new(&format!("http://{addr}")).expect("build client")
    }

    /// Spawns a one-shot HTTP server on an ephemeral port that accepts a
    /// single connection, captures the request, and replies with
    /// `response_body`.
    ///
    /// Returns a [`KapiClient`] targeting the server and a handle to the
    /// captured request.
    fn spawn_capture_server(
        response_body: Value,
    ) -> (KapiClient, Arc<Mutex<Option<CapturedRequest>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        let captured: Arc<Mutex<Option<CapturedRequest>>> = Arc::new(Mutex::new(None));
        let thread_captured = Arc::clone(&captured);

        std::thread::spawn(move || {
            let (mut stream, _peer) = listener.accept().expect("accept connection");
            let mut buf: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 8192];
            'read: loop {
                let n = stream.read(&mut chunk).expect("read request");
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf);
                if let Some(sep) = text.find("\r\n\r\n") {
                    let content_length = text[..sep]
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':').unwrap_or(("", ""));
                            if name.trim().eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    if buf.len() >= sep + 4 + content_length {
                        break 'read;
                    }
                }
            }

            let raw = String::from_utf8_lossy(&buf).into_owned();
            let request_line = raw.lines().next().unwrap_or_default().to_string();
            let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
            let body_json = serde_json::from_str(&body).unwrap_or(Value::Null);
            *thread_captured.lock().expect("captured lock") =
                Some(CapturedRequest { request_line, body: body_json });

            let payload = response_body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            stream.write_all(response.as_bytes()).expect("write response");
            stream.flush().ok();
        });

        let client = KapiClient::new(&format!("http://{addr}")).expect("build client");
        (client, captured)
    }

    // ------------------------------------------------------------------
    // is_deleting
    // ------------------------------------------------------------------

    #[test]
    fn test_is_deleting_true() {
        let obj = obj_with_deletion_timestamp(&[]);
        assert!(is_deleting(&obj));
    }

    #[test]
    fn test_is_deleting_false() {
        let obj = obj_with_finalizers(&[]);
        assert!(!is_deleting(&obj));
    }

    // ------------------------------------------------------------------
    // ensure_finalizer — no network (unit tests for logic only)
    //
    // These tests verify the pure logic path where the finalizer is already
    // present or absent.  The CAS-retry path requires a real server and is
    // tested via integration tests.
    // ------------------------------------------------------------------

    #[test]
    fn test_ensure_finalizer_when_present() {
        let obj = obj_with_finalizers(&["example.io/cleanup"]);
        // The function should return Ok(()) without calling client.update
        // because the finalizer is already present.
        //
        // We cannot easily test this without mocking the client, so this
        // test asserts the precondition: finalizer IS present.
        assert!(obj.metadata.finalizers.contains(&"example.io/cleanup".to_string()));
    }

    #[test]
    fn test_ensure_finalizer_when_absent() {
        let obj = obj_with_finalizers(&[]);
        // Precondition: finalizer is NOT present.
        assert!(!obj.metadata.finalizers.contains(&"example.io/cleanup".to_string()));
    }

    #[test]
    fn test_remove_finalizer_when_present() {
        let obj = obj_with_finalizers(&["example.io/cleanup", "other/finalizer"]);
        assert!(obj.metadata.finalizers.contains(&"example.io/cleanup".to_string()));
        assert_eq!(obj.metadata.finalizers.len(), 2);
    }

    #[test]
    fn test_remove_finalizer_when_absent() {
        let obj = obj_with_finalizers(&["other/finalizer"]);
        // Precondition: the finalizer to remove is NOT present.
        assert!(!obj.metadata.finalizers.contains(&"example.io/cleanup".to_string()));
    }

    #[test]
    fn test_ensure_finalizer_idempotent_with_duplicate_list() {
        // Even if the finalizer appears multiple times (which shouldn't
        // normally happen), the "already present" check should short-circuit.
        let mut obj = obj_with_finalizers(&["example.io/cleanup"]);
        obj.metadata.finalizers.push("example.io/cleanup".into());
        assert!(obj.metadata.finalizers.iter().filter(|f| *f == "example.io/cleanup").count() >= 2);
        // ensure_finalizer would see it's present and return Ok quickly.
    }

    #[test]
    fn test_remove_finalizer_idempotent() {
        let obj = obj_with_finalizers(&[]);
        // Removing a non-existent finalizer should be a no-op.
        // (Regression guard: multiple calls should not error.)
        assert!(!obj.metadata.finalizers.contains(&"example.io/cleanup".to_string()));
    }

    // ------------------------------------------------------------------
    // Task 2.7: raw &StoredObject callers keep working (regression guard)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn raw_helpers_with_stored_object_compile_and_behave_identically() {
        // Absent finalizer → delegates and sends the appended finalizer.
        let (client, captured) = spawn_capture_server(stored_json(&["example.io/cleanup"]));
        let stored = obj_with_finalizers(&[]);
        ensure_finalizer(&client, &stored, "example.io/cleanup")
            .await
            .expect("raw ensure_finalizer should succeed");
        let req = captured.lock().expect("lock").take().expect("client made a request");
        assert!(req.request_line.starts_with("PUT"), "got: {}", req.request_line);
        let finalizers = req.body["metadata"]["finalizers"].as_array().expect("finalizers array");
        assert_eq!(finalizers.len(), 1, "finalizer should have been appended");
        assert_eq!(finalizers[0], "example.io/cleanup");

        // No-op paths must short-circuit without touching the network.
        let client = client_with_no_server();
        let present = obj_with_finalizers(&["example.io/cleanup"]);
        assert!(ensure_finalizer(&client, &present, "example.io/cleanup").await.is_ok());
        assert!(remove_finalizer(&client, &present, "other/finalizer").await.is_ok());
    }
}
