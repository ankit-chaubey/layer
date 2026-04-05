# Socks5 Proxy

`layer-client` supports SOCKS5 proxies, including those with username/password authentication.

## Configuration

```rust
use layer_client::{Client, Config, Socks5Config};

let (client, _shutdown) = Client::connect(Config {
    api_id:       12345,
    api_hash:     "your_hash".into(),
    socks5:       Some(Socks5Config {
        addr:     "127.0.0.1:1080".parse().unwrap(),
        username: None,
        password: None,
    }),
    ..Default::default()
}).await?;
```

## With authentication

```rust
socks5: Some(Socks5Config {
    addr:     "proxy.example.com:1080".parse().unwrap(),
    username: Some("user".into()),
    password: Some("pass".into()),
}),
```

## Common use cases

**MTProxy** is a Telegram-specific proxy format. `layer-client` uses standard SOCKS5. To use an MTProxy, you'll need a SOCKS5 bridge or use the `transport_obfuscated` module for protocol obfuscation.

**Tor**: Point SOCKS5 at `127.0.0.1:9050` (the default Tor port) to route all Telegram traffic through the Tor network.

```rust
socks5: Some(Socks5Config {
    addr:     "127.0.0.1:9050".parse().unwrap(),
    username: None,
    password: None,
}),
```

> **NOTE:** When using Tor, Telegram connections may be slower and some DCs may block Tor exit nodes. Consider using Telegram's `.onion` address if available.

## Obfuscated transport

For networks that block Telegram, layer also supports the obfuscated transport:

```rust
use layer_client::TransportKind;

Config {
    transport: TransportKind::ObfuscatedAbridged,
    ..Default::default()
}
```

This disguises MTProto traffic to look like random bytes, making it harder for firewalls to detect and block.
