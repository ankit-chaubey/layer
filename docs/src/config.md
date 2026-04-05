# Configuration

`Config` is the single struct passed to `Client::connect`. All fields except `api_id` and `api_hash` have defaults.

```rust
use layer_client::{Config, AutoSleep, TransportKind, Socks5Config};
use layer_client::session_backend::{BinaryFileBackend, InMemoryBackend};
use std::sync::Arc;

let (client, _shutdown) = Client::connect(Config {
    // Required
    api_id:   12345,
    api_hash: "your_api_hash".into(),

    // Session (default: BinaryFileBackend("session.session"))
    

    // DC override (default: DC2)
    dc_addr: None,

    // Transport (default: Abridged)
    transport: TransportKind::Abridged,

    // Flood wait retry (default: AutoSleep)
    retry_policy: Arc::new(AutoSleep::default()),

    // Proxy (default: None)
    socks5: None,

    ..Default::default()
}).await?;
```

---

## All fields

### `api_id` — required
Your Telegram app's numeric ID from [my.telegram.org](https://my.telegram.org).

```rust
api_id: 12345_i32,
```

### `api_hash` — required
Your Telegram app's hex hash string from [my.telegram.org](https://my.telegram.org).

```rust
api_hash: "deadbeef01234567...".into(),
```

### `session_backend`
Path to the binary session file. Default: `"session.session"`.

```rust

```

### `dc_addr`
Override the initial DC address. Default: `None` (uses DC2 = `149.154.167.51:443`). After login, the correct DC is cached in the session.

```rust
dc_addr: Some("149.154.175.53:443".parse().unwrap()), // DC1
```

### `transport`
The MTProto transport protocol. Default: `TransportKind::Abridged`.

| Variant | Description |
|---|---|
| `Abridged` | Minimal overhead, default |
| `Intermediate` | Fixed-length framing |
| `ObfuscatedAbridged` | Disguised for firewall evasion |

```rust
transport: TransportKind::ObfuscatedAbridged,
```

### `retry_policy`
How to handle `FLOOD_WAIT` errors. Default: `AutoSleep`.

```rust
use layer_client::{AutoSleep, NoRetries};

retry_policy: Arc::new(AutoSleep::default()),  // auto-sleep and retry
retry_policy: Arc::new(NoRetries),             // propagate immediately
```

### `socks5`
Optional SOCKS5 proxy configuration.

```rust
socks5: Some(Socks5Config {
    addr:     "127.0.0.1:1080".parse().unwrap(),
    username: None,
    password: None,
}),
```

---

## Full default values

```rust
Config {
    api_id:        0,
    api_hash:      String::new(),
    
    dc_addr:       None,
    transport:     TransportKind::Abridged,
    retry_policy:  Arc::new(AutoSleep::default()),
    socks5:        None,
}
```
