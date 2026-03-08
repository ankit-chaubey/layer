# Session Persistence

A session stores your auth key, DC address, and peer access hash cache. Without it, you'd need to log in on every run.

## Binary file (default)

```rust
Config {
    session_path: "my.session".into(),
    ..Default::default()
}
```

After login:

```rust
client.save_session().await?;
```

The file is created at the given path and loaded automatically on the next `Client::connect`. Keep it in `.gitignore` — it's equivalent to your account password.

## In-memory (ephemeral)

Nothing written to disk. Useful for tests or short-lived scripts:

```rust
use layer_client::session_backend::InMemoryBackend;

// Using InMemoryBackend directly via Config
Config {
    // session_path is ignored when using a custom backend
    ..Default::default()
}
```

With an in-memory session, login is required on every run.

## SQLite (robust, long-running servers)

Enable the feature flag:

```toml
layer-client = { version = "0.2.2", features = ["sqlite-session"] }
```

```rust
// SQLite session is automatically used when the feature is enabled
// and the session file has a .db extension
Config {
    session_path: "session.db".into(),
    ..Default::default()
}
```

SQLite is more robust against crash-corruption than the binary file format, making it ideal for production bots.

## What's stored in a session

| Field | Description |
|---|---|
| Auth key | 2048-bit DH-derived key for encryption |
| Auth key ID | Hash of the key, used as identifier |
| DC ID | Which Telegram data center to connect to |
| DC address | The IP:port of the DC |
| Server salt | Updated regularly by Telegram |
| Sequence numbers | For message ordering |
| Peer cache | User/channel access hashes (speeds up API calls) |

## Security

> **SECURITY:** A stolen session file gives full API access to your account. Protect it like a password.

- Add to `.gitignore`: `*.session`, `*.session.db`
- Set restrictive permissions: `chmod 600 my.session`
- Never log or print session file contents
- If compromised: revoke from **Telegram → Settings → Devices → Terminate session**

## Multi-session / multi-account

Each `Client::connect` call loads one session. For multiple accounts, use multiple files:

```rust
let client_a = Client::connect(Config {
    session_path: "account_a.session".into(),
    api_id, api_hash: api_hash.clone(), ..Default::default()
}).await?;

let client_b = Client::connect(Config {
    session_path: "account_b.session".into(),
    api_id, api_hash: api_hash.clone(), ..Default::default()
}).await?;
```
