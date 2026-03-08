# User Login

User login happens in three steps: request code → submit code → (optional) submit 2FA password.

## Step 1 — Request login code

```rust
let token = client.request_login_code("+1234567890").await?;
```

This sends a verification code to the phone number via SMS or Telegram app notification. The returned `LoginToken` must be passed to the next step.

## Step 2 — Submit the code

```rust
match client.sign_in(&token, "12345").await {
    Ok(name) => {
        println!("Signed in as {name}");
    }
    Err(SignInError::PasswordRequired(password_token)) => {
        // 2FA is enabled — go to step 3
    }
    Err(e) => return Err(e.into()),
}
```

`sign_in` returns:
- `Ok(String)` — the user's full name, login complete
- `Err(SignInError::PasswordRequired(PasswordToken))` — 2FA is enabled, need password
- `Err(e)` — wrong code, expired code, or network error

## Step 3 — 2FA password (if required)

```rust
client.check_password(password_token, "my_2fa_password").await?;
```

This performs the full SRP (Secure Remote Password) exchange. The password is never sent to Telegram in plain text — only a cryptographic proof is transmitted.

## Save the session

After a successful login, always save the session so you don't need to log in again:

```rust
client.save_session().await?;
```

## Full example with stdin

```rust
use layer_client::{Client, Config, SignInError};
use std::io::{self, BufRead, Write};

async fn login(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    if client.is_authorized().await? {
        return Ok(());
    }

    print!("Phone number: ");
    io::stdout().flush()?;
    let phone = read_line();

    let token = client.request_login_code(&phone).await?;

    print!("Code: ");
    io::stdout().flush()?;
    let code = read_line();

    match client.sign_in(&token, &code).await {
        Ok(name) => println!("✅ Welcome, {name}!"),
        Err(SignInError::PasswordRequired(t)) => {
            print!("2FA password: ");
            io::stdout().flush()?;
            let pw = read_line();
            client.check_password(t, &pw).await?;
            println!("✅ 2FA verified");
        }
        Err(e) => return Err(e.into()),
    }

    client.save_session().await?;
    Ok(())
}

fn read_line() -> String {
    let stdin = io::stdin();
    stdin.lock().lines().next().unwrap().unwrap().trim().to_string()
}
```

## Sign out

```rust
client.sign_out().await?;
```

This revokes the auth key on Telegram's servers and deletes the local session file.
