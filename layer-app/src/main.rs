//! layer-app — Userbot for USER accounts only.
//!
//! # Bugs fixed
//! 1. Own `.ping` never triggered  → `out=true` messages were dropped entirely;
//!    now dot-commands from self are processed.
//! 2. Non-contact DM replies failed → `UpdateShortMessage` carries no User object,
//!    so access_hash was unknown; we now fetch it via `InputUserFromMessage` before
//!    sending any reply.
//! 3. Group non-contacts same root → same fix covers group senders too.
//!
//! # Setup
//! Fill in API_ID, API_HASH and PHONE at the top, then:
//!   cargo run -p layer-app
//!
//! # Commands (dot-prefix, for user accounts)
//! .ping   .me     .id      .msgid   .dc       .layer
//! .read   .del    .pin     .unpin   .typing   .dialogs
//! .whois  .echo   .upper   .lower   .rev      .calc
//! .edit   .fwd    .help

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use layer_client::{
    Client, Config, InputMessage, SignInError, parsers::parse_markdown, update::Update,
};
use layer_tl_types::{self as tl, Cursor, Deserializable};

// ── Credentials ───────────────────────────────────────────────────────────────
const API_ID: i32 = 0; // https://my.telegram.org
const API_HASH: &str = "";
const PHONE: &str = ""; // your phone number  e.g. "+919876543210"
// Leave BOT_TOKEN empty for user login; set it to run as a bot instead
const BOT_TOKEN: &str = "";
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "layer_client=warn,layer_app=info");
        }
    }
    env_logger::init();
    if let Err(e) = run().await {
        eprintln!("\n✗ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH.is_empty() {
        eprintln!("Set API_ID and API_HASH at the top of src/main.rs");
        std::process::exit(1);
    }

    println!("🔌 Connecting…");
    let (client, _shutdown) = Client::connect(Config {
        api_id: API_ID,
        api_hash: API_HASH.to_string(),
        ..Default::default()
    })
    .await?;

    if !client.is_authorized().await? {
        do_login(&client).await?;
        client.save_session().await?;
        println!("💾 Session saved");
    } else {
        println!("✅ Already logged in");
    }

    let me = client.get_me().await?;
    println!(
        "👤 Userbot: {} (id={}){}",
        full_name(&me),
        me.id,
        me.username
            .as_deref()
            .map(|u| format!(" @{u}"))
            .unwrap_or_default()
    );

    // Startup ping to verify connection
    let t = Instant::now();
    match client
        .invoke(&tl::functions::Ping {
            ping_id: 0xDEAD_BEEF,
        })
        .await
    {
        Ok(tl::enums::Pong::Pong(p)) => println!(
            "🏓 Ping OK  rtt={}ms  msg_id={}",
            t.elapsed().as_millis(),
            p.msg_id
        ),
        Err(e) => println!("⚠️  Ping: {e}"),
    }

    println!("\n👂 Listening for updates… (Ctrl+C to quit)\n");

    let client = Arc::new(client);
    let my_id = me.id;
    let mut stream = client.stream_updates();

    while let Some(upd) = stream.next().await {
        let c = client.clone();
        tokio::spawn(async move { dispatch(upd, &c, my_id).await });
    }

    Ok(())
}

// ─── Dispatcher ───────────────────────────────────────────────────────────────

async fn dispatch(upd: Update, client: &Client, my_id: i64) {
    match upd {
        Update::NewMessage(msg) => {
            let text = msg.text().unwrap_or("").trim().to_string();
            let out = msg.outgoing();
            let msg_id = msg.id();

            // BUG-FIX #1: userbot handles its OWN dot-commands (out=true).
            // Skip outgoing messages that are NOT dot-commands (avoid echo loops).
            if out && !text.starts_with('.') {
                return;
            }

            let Some(peer) = msg.peer_id() else { return };

            // Skip "Saved Messages" for plain outgoing non-commands
            if is_self_peer(peer, my_id) && !text.starts_with('.') {
                return;
            }

            let sender_uid = user_id_from_peer(msg.sender_id());
            let peer = peer.clone();

            println!(
                "{}  [msg={}{}] {}",
                if out { "📤" } else { "📨" },
                msg_id,
                sender_uid
                    .map(|id| format!(" from={id}"))
                    .unwrap_or_default(),
                &text[..text.len().min(120)]
            );

            if !text.starts_with('.') {
                return;
            }

            // BUG-FIX #2 & #3: ensure sender's access_hash is cached
            // so we can reply to non-contacts (UpdateShortMessage strips User objects).
            if let Some(uid) = sender_uid {
                if uid != my_id {
                    cache_sender(client, uid, msg_id, &peer).await;
                }
            }

            let (cmd, arg) = parse_dot_cmd(&text);
            route(
                client,
                cmd.as_str(),
                arg.as_str(),
                peer,
                msg_id,
                my_id,
                sender_uid,
            )
            .await;
        }

        Update::MessageEdited(msg) => println!(
            "✏️  [msg={}] edited: {}",
            msg.id(),
            msg.text().unwrap_or("")
        ),

        Update::MessageDeleted(del) => println!("🗑️  deleted msg IDs: {:?}", del.message_ids),

        Update::Raw(raw) => println!("⚙️  raw constructor: {:#010x}", raw.constructor_id),

        _ => {}
    }
}

