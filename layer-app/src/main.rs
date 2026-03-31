use std::io::{self, BufRead, Write};

use layer_client::{Client, Config, SignInError, update::Update};
use layer_tl_types as tl;

const API_ID:    i32  = 0;   // https://my.telegram.org
const API_HASH:  &str = "";
const PHONE:     &str = "";  // user login, leave empty if using bot
const BOT_TOKEN: &str = "";  // bot login, leave empty if using user

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "layer_client=info"); }
    }
    env_logger::init();

    if let Err(e) = run().await {
        eprintln!("\n✗ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH.is_empty() {
        eprintln!("Set API_ID and API_HASH at the top of main.rs");
        std::process::exit(1);
    }

    let (client, _shutdown) = Client::connect(Config {
        api_id:   API_ID,
        api_hash: API_HASH.to_string(),
        ..Default::default()
    }).await?;

    let is_bot = if !client.is_authorized().await? {
        let bot = login(&client).await?;
        client.save_session().await?;
        println!("💾 Session saved\n");
        bot
    } else {
        println!("✅ Already logged in\n");
        !BOT_TOKEN.is_empty()
    };

    let me = client.get_me().await?;
    if is_bot {
        println!("🤖 Running as bot: @{}", me.username.as_deref().unwrap_or("unknown"));
    } else {
        println!("👤 Running as user: {} [id={}]", display_name(&me), me.id);
    }

    ping_check(&client).await;

    println!("\n👂 Listening for updates (Ctrl+C to quit) …\n");
    listen(&client, me.id, is_bot).await;

    Ok(())
}

async fn login(client: &Client) -> Result<bool, Box<dyn std::error::Error>> {
    if !BOT_TOKEN.is_empty() {
        println!("🤖 Signing in as bot …");
        client.bot_sign_in(BOT_TOKEN).await?;
        return Ok(true);
    }

    if PHONE.is_empty() {
        eprintln!("Set PHONE (user login) or BOT_TOKEN (bot login) in the source.");
        std::process::exit(1);
    }

    println!("📱 Sending login code to {PHONE} …");
    let token = client.request_login_code(PHONE).await?;
    let code  = prompt("Enter the code: ")?;

    match client.sign_in(&token, &code).await {
        Ok(name) => println!("✅ Signed in as {name}"),
        Err(SignInError::PasswordRequired(pw_token)) => {
            let hint = pw_token.hint().unwrap_or("(no hint)");
            let pw   = prompt(&format!("2FA password (hint: {hint}): "))?;
            client.check_password(pw_token, pw.trim()).await?;
            println!("✅ 2FA complete");
        }
        Err(SignInError::SignUpRequired) => {
            eprintln!("✗ Number not registered. Sign up via the official Telegram app.");
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    }

    Ok(false)
}

async fn ping_check(client: &Client) {
    let t = std::time::Instant::now();
    match client.invoke(&tl::functions::Ping { ping_id: 0xDEAD_BEEF }).await {
        Ok(tl::enums::Pong::Pong(p)) => println!("🏓 Ping OK  rtt={}ms  msg_id={}", t.elapsed().as_millis(), p.msg_id),
        Err(e) => println!("❌ Ping failed: {e}"),
    }
}

fn is_self_chat(peer: &tl::enums::Peer, my_id: i64) -> bool {
    matches!(peer, tl::enums::Peer::User(u) if u.user_id == my_id)
}

async fn listen(client: &Client, my_id: i64, is_bot: bool) {
    let mut updates = client.stream_updates();

    while let Some(update) = updates.next().await {
        match update {
            Update::NewMessage(msg) if !msg.outgoing() => {
                let text = msg.text().unwrap_or("");
                println!("📨 [id={}] {}", msg.id(), text);

                let Some(peer) = msg.peer_id() else { continue };

                // never echo in Saved Messages — out flag is not set there
                if is_self_chat(peer, my_id) { continue; }

                match text {
                    "/ping" => {
                        let _ = client.send_message_to_peer(peer.clone(), "🏓 pong").await;
                    }
                    "/me" if !is_bot => {
                        if let Ok(me) = client.get_me().await {
                            let _ = client.send_message_to_peer(
                                peer.clone(),
                                &format!("👤 {} | id: {}", display_name(&me), me.id),
                            ).await;
                        }
                    }
                    "/read" if !is_bot => {
                        let _ = client.mark_as_read(peer.clone()).await;
                        let _ = client.send_message_to_peer(peer.clone(), "✅ Marked as read").await;
                    }
                    t if !t.is_empty() => {
                        let _ = client.send_message_to_peer(peer.clone(), &format!("Echo: {t}")).await;
                    }
                    _ => {}
                }
            }
            Update::MessageEdited(msg) => {
                println!("✏️  [id={}] edited: {}", msg.id(), msg.text().unwrap_or(""));
            }
            Update::MessageDeleted(del) => {
                println!("🗑️  deleted: {:?}", del.message_ids);
            }
            Update::CallbackQuery(cb) => {
                println!("🔘 callback [id={}]: {:?}", cb.query_id, cb.data());
                let _ = client.answer_callback_query(cb.query_id, Some("Got it!"), false).await;
            }
            Update::InlineQuery(iq) if is_bot => {
                println!("🔍 inline [id={}]: {}", iq.query_id, iq.query());
            }
            Update::Raw(raw) => {
                println!("⚙️  raw: {:#010x}", raw.constructor_id);
            }
            _ => {}
        }
    }
}

fn display_name(user: &tl::types::User) -> String {
    let first = user.first_name.as_deref().unwrap_or("");
    let last  = user.last_name.as_deref().unwrap_or("");
    format!("{} {}", first, last).trim().to_string()
}

fn prompt(msg: &str) -> io::Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
