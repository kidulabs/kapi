//! Swagger UI serving for the kapi OpenAPI spec.
//!
//! Provides a static HTML page that loads Swagger UI from CDN and
//! configures it to fetch the spec from `/openapi`.

use axum::response::Html;

/// Swagger UI HTML page loaded from CDN, configured to fetch the spec from `/openapi`.
const SWAGGER_UI_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>kapi API — Swagger UI</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    SwaggerUIBundle({ url: "/openapi", dom_id: "#swagger-ui" });
  </script>
</body>
</html>"##;

/// Handler for `GET /swagger-ui/`.
///
/// Serves a minimal HTML page that loads Swagger UI from CDN and points to `/openapi`.
pub async fn get_swagger_ui_handler() -> Html<&'static str> {
    Html(SWAGGER_UI_HTML)
}