// ─── Command router ───────────────────────────────────────────────────────────

async fn route(
    client: &Client,
    cmd: &str,
    arg: &str,
    peer: tl::enums::Peer,
    msg_id: i32,
    my_id: i64,
    sender_uid: Option<i64>,
) {
    match cmd {
        // ── Info ───────────────────────────────────────────────────────────
        ".ping" => cmd_ping(client, peer, msg_id).await,
        ".me" => cmd_me(client, peer, msg_id).await,
        ".id" => cmd_id(client, peer.clone(), msg_id, sender_uid, &peer).await,
        ".msgid" => cmd_msgid(client, peer, msg_id).await,
        ".dc" => cmd_dc(client, peer, msg_id).await,
        ".layer" => cmd_layer(client, peer, msg_id).await,
        ".whois" => cmd_whois(client, peer.clone(), msg_id, sender_uid, &peer).await,
        ".time" => cmd_time(client, peer, msg_id).await,

        // ── Chat actions ───────────────────────────────────────────────────
        ".read" => {
            let _ = client.mark_as_read(peer).await;
        }
        ".del" => cmd_del(client, msg_id).await,
        ".pin" => {
            let _ = client.pin_message(peer, msg_id, true, false, false).await;
        }
        ".unpin" => {
            let _ = client.unpin_message(peer, msg_id).await;
        }
        ".typing" => {
            let _ = client
                .send_chat_action(peer, tl::enums::SendMessageAction::Typing)
                .await;
        }

        // ── Lists ──────────────────────────────────────────────────────────
        ".dialogs" => cmd_dialogs(client, peer, msg_id).await,

        // ── Text tools ────────────────────────────────────────────────────
        ".echo" => send_reply(client, peer, msg_id, arg).await,
        ".upper" => send_reply(client, peer, msg_id, &arg.to_uppercase()).await,
        ".lower" => send_reply(client, peer, msg_id, &arg.to_lowercase()).await,
        ".rev" => {
            let r: String = arg.chars().rev().collect();
            send_reply(client, peer, msg_id, &r).await;
        }
        ".calc" => {
            let result = eval_expr(arg.trim());
            let out = match result {
                Ok(v) => format!("🧮 `{arg}` = **{v}**"),
                Err(e) => format!("❌ {e}"),
            };
            send_reply_md(client, peer, msg_id, &out).await;
        }
        ".count" => {
            let t = format!(
                "📊 **Stats**\n\n**Chars:** `{}`\n**Bytes:** `{}`\n**Words:** `{}`\n**Lines:** `{}`",
                arg.chars().count(),
                arg.len(),
                arg.split_whitespace().count(),
                arg.lines().count()
            );
            send_reply_md(client, peer, msg_id, &t).await;
        }

        // ── Edit / forward ─────────────────────────────────────────────────
        ".edit" if !arg.is_empty() => {
            let _ = client.edit_message(peer, msg_id, arg).await;
        }
        ".fwd" if !arg.is_empty() => {
            // Forward current message to a username / peer
            let _ = client.forward_messages(arg, &[msg_id], peer).await;
        }

        // ── Help ───────────────────────────────────────────────────────────
        ".help" => cmd_help(client, peer, msg_id).await,

        _ => {} // unknown command — silently ignore
    }
}

// ─── Bug-fix: cache sender's access_hash ─────────────────────────────────────

