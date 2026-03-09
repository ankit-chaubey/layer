//! layer-app — Interactive login + update stream demo.
//!
//! Supports both user accounts and bot tokens.
//!
//! Fill in the constants below and run:
//!   cargo run -p layer-app
//!
//! For bots, set BOT_TOKEN and leave PHONE empty.

use std::io::{self, BufRead, Write};

use layer_client::{Client, Config, SignInError, update::Update};

// ── Fill in your credentials ──────────────────────────────────────────────────
const API_ID:    i32  = 0;                  // https://my.telegram.org
const API_HASH:  &str = "";
const PHONE:     &str = "";                 // leave empty for bot login
const BOT_TOKEN: &str = "";                 // leave empty for user login
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Enable logging: RUST_LOG=layer_client=info,layer_app=info cargo run
    if std::env::var("RUST_LOG").is_err() {
        // SAFETY: single-threaded at this point, no other threads reading env
        unsafe { std::env::set_var("RUST_LOG", "layer_client=info,layer_app=info"); }
    env_logger::init();
    }

    if let Err(e) = run().await {
        eprintln!("\n✗ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH == "YOUR_API_HASH" {
        eprintln!("Edit API_ID and API_HASH at the top of layer-app/src/main.rs");
        std::process::exit(1);
    }

    let (client, _shutdown) = Client::connect(Config {
        api_id:       API_ID,
        api_hash:     API_HASH.to_string(),
        ..Default::default()
    }).await?;

    if !client.is_authorized().await? {
        if !BOT_TOKEN.is_empty() {
            // ── Bot login ──────────────────────────────────────────────
            println!("🤖 Signing in as bot …");
            client.bot_sign_in(BOT_TOKEN).await?;
        } else if !PHONE.is_empty() {
            // ── User login ─────────────────────────────────────────────
            println!("📱 Sending login code to {} …", PHONE);
            let token = client.request_login_code(PHONE).await?;
            let code  = prompt("Enter the code you received: ")?;

            match client.sign_in(&token, &code).await {
                Ok(name) => println!("✅ Signed in as {name}"),
                Err(SignInError::PasswordRequired(pw_token)) => {
                    let hint = pw_token.hint().unwrap_or("(no hint)");
                    let pw   = prompt(&format!("2FA password (hint: {hint}): "))?;
                    client.check_password(pw_token, pw.trim()).await?;
                    println!("✅ 2FA complete");
                }
                Err(SignInError::SignUpRequired) => {
                    eprintln!("✗ This number is not registered. Sign up via the official Telegram app first.");
                    std::process::exit(1);
                }
                Err(e) => return Err(e.into()),
            }
        } else {
            eprintln!("Set PHONE (for user login) or BOT_TOKEN (for bot login) in the source.");
            std::process::exit(1);
        }

        client.save_session().await?;
        println!("💾 Session saved");
    } else {
        println!("✅ Already logged in");
    }

    // ── Send a test message ────────────────────────────────────────────
    client.send_to_self("Hello from layer! 👋").await?;
    println!("💬 Sent test message to Saved Messages");

    // ── Update stream loop ─────────────────────────────────────────────
    println!("\n👂 Listening for updates (Ctrl+C to quit) …\n");
    let mut updates = client.stream_updates();

    while let Some(update) = updates.next().await {
        match update {
            Update::NewMessage(msg) => {
                if !msg.outgoing() {
                    println!(
                        "📨 New message [id={}]: {}",
                        msg.id(),
                        msg.text().unwrap_or("")
                    );
                    // Echo back
                    if let Some(peer) = msg.peer_id() {
                        let _ = client.send_message_to_peer(
                            peer.clone(),
                            &format!("Echo: {}", msg.text().unwrap_or("")),
                        ).await;
                    }
                }
            }
            Update::MessageEdited(msg) => {
                println!("✏️  Message edited [id={}]: {}", msg.id(), msg.text().unwrap_or(""));
            }
            Update::MessageDeleted(del) => {
                println!("🗑️  Messages deleted: {:?}", del.message_ids);
            }
            Update::CallbackQuery(cb) => {
                println!(
                    "🔘 Callback query [id={}]: {:?}",
                    cb.query_id,
                    cb.data()
                );
                // Answer the callback
                let _ = client.answer_callback_query(cb.query_id, Some("Got it!"), false).await;
            }
            Update::InlineQuery(iq) => {
                println!("🔍 Inline query [id={}]: {}", iq.query_id, iq.query());
            }
            Update::Raw(raw) => {
                println!("⚙️  Raw update: constructor_id={:#010x}", raw.constructor_id);
            }
            _ => {}
        }
    }

    Ok(())
}

fn prompt(msg: &str) -> io::Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
