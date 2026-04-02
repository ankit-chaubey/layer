# Session Backends

`layer-client` ships four session backends out of the box. All implement the `SessionBackend` trait, so you can also plug in your own.

## Overview

| Backend | Feature flag | Storage | Best for |
|---|---|---|---|
| `BinaryFileBackend` | _(default)_ | Local binary file | Development, simple scripts |
| `InMemoryBackend` | _(default)_ | RAM only | Tests, ephemeral bots |
| `SqliteBackend` | `sqlite-session` | Local SQLite file | Production bots, long-running servers |
| `LibSqlBackend` | `libsql-session` | libsql / Turso (remote or embedded) | Serverless, distributed deployments |
| `StringSessionBackend` | _(default)_ | Caller-provided string | Env vars, DB columns, CI environments |

---

## BinaryFileBackend (default)

```toml
# No extra feature needed
layer-client = "0.4.4"
```

```rust
use layer_client::{Client, Config};

let (client, _shutdown) = Client::connect(Config {
    session_path: "my.session".into(),
    api_id:       12345,
    api_hash:     "abc123".into(),
    ..Default::default()
}).await?;
```

The session is stored in a compact binary format at `session_path`. Created on first login; reloaded automatically on subsequent `connect()` calls.

**Security:** treat this file like a password — add `*.session` to `.gitignore` and `chmod 600`.

---

## InMemoryBackend

```rust
use layer_client::{Client, Config};
use layer_client::session_backend::InMemoryBackend;

let (client, _shutdown) = Client::builder()
    .session(InMemoryBackend::new())
    .api_id(12345)
    .api_hash("abc123")
    .connect()
    .await?;
```

Nothing is written to disk. Login is required on every run. Ideal for integration tests and short-lived scripts.

---

## SqliteBackend

```toml
layer-client = { version = "0.4.4", features = ["sqlite-session"] }
```

```rust
use layer_client::{Client, Config};

let (client, _shutdown) = Client::connect(Config {
    session_path: "session.db".into(),  // use a .db extension
    api_id:       12345,
    api_hash:     "abc123".into(),
    ..Default::default()
}).await?;
```

SQLite is more resilient against crash-corruption than the binary format. A good choice for any bot that runs continuously or handles many accounts.

---

## LibSqlBackend — New in 0.4.4

For [libsql](https://github.com/tursodatabase/libsql) (the open-source Turso database engine):

```toml
layer-client = { version = "0.4.4", features = ["libsql-session"] }
```

```rust
use layer_client::session_backend::LibSqlBackend;
use layer_client::{Client, Config};

// Local embedded libsql file
let backend = LibSqlBackend::open_local("session.libsql").await?;

// OR: remote Turso cloud database
let backend = LibSqlBackend::open_remote(
    "libsql://your-db.turso.io",
    "your-turso-auth-token",
).await?;

let (client, _shutdown) = Client::builder()
    .session(backend)
    .api_id(12345)
    .api_hash("abc123")
    .connect()
    .await?;
```

`LibSqlBackend` is a drop-in replacement for `SqliteBackend` but works with remote databases, making it ideal for serverless or horizontally-scaled deployments.

---

## StringSessionBackend — New in 0.4.4

Encodes the entire session as a portable base64 string. Store it in environment variables, a secrets manager, a database column, or anywhere else you can store a string.

### Export an existing session

```rust
// After a successful login, export the session
let session_string = client.export_session_string().await?;
println!("{session_string}");
// → "AQAAAAEDAADtE1lMHBT7...LrKO3y8=" (example)
```

Save this string securely (e.g. in a `SESSION` environment variable).

### Restore from string

```rust
use layer_client::{Client, Config};
use layer_client::session_backend::StringSessionBackend;

let session_str = std::env::var("TG_SESSION")?;

let (client, _shutdown) = Client::builder()
    .session(StringSessionBackend::new(&session_str))
    .api_id(12345)
    .api_hash("abc123")
    .connect()
    .await?;

// If the session is valid, is_authorized() returns true — no re-login needed
assert!(client.is_authorized().await?);
```

### Convenience constructor

```rust
// Equivalent to the above — shorthand for StringSessionBackend
let session_str = std::env::var("TG_SESSION")?;
let (client, _shutdown) = Client::with_string_session(
    &session_str,
    12345,         // api_id
    "abc123",      // api_hash
).await?;
```

### Typical workflow for CI / serverless

```bash
# One-time: generate session on your dev machine
cargo run --bin login_helper
# → prints: TG_SESSION=AQAAAAEDAADtE1lMHBT7...
# Add TG_SESSION to your CI secrets
```

```rust
// In production / CI
let (client, _shutdown) = Client::with_string_session(
    &std::env::var("TG_SESSION")?,
    std::env::var("TG_API_ID")?.parse()?,
    std::env::var("TG_API_HASH")?,
).await?;
```

---

## Implementing a custom backend

```rust
use layer_client::session_backend::SessionBackend;

pub struct MyRedisBackend {
    key: String,
    // ... your redis client
}

#[async_trait::async_trait]
impl SessionBackend for MyRedisBackend {
    async fn load(&self) -> Option<Vec<u8>> {
        // read bytes from Redis
    }

    async fn save(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // write bytes to Redis
    }
}
```

Then pass it via `ClientBuilder::session()`.

---

## Comparing session backends

| | File | SQLite | LibSQL | String | Memory |
|---|:---:|:---:|:---:|:---:|:---:|
| Survives restart | ✅ | ✅ | ✅ | ✅* | ❌ |
| Crash-safe | ⚠️ | ✅ | ✅ | N/A | N/A |
| Remote storage | ❌ | ❌ | ✅ | ✅* | ❌ |
| Zero disk I/O | ❌ | ❌ | ❌ | ✅ | ✅ |
| Extra deps | None | rusqlite | libsql | None | None |

\* — provided the caller re-saves the exported string after each connect.