/// Fetch the sender via `users.getUsers + InputUserFromMessage` so their
/// access_hash ends up in the peer cache.  Without this, replies to
/// non-contacts silently fail (access_hash = 0 → USER_ID_INVALID).
async fn cache_sender(client: &Client, user_id: i64, msg_id: i32, chat_peer: &tl::enums::Peer) {
    let ctx_peer = match chat_peer {
        // Regular group: no access_hash needed for InputPeerChat
        tl::enums::Peer::Chat(c) => {
            tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: c.chat_id })
        }
        // For DMs or channels, use Empty and let server use msg context
        _ => tl::enums::InputPeer::Empty,
    };

    let req = tl::functions::users::GetUsers {
        id: vec![tl::enums::InputUser::FromMessage(
            tl::types::InputUserFromMessage {
                peer: ctx_peer,
                msg_id,
                user_id,
            },
        )],
    };

    if let Ok(body) = client.rpc_call_raw_pub(&req).await {
        let mut cur = Cursor::from_slice(&body);
        if let Ok(users) = Vec::<tl::enums::User>::deserialize(&mut cur) {
            client.cache_users_slice_pub(&users).await;
        }
    }
}

// ─── Command implementations ──────────────────────────────────────────────────

async fn cmd_ping(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let t = Instant::now();
    let ok = client
        .invoke(&tl::functions::Ping {
            ping_id: 0xDEAD_BEEF,
        })
        .await
        .is_ok();
    let rtt = t.elapsed().as_millis();
    let msg = if ok {
        format!("🏓 pong | **{rtt}ms**")
    } else {
        "🏓 pong | timeout".into()
    };
    send_reply_md(client, peer, reply_to, &msg).await;
}

async fn cmd_me(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    match client.get_me().await {
        Ok(me) => {
            let uname = me
                .username
                .as_deref()
                .map(|u| format!("@{u}"))
                .unwrap_or_else(|| "_(none)_".into());
            let text = format!(
                "👤 **Me**\n\n\
                **Name:** {}\n\
                **Username:** {}\n\
                **ID:** `{}`\n\
                **Phone:** `{}`\n\
                **Premium:** {}\n\
                **Bot:** {}",
                full_name(&me),
                uname,
                me.id,
                me.phone.as_deref().unwrap_or("hidden"),
                if me.premium { "✅" } else { "❌" },
                if me.bot { "✅" } else { "❌" },
            );
            send_reply_md(client, peer, reply_to, &text).await;
        }
        Err(e) => send_reply(client, peer, reply_to, &format!("❌ {e}")).await,
    }
}

async fn cmd_id(
    client: &Client,
    peer: tl::enums::Peer,
    reply_to: i32,
    sender_uid: Option<i64>,
    chat_peer: &tl::enums::Peer,
) {
    let sender_str = match sender_uid {
        Some(id) => format!("`{id}`"),
        None => "_unknown_".into(),
    };
    let chat_str = match chat_peer {
        tl::enums::Peer::User(u) => format!("`{}` _(DM)_", u.user_id),
        tl::enums::Peer::Chat(c) => format!("`{}` _(group)_", c.chat_id),
        tl::enums::Peer::Channel(c) => format!("`{}` _(channel/supergroup)_", c.channel_id),
    };
    let text = format!(
        "🪪 **IDs**\n\n\
        **Sender:** {sender_str}\n\
        **Chat:** {chat_str}\n\
        **Msg ID:** `{reply_to}`"
    );
    send_reply_md(client, peer, reply_to, &text).await;
}

async fn cmd_msgid(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    send_reply_md(
        client,
        peer,
        reply_to,
        &format!("📌 **Msg ID:** `{reply_to}`"),
    )
    .await;
}

async fn cmd_dc(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    match client.get_me().await {
        Ok(me) => {
            let text = format!(
                "🌐 **Connection Info**\n\n\
                **MTProto Layer:** `{}`\n\
                **User ID:** `{}`\n\
                **Bot:** {}",
                tl::LAYER,
                me.id,
                if me.bot { "✅" } else { "❌" },
            );
            send_reply_md(client, peer, reply_to, &text).await;
        }
        Err(e) => send_reply(client, peer, reply_to, &format!("❌ {e}")).await,
    }
}

async fn cmd_layer(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = format!(
        "📡 **layer library**\n\n\
        **MTProto Layer:** `{}`\n\
        **Crate:** `layer-client 0.4.5`\n\
        **Language:** Rust 🦀\n\
        **GitHub:** https://github.com/ankit-chaubey/layer",
        tl::LAYER
    );
    send_reply_md(client, peer, reply_to, &text).await;
}

