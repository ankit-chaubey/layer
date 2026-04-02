# DC Migration

Telegram's infrastructure is split across multiple **Data Centers (DCs)**. When you connect to the wrong DC for your account, Telegram responds with a `PHONE_MIGRATE_X` or `USER_MIGRATE_X` error telling you which DC to use instead.

`layer-client` handles DC migration **automatically and transparently**. You don't need to do anything.

## How it works

1. You connect to DC2 (the default)
2. You log in with a phone number registered on DC1
3. Telegram returns `PHONE_MIGRATE_1`
4. `layer-client` reconnects to DC1, re-performs the DH handshake, and retries your request
5. Your code sees a successful response — the migration is invisible

The correct DC is then saved in the session file for future connections.

## Overriding the initial DC

By default, `layer-client` starts on DC2. If you know your account is on a different DC, you can set the initial address:

```rust
use std::net::SocketAddr;

let (client, _shutdown) = Client::connect(Config {
    dc_addr: Some("149.154.167.91:443".parse::<SocketAddr>().unwrap()),
    ..Default::default()
}).await?;
```

DC addresses:

| DC | IP |
|---|---|
| DC1 | 149.154.175.53 |
| DC2 | 149.154.167.51 |
| DC3 | 149.154.175.100 |
| DC4 | 149.154.167.91 |
| DC5 | 91.108.56.130 |

In practice, just leave `dc_addr: None` and let the auto-migration handle it.
