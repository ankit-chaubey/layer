# Error Types

## InvocationError

All `Client` async methods return `Result<T, InvocationError>`:

```rust
pub enum InvocationError {
    Rpc(RpcError),        // Telegram returned an error response
    Deserialize(String),  // failed to decode the server's binary response
    Io(std::io::Error),   // network or IO failure
}
```

## RpcError

```rust
pub struct RpcError {
    pub code:    i32,
    pub message: String,
}
```

### Error code groups

| Code | Category | Meaning |
|---|---|---|
| `303` | See Other | DC migration — handled automatically by layer |
| `400` | Bad Request | Wrong parameters, invalid data |
| `401` | Unauthorized | Not logged in, session invalid/expired |
| `403` | Forbidden | Insufficient permissions |
| `404` | Not Found | Resource doesn't exist |
| `406` | Not Acceptable | Content not acceptable |
| `420` | Flood | `FLOOD_WAIT_X` — rate limited |
| `500` | Server Error | Telegram internal error, retry later |

### Common error messages

| Message | Cause | Fix |
|---|---|---|
| `PHONE_NUMBER_INVALID` | Bad phone format | Use E.164 format: `+12345678900` |
| `PHONE_CODE_INVALID` | Wrong code | Ask user to try again |
| `PHONE_CODE_EXPIRED` | Code timed out | Call `request_login_code` again |
| `SESSION_PASSWORD_NEEDED` | 2FA required | Use `check_password` |
| `PASSWORD_HASH_INVALID` | Wrong 2FA password | Re-prompt the user |
| `PEER_ID_INVALID` | Unknown peer | Resolve peer first or check the ID |
| `ACCESS_TOKEN_INVALID` | Bad bot token | Check token from @BotFather |
| `CHAT_WRITE_FORBIDDEN` | Can't post here | Bot not in group or read-only channel |
| `USER_PRIVACY_RESTRICTED` | Privacy blocks action | Can't message/add this user |
| `FLOOD_WAIT_N` | Rate limited | Wait N seconds (AutoSleep handles this) |
| `FILE_PARTS_INVALID` | Upload error | Retry the upload |
| `MEDIA_EMPTY` | No media provided | Check your InputMedia |
| `MESSAGE_NOT_MODIFIED` | Edit with no changes | Ensure new text differs |
| `BOT_INLINE_DISABLED` | Inline mode off | Enable in @BotFather |
| `QUERY_ID_INVALID` | Callback too old | Answer within 60 seconds |

---

## SignInError

Returned specifically by `client.sign_in()`:

```rust
pub enum SignInError {
    PasswordRequired(PasswordToken), // 2FA is on — pass to check_password()
    InvalidCode,                     // wrong code submitted
    Other(InvocationError),          // anything else
}
```

---

## Full error handling example

```rust
use layer_client::{InvocationError, RpcError, SignInError};
use std::time::Duration;

// Login errors
match client.sign_in(&token, &code).await {
    Ok(name)                                       => println!("✅ {name}"),
    Err(SignInError::PasswordRequired(pw))         => handle_2fa(pw).await?,
    Err(SignInError::InvalidCode)                  => println!("❌ Wrong code"),
    Err(SignInError::Other(InvocationError::Rpc(e))) => println!("RPC {}: {}", e.code, e.message),
    Err(SignInError::Other(e))                     => println!("IO/decode error: {e}"),
}

// General method errors
match client.send_message("@user", "hi").await {
    Ok(_) => {}

    // Rate limit (only visible if using NoRetries policy)
    Err(InvocationError::Rpc(RpcError { code: 420, ref message, .. })) => {
        let secs: u64 = message
            .strip_prefix("FLOOD_WAIT_")
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        println!("Rate limited. Sleeping {secs}s");
        tokio::time::sleep(Duration::from_secs(secs)).await;
    }

    // Permission error
    Err(InvocationError::Rpc(RpcError { code: 403, ref message, .. })) => {
        println!("Permission denied: {message}");
    }

    // Network error
    Err(InvocationError::Io(e)) => {
        println!("Network error: {e}");
        // Consider reconnecting
    }

    Err(e) => eprintln!("Unexpected: {e}"),
}
```

---

## Implementing From for your error type

```rust
#[derive(Debug)]
enum MyError {
    Telegram(layer_client::InvocationError),
    Io(std::io::Error),
    Custom(String),
}

impl From<layer_client::InvocationError> for MyError {
    fn from(e: layer_client::InvocationError) -> Self {
        MyError::Telegram(e)
    }
}

// Now you can use ? throughout your handlers
async fn my_handler(client: &Client) -> Result<(), MyError> {
    client.send_message("me", "hello").await?;  // auto-converts
    Ok(())
}
```