async fn cmd_time(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let now = Utc::now();
    let text = format!(
        "🕐 **Time**\n\n\
        **Date:** {}\n\
        **UTC:** `{}`\n\
        **Unix:** `{}`",
        now.format("%A, %B %d %Y"),
        now.format("%H:%M:%S"),
        now.timestamp(),
    );
    send_reply_md(client, peer, reply_to, &text).await;
}

async fn cmd_del(client: &Client, msg_id: i32) {
    // Revoke = true so it's deleted for both sides
    let _ = client.delete_messages(vec![msg_id], true).await;
}

async fn cmd_dialogs(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    match client.get_dialogs(10).await {
        Ok(dialogs) => {
            let mut lines = vec!["📋 **Recent Dialogs**\n".to_string()];
            for (i, d) in dialogs.iter().enumerate() {
                let unread = d.unread_count();
                let badge = if unread > 0 {
                    format!("  🔴 {unread}")
                } else {
                    String::new()
                };
                lines.push(format!("{}.  {}{}", i + 1, d.title(), badge));
            }
            send_reply_md(client, peer, reply_to, &lines.join("\n")).await;
        }
        Err(e) => send_reply(client, peer, reply_to, &format!("❌ {e}")).await,
    }
}

async fn cmd_whois(
    client: &Client,
    peer: tl::enums::Peer,
    reply_to: i32,
    sender_uid: Option<i64>,
    chat_peer: &tl::enums::Peer,
) {
    let Some(uid) = sender_uid else {
        send_reply(client, peer, reply_to, "❓ Unknown sender.").await;
        return;
    };

    let ctx_peer = match chat_peer {
        tl::enums::Peer::Chat(c) => {
            tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: c.chat_id })
        }
        _ => tl::enums::InputPeer::Empty,
    };

    let req = tl::functions::users::GetUsers {
        id: vec![tl::enums::InputUser::FromMessage(
            tl::types::InputUserFromMessage {
                peer: ctx_peer,
                msg_id: reply_to,
                user_id: uid,
            },
        )],
    };

    match client.rpc_call_raw_pub(&req).await {
        Ok(body) => {
            let mut cur = Cursor::from_slice(&body);
            match Vec::<tl::enums::User>::deserialize(&mut cur) {
                Ok(users) => {
                    client.cache_users_slice_pub(&users).await;
                    match users.into_iter().next() {
                        Some(tl::enums::User::User(u)) => {
                            let uname = u
                                .username
                                .as_deref()
                                .map(|s| format!("@{s}"))
                                .unwrap_or_else(|| "_(none)_".into());
                            let text = format!(
                                "👤 **User Info**\n\n\
                                **Name:** {}\n\
                                **Username:** {}\n\
                                **ID:** `{}`\n\
                                **Bot:** {}\n\
                                **Verified:** {}\n\
                                **Premium:** {}\n\
                                **Deleted:** {}",
                                {
                                    let f = u.first_name.as_deref().unwrap_or("");
                                    let l = u.last_name.as_deref().unwrap_or("");
                                    format!("{f} {l}").trim().to_string()
                                },
                                uname,
                                u.id,
                                if u.bot { "✅" } else { "❌" },
                                if u.verified { "✅" } else { "❌" },
                                if u.premium { "✅" } else { "❌" },
                                if u.deleted { "✅" } else { "❌" },
                            );
                            send_reply_md(client, peer, reply_to, &text).await;
                        }
                        _ => send_reply(client, peer, reply_to, "❓ User not found.").await,
                    }
                }
                Err(e) => send_reply(client, peer, reply_to, &format!("❌ deserialize: {e}")).await,
            }
        }
        Err(e) => send_reply(client, peer, reply_to, &format!("❌ {e}")).await,
    }
}

