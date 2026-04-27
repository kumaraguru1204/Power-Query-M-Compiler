# Interface — HTTP API

## Scope
The web-server entry point. Hosts an HTTP service on a fixed local address (port 8080 by default) and exposes three routes plus a static file server.

## Routes

### GET /health

**Purpose.** Liveness probe.

**Request.** No body. No query parameters.

**Response.** 200 OK. JSON body with two fields:
- a status field, set to the text *ok*;
- a version label.

The response is intended to confirm the server is up; it does not exercise the engine.

### POST /api/compile

**Purpose.** The primary engine-invocation endpoint.

**Request.** A JSON body with three fields:
- *formula* — the M source string (required).
- *json* — the workbook payload string (optional; the shared compile function substitutes a default if absent, per R-INV-12).
- *debug* — a Boolean (optional; defaults to false).

**Response on success.** 200 OK. JSON body with up to nine fields:
- *success* — true.
- *result* — the rendered result table preview text.
- *errors* — an empty list.
- *warnings* — a list of warning strings (W001, W002, etc.).
- *formatted* — the cleaned formula text.
- *sql* — the generated SQL.
- *tokens* — present only when *debug* is true; the token list as a human-readable string.
- *ast* — present only when *debug* is true; a debug-formatted dump of the parsed program.

**Response on engine failure.** 200 OK (HTTP-level success; the failure is in the body). JSON body with:
- *success* — false.
- *errors* — a list of detailed error records, each with:
  - *message*,
  - *line* (one-based),
  - *column* (one-based),
  - *length*,
  - *code* (the diagnostic code from the catalogue in `cross_cutting/13_error_model.md`),
  - *source_line* (the source line text containing the error).
- All other success-mode fields absent or empty.

**Response on transport failure.** 4xx or 5xx as appropriate (malformed JSON request body, internal server panic). The body is a plain-text error message.

### POST /api/compile/debug

**Purpose.** Same as POST /api/compile but always sets *debug* to true regardless of the request body.

**Request.** Same shape as POST /api/compile. The *debug* field, if present, is ignored.

**Response.** Same as POST /api/compile in success mode, with *tokens* and *ast* always populated.

### GET /

**Purpose.** Serve the playground HTML page from the static-assets folder (the public folder).

**Response.** 200 OK with the page's HTML.

### GET /<asset path>

**Purpose.** Serve any asset under the static-assets folder (CSS, JavaScript, images).

**Response.** 200 OK with the asset content and the appropriate Content-Type header.

## Cross-cutting

- **CORS.** Permissive by default for development; locked down in production deployments. Configurable via environment variable.
- **Logging.** Every request is logged at info level with method, path, status, and duration.
- **Concurrency.** The server uses an async runtime; each request is handled on a worker thread. The engine itself is single-threaded per request.

## Conformance
Pointers to fixtures will live under `conformance/interfaces/http/`.

