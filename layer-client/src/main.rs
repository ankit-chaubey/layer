//! Telegram client — mirrors the grammers dialogs.rs example flow exactly.
//!
//! Fill in the three constants below, then:
//!   cargo run -p layer-client

use layer_client_core::{Client, SignInError};

// ── Fill these in ──────────────────────────────────────────────────────────────
const API_ID:   i32  = 0;               // https://my.telegram.org
const API_HASH: &str = "YOUR_API_HASH";
const PHONE:    &str = "+1234567890";
// ──────────────────────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = run() {
        eprintln!("\n✗ {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH == "YOUR_API_HASH" || PHONE == "+1234567890" {
        eprintln!("Edit API_ID, API_HASH, and PHONE at the top of src/main.rs");
        std::process::exit(1);
    }

    // ── Connect (reuses session.bin if it exists) ─────────────────────────────
    let mut client = Client::load_or_connect("session.bin", API_ID, API_HASH)?;

    // ── Auth ──────────────────────────────────────────────────────────────────
    if !client.is_authorized()? {
        println!("Signing in…");

        let token = client.request_login_code(PHONE)?;
        let code  = prompt("Enter the code Telegram sent you: ")?;

        match client.sign_in(&token, &code) {
            Ok(name) => {
                println!("Signed in as {name}!");
            }

            // 2FA — exactly like the grammers dialogs.rs example
            Err(SignInError::PasswordRequired(password_token)) => {
                let hint = password_token.hint().unwrap_or("no hint");
                let pw   = prompt(&format!("Enter your 2FA password (hint: {hint}): "))?;
                client.check_password(password_token, pw.trim())?;
                println!("Signed in via 2FA!");
            }

            Err(SignInError::InvalidCode)   => return Err("Invalid code — try again".into()),
            Err(SignInError::SignUpRequired) => return Err("Number not registered. Sign up via official app first.".into()),
            Err(SignInError::Other(e))      => return Err(e.into()),
        }

        client.save_session("session.bin")?;
        println!("Session saved.");
    }

    // ── Send a message to Saved Messages ──────────────────────────────────────
    client.send_message("me", "😁")?;
    println!("\n✓ Done — check Saved Messages in Telegram.");
    Ok(())
}

fn prompt(msg: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::{BufRead, Write};
    print!("{msg}");
    std::io::stdout().flush()?;
    let line = std::io::stdin().lock().lines().next()
        .ok_or("stdin closed")??;
    Ok(line)
}
