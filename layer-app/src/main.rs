//! layer-app — Interactive login + update stream demo with Ping test.
//!
//! Supports both user accounts and bot tokens.
//!
//! Fill in the constants below and run:
//!   cargo run -p layer-app
//!
//! For bots, set BOT_TOKEN and leave PHONE empty.

use std::io::{self, BufRead, Write};

use layer_client::{Client, Config, SignInError, update::Update};
use layer_tl_types as tl;

// ── Fill in your credentials ──────────────────────────────────────────────────
const API_ID:    i32  = 0;                  // https://my.telegram.org
const API_HASH:  &str = "";
const PHONE:     &str = "";                 // leave empty for bot login
const BOT_TOKEN: &str = "";                 // leave empty for user login
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "layer_client=info,layer_app=info"); }
    }
    env_logger::init();

    if let Err(e) = run().await {
        eprintln!("\n✗ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH.is_empty() {
        eprintln!("Edit API_ID and API_HASH at the top of layer-app/src/main.rs");
        std::process::exit(1);
    }

    let (client, _shutdown) = Client::connect(Config {
        api_id:   API_ID,
        api_hash: API_HASH.to_string(),
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

    // ── Ping tests ─────────────────────────────────────────────────────
    println!("\n🧪 Test 1 — ping_id = 0x0");
    test_ping(&client, 0).await;

    println!("\n🧪 Test 2 — ping_id = 0xdeadbeef12345678");
    test_ping(&client, 0xDEAD_BEEF_1234_5678_u64 as i64).await;

    println!("\n🧪 Test 3 — cloned client");
    let client2 = client.clone();
    match client2.invoke(&tl::functions::Ping { ping_id: 42 }).await {
        Ok(tl::enums::Pong::Pong(p)) => println!("  ✅ ping_id={}  msg_id={}", p.ping_id, p.msg_id),
        Err(e) => println!("  ❌ {e}"),
    }

    println!("\n🧪 Test 4 — rapid-fire 3 pings");
    for i in 1i64..=3 {
        let t = std::time::Instant::now();
        match client.invoke(&tl::functions::Ping { ping_id: i }).await {
            Ok(tl::enums::Pong::Pong(p)) => println!("  ✅ #{i}  rtt={}ms  ping_id={}  msg_id={}", t.elapsed().as_millis(), p.ping_id, p.msg_id),
            Err(e) => println!("  ❌ #{i} {e}"),
        }
    }

    // ── Send a test message ────────────────────────────────────────────
    client.send_to_self("Hello from layer! 👋").await?;
    println!("\n💬 Sent test message to Saved Messages");

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
                println!("🔘 Callback query [id={}]: {:?}", cb.query_id, cb.data());
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

async fn test_ping(client: &Client, ping_id: i64) {
    match client.invoke(&tl::functions::Ping { ping_id }).await {
        Ok(tl::enums::Pong::Pong(p)) if p.ping_id == ping_id => {
            println!("  ✅ ping_id={:#x}  msg_id={}", p.ping_id, p.msg_id);
        }
        Ok(tl::enums::Pong::Pong(p)) => {
            println!("  ⚠️  mismatch — sent {ping_id:#x}, got {:#x}", p.ping_id);
        }
        Err(e) => println!("  ❌ {e}"),
    }
}

fn prompt(msg: &str) -> io::Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