async fn cmd_help(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = "📖 **layer-app — Userbot Commands**\n\n\
        **Info**\n\
        `.ping` — latency 🏓\n\
        `.me` — self info\n\
        `.id` — sender / chat / msg IDs\n\
        `.msgid` — this message ID\n\
        `.dc` — DC & layer version\n\
        `.layer` — library info\n\
        `.time` — current UTC time\n\
        `.whois` — sender's full info\n\
        `.dialogs` — last 10 dialogs\n\n\
        **Actions**\n\
        `.read` — mark chat as read\n\
        `.del` — delete this message\n\
        `.pin` — pin this message\n\
        `.unpin` — unpin this message\n\
        `.typing` — send typing action\n\n\
        **Text tools**\n\
        `.echo <text>` — echo\n\
        `.upper <text>` — UPPERCASE\n\
        `.lower <text>` — lowercase\n\
        `.rev <text>` — reverse\n\
        `.count <text>` — char / word stats\n\
        `.calc <expr>` — calculator\n\
        `.edit <text>` — edit this msg\n\
        `.fwd <@user>` — forward here → there\n\n\
        `.help` — this list";
    send_reply_md(client, peer, reply_to, text).await;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn send_reply(client: &Client, peer: tl::enums::Peer, reply_to: i32, text: &str) {
    let _ = client
        .send_message_to_peer_ex(peer, &InputMessage::text(text).reply_to(Some(reply_to)))
        .await;
}

async fn send_reply_md(client: &Client, peer: tl::enums::Peer, reply_to: i32, md: &str) {
    let (plain, ents) = parse_markdown(md);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

fn is_self_peer(peer: &tl::enums::Peer, my_id: i64) -> bool {
    matches!(peer, tl::enums::Peer::User(u) if u.user_id == my_id)
}

fn user_id_from_peer(peer: Option<&tl::enums::Peer>) -> Option<i64> {
    match peer? {
        tl::enums::Peer::User(u) => Some(u.user_id),
        _ => None,
    }
}

/// Split ".ping rest" → (".ping", "rest")
fn parse_dot_cmd(text: &str) -> (String, String) {
    match text.split_once(' ') {
        Some((cmd, rest)) => (cmd.to_ascii_lowercase(), rest.trim().to_string()),
        None => (text.to_ascii_lowercase(), String::new()),
    }
}

fn full_name(u: &tl::types::User) -> String {
    let f = u.first_name.as_deref().unwrap_or("");
    let l = u.last_name.as_deref().unwrap_or("");
    format!("{f} {l}").trim().to_string()
}

// ── Auth ──────────────────────────────────────────────────────────────────────

async fn do_login(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    if !BOT_TOKEN.is_empty() {
        println!("🤖 Signing in as bot…");
        client.bot_sign_in(BOT_TOKEN).await?;
        return Ok(());
    }
    if PHONE.is_empty() {
        eprintln!("Set PHONE or BOT_TOKEN in src/main.rs");
        std::process::exit(1);
    }
    println!("📱 Sending code to {PHONE}…");
    let token = client.request_login_code(PHONE).await?;
    let code = prompt("Enter the code: ")?;
    match client.sign_in(&token, &code).await {
        Ok(name) => println!("✅ Signed in as {name}"),
        Err(SignInError::PasswordRequired(pw)) => {
            let hint = pw.hint().unwrap_or("(no hint)");
            let pass = prompt(&format!("2FA password (hint: {hint}): "))?;
            client.check_password(*pw, pass.trim()).await?;
            println!("✅ 2FA ok");
        }
        Err(SignInError::SignUpRequired) => {
            eprintln!("✗ Number not registered. Sign up via the official Telegram app.");
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn prompt(msg: &str) -> io::Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

// ── Simple expression evaluator (no deps) ────────────────────────────────────

fn eval_expr(expr: &str) -> Result<String, String> {
    // Support: +  -  *  /  %  ^
    // Evaluate right-to-left for +/-, left-to-right for */ — simple single op.
    for op in ['+', '-', '*', '/', '%'] {
        let from = if op == '-' { 1 } else { 0 };
        if let Some(pos) = expr[from..].rfind(op).map(|p| p + from) {
            let lhs: f64 = expr[..pos]
                .trim()
                .parse()
                .map_err(|_| format!("bad number: '{}'", expr[..pos].trim()))?;
            let rhs: f64 = expr[pos + 1..]
                .trim()
                .parse()
                .map_err(|_| format!("bad number: '{}'", expr[pos + 1..].trim()))?;
            let res = match op {
                '+' => lhs + rhs,
                '-' => lhs - rhs,
                '*' => lhs * rhs,
                '/' => {
                    if rhs == 0.0 {
                        return Err("division by zero".into());
                    }
                    lhs / rhs
                }
                '%' => {
                    if rhs == 0.0 {
                        return Err("mod by zero".into());
                    }
                    lhs % rhs
                }
                _ => unreachable!(),
            };
            return Ok(fmt_num(res));
        }
    }
    expr.trim()
        .parse::<f64>()
        .map(fmt_num)
        .map_err(|_| format!("cannot evaluate '{expr}'"))
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
