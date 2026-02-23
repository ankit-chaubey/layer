//! layer-bot — Showcase bot built with layer-client.
//!
//! # Setup
//! 1. Set your API_ID, API_HASH and BOT_TOKEN below.
//! 2. `cargo run -p layer-bot`

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use layer_client::{Client, Config, InputMessage, parsers::parse_markdown, update::Update};
use layer_tl_types as tl;

// // ── Fill in your credentials ──────────────────────────────────────────────────
const API_ID:    i32  = 0;                       // https://my.telegram.org
const API_HASH:  &str = "";
const BOT_TOKEN: &str = "";        // from @BotFather
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "layer_client=info,layer_bot=info"); }
    }
    env_logger::init();
    if let Err(e) = run().await {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH == "YOUR_API_HASH" || BOT_TOKEN == "YOUR_BOT_TOKEN" {
        eprintln!("Set API_ID, API_HASH and BOT_TOKEN at the top of src/main.rs");
        std::process::exit(1);
    }

    println!("🔌 Connecting…");
    let client = Client::connect(Config {
        api_id:   API_ID,
        api_hash: API_HASH.to_string(),
        ..Default::default()
    }).await?;

    if !client.is_authorized().await? {
        println!("🤖 Signing in as bot…");
        client.bot_sign_in(BOT_TOKEN).await?;
        client.save_session().await?;
    }

    let me = client.get_me().await?;
    let bot_id = me.id;
    println!("✅ Logged in as @{} (id={bot_id})", me.username.as_deref().unwrap_or("bot"));
    println!("👂 Listening for updates… (Ctrl+C to quit)\n");

    // Arc so each spawned task gets its own shared handle
    let client = Arc::new(client);
    let me     = Arc::new(me);

    let mut updates = client.stream_updates();

    while let Some(update) = updates.next().await {
        let client = client.clone();
        let me     = me.clone();
        // Spawn each update into its own task so the receive loop never blocks
        tokio::spawn(async move {
            dispatch(update, client, me, bot_id).await;
        });
    }

    Ok(())
}

// ─── Central dispatcher ───────────────────────────────────────────────────────

async fn dispatch(update: Update, client: Arc<Client>, me: Arc<tl::types::User>, bot_id: i64) {
    match update {
        Update::NewMessage(msg) => {
            // Drop outgoing (bot's own messages echoed back as updates)
            if msg.outgoing() { return; }

            // Belt-and-suspenders: also drop by sender ID
            // (in groups, `out` flag can be absent for bot messages)
            if sender_user_id(&msg) == Some(bot_id) { return; }

            // Only handle commands
            let text = msg.text().unwrap_or("").trim().to_string();
            if !text.starts_with('/') { return; }

            let peer = match msg.peer_id() {
                Some(p) => p.clone(),
                None    => return,
            };
            let msg_id  = msg.id();
            let user_id = sender_user_id(&msg);
            let (cmd, arg) = split_command(&text, me.username.as_deref().unwrap_or(""));

            match cmd.as_deref() {
                Some("/start")   => handle_start(&client, peer, msg_id).await,
                Some("/help")    => handle_help(&client, peer, msg_id).await,
                Some("/ping")    => handle_ping(&client, peer, msg_id).await,
                Some("/info")    => handle_info(&client, peer, msg_id, &me).await,
                Some("/id")      => handle_id(&client, peer.clone(), msg_id, user_id, &peer).await,
                Some("/echo")    => handle_echo(&client, peer, msg_id, &arg).await,
                Some("/upper")   => handle_transform(&client, peer, msg_id, &arg, |s| s.to_uppercase()).await,
                Some("/lower")   => handle_transform(&client, peer, msg_id, &arg, |s| s.to_lowercase()).await,
                Some("/reverse") => handle_transform(&client, peer, msg_id, &arg, |s| s.chars().rev().collect()).await,
                Some("/count")   => handle_count(&client, peer, msg_id, &arg).await,
                Some("/calc")    => handle_calc(&client, peer, msg_id, &arg).await,
                Some("/time")    => handle_time(&client, peer, msg_id).await,
                Some("/about")   => handle_about(&client, peer, msg_id).await,
                _ => {
                    let _ = client.send_message_to_peer_ex(
                        peer,
                        &InputMessage::text("❓ Unknown command. Use /help to see all commands.")
                            .reply_to(Some(msg_id)),
                    ).await;
                }
            }
        }

        Update::CallbackQuery(cb) => {
            let data = cb.data().unwrap_or("").to_string();
            let qid  = cb.query_id;
            match data.as_str() {
                "cb:ping"  => { let _ = client.answer_callback_query(qid, Some("🏓 Pong!"), false).await; }
                "cb:time"  => {
                    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                    let _ = client.answer_callback_query(qid, Some(&now), false).await;
                }
                "cb:about" => { let _ = client.answer_callback_query(qid, Some("Built with layer — a Rust MTProto library 🦀"), true).await; }
                "cb:help"  => { let _ = client.answer_callback_query(qid, Some("Use /help to see all commands"), false).await; }
                _          => { let _ = client.answer_callback_query(qid, Some("🤷 Unknown action"), false).await; }
            }
        }

        Update::InlineQuery(iq) => {
            let q   = iq.query().to_string();
            let qid = iq.query_id;
            let results = if q.is_empty() {
                vec![
                    make_inline_article("1", "🕐 Current Time", &Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()),
                    make_inline_article("2", "🤖 About layer-bot", "Built with layer — Rust MTProto library 🦀"),
                    make_inline_article("3", "📖 Help", "Send /help to see all commands"),
                ]
            } else {
                vec![
                    make_inline_article("u", &format!("UPPER: {}", q.to_uppercase()), &q.to_uppercase()),
                    make_inline_article("l", &format!("lower: {}", q.to_lowercase()), &q.to_lowercase()),
                    make_inline_article("r", &format!("Reversed: {}", q.chars().rev().collect::<String>()), &q.chars().rev().collect::<String>()),
                    make_inline_article("c",
                        &format!("{} chars, {} words", q.len(), q.split_whitespace().count()),
                        &format!("{} characters, {} words", q.len(), q.split_whitespace().count())),
                ]
            };
            let _ = client.answer_inline_query(qid, results, 30, false, None).await;
        }

        _ => {}
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn handle_start(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = "👋 **Welcome to layer-bot!**\n\n\
        Showcase bot built with **layer** — a Telegram MTProto library in Rust 🦀\n\n\
        Use the buttons below or send /help for all commands.";
    let keyboard = inline_keyboard(vec![
        vec![btn_callback("🏓 Ping", "cb:ping"), btn_callback("🕐 Time", "cb:time")],
        vec![btn_callback("📖 Help", "cb:help"), btn_callback("ℹ️ About", "cb:about")],
        vec![btn_url("⭐ Star on GitHub", "https://github.com/ankit-chaubey/layer")],
    ]);
    let (plain, ents) = parse_markdown(text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_markup(keyboard).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_help(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = "📖 **Commands**\n\n\
        /ping — Latency 🏓\n\
        /time — UTC date & time 🕐\n\
        /calc `<expr>` — Calculator\n\
        /echo `<text>` — Echo text\n\
        /upper `<text>` — UPPERCASE\n\
        /lower `<text>` — lowercase\n\
        /reverse `<text>` — esreveR\n\
        /count `<text>` — Stats\n\
        /id — Your & chat IDs\n\
        /info — Bot info\n\
        /about — About\n\n\
        **Inline:** `@bot <text>` in any chat";
    let (plain, ents) = parse_markdown(text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_ping(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    // Measure RTT of one RPC send — single message only.
    // A two-message ping (send "Pinging…" then edit) would require the sent
    // message ID which send_message_to_peer_ex doesn't return. Keeping it simple.
    let start = Instant::now();
    let _ = client.send_message_to_peer_ex(
        peer.clone(),
        &InputMessage::text("🏓 …").reply_to(Some(reply_to)),
    ).await;
    let ms = start.elapsed().as_millis();
    let (plain, ents) = parse_markdown(&format!("🏓 **Pong!** `{ms} ms`"));
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_info(client: &Client, peer: tl::enums::Peer, reply_to: i32, me: &tl::types::User) {
    let first    = me.first_name.as_deref().unwrap_or("");
    let last     = me.last_name.as_deref().unwrap_or("");
    let name     = format!("{first} {last}").trim().to_string();
    let username = me.username.as_deref().unwrap_or("(none)");
    let text = format!(
        "🤖 **Bot Info**\n\n\
        **Name:** {name}\n\
        **Username:** @{username}\n\
        **ID:** `{}`\n\
        **Bot:** {}\n\
        **Verified:** {}",
        me.id,
        if me.bot { "✅" } else { "❌" },
        if me.verified { "✅" } else { "❌" },
    );
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_id(client: &Client, peer: tl::enums::Peer, reply_to: i32, user_id: Option<i64>, chat_peer: &tl::enums::Peer) {
    let user_str = match user_id {
        Some(id) => format!("`{id}`"),
        None     => "_(unknown)_".to_string(),
    };
    let chat_str = match chat_peer {
        tl::enums::Peer::User(u)    => format!("`{}` _(private)_",            u.user_id),
        tl::enums::Peer::Chat(c)    => format!("`{}` _(group)_",              c.chat_id),
        tl::enums::Peer::Channel(c) => format!("`{}` _(channel/supergroup)_", c.channel_id),
    };
    let text = format!("🪪 **IDs**\n\n**User:** {user_str}\n**Chat:** {chat_str}");
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_echo(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "💬 Usage: /echo <text>".to_string()
    } else {
        format!("💬 **Echo:**\n\n{arg}")
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_transform<F: Fn(&str) -> String>(
    client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str, f: F,
) {
    let text = if arg.is_empty() {
        "Usage: <command> <text>".to_string()
    } else {
        format!("`{}`", f(arg))
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_count(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "📊 Usage: /count <text>".to_string()
    } else {
        format!(
            "📊 **Stats**\n\n\
            **Chars:** `{}`\n**Bytes:** `{}`\n**Words:** `{}`\n**Lines:** `{}`",
            arg.chars().count(), arg.len(),
            arg.split_whitespace().count(), arg.lines().count(),
        )
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_calc(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "🧮 Usage: /calc <expr>  e.g. /calc 12 * 7".to_string()
    } else {
        match eval_expr(arg.trim()) {
            Ok(v)  => format!("🧮 `{arg}` = **{v}**"),
            Err(e) => format!("❌ {e}"),
        }
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_time(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let now  = Utc::now();
    let text = format!(
        "🕐 **Time**\n\n\
        **Date:** {}\n**Time:** `{}` UTC\n**Unix:** `{}`",
        now.format("%A, %B %d %Y"),
        now.format("%H:%M:%S"),
        now.timestamp(),
    );
    let (plain, ents) = parse_markdown(&text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_to(Some(reply_to)),
    ).await;
}

async fn handle_about(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text =
        "ℹ️ **About layer-bot**\n\n\
        Built with **layer** — async Telegram MTProto in pure **Rust** 🦀\n\n\
        **Features:** Commands · Inline keyboards · Callback queries · \
        Inline mode · Markdown entities · Concurrent update handling\n\n\
        **[layer on GitHub](https://github.com/ankit-chaubey/layer)** · Layer 222";
    let keyboard = inline_keyboard(vec![
        vec![btn_url("⭐ Star on GitHub", "https://github.com/ankit-chaubey/layer")],
    ]);
    let (plain, ents) = parse_markdown(text);
    let _ = client.send_message_to_peer_ex(peer,
        &InputMessage::text(plain).entities(ents).reply_markup(keyboard).reply_to(Some(reply_to)),
    ).await;
}

// ─── Keyboard helpers ─────────────────────────────────────────────────────────

fn inline_keyboard(rows: Vec<Vec<tl::enums::KeyboardButton>>) -> tl::enums::ReplyMarkup {
    tl::enums::ReplyMarkup::ReplyInlineMarkup(tl::types::ReplyInlineMarkup {
        rows: rows.into_iter().map(|row| {
            tl::enums::KeyboardButtonRow::KeyboardButtonRow(
                tl::types::KeyboardButtonRow { buttons: row }
            )
        }).collect(),
    })
}

fn btn_callback(text: &str, data: &str) -> tl::enums::KeyboardButton {
    tl::enums::KeyboardButton::Callback(tl::types::KeyboardButtonCallback {
        requires_password: false,
        style: None,
        text:  text.to_string(),
        data:  data.as_bytes().to_vec(),
    })
}

fn btn_url(text: &str, url: &str) -> tl::enums::KeyboardButton {
    tl::enums::KeyboardButton::Url(tl::types::KeyboardButtonUrl {
        style: None,
        text:  text.to_string(),
        url:   url.to_string(),
    })
}

// ─── Inline results ───────────────────────────────────────────────────────────

fn make_inline_article(id: &str, title: &str, content: &str) -> tl::enums::InputBotInlineResult {
    tl::enums::InputBotInlineResult::InputBotInlineResult(tl::types::InputBotInlineResult {
        id:          id.to_string(),
        r#type:      "article".to_string(),
        title:       Some(title.to_string()),
        description: Some(content.to_string()),
        url:    None, thumb: None, content: None,
        send_message: tl::enums::InputBotInlineMessage::Text(
            tl::types::InputBotInlineMessageText {
                no_webpage: false, invert_media: false,
                message:      content.to_string(),
                entities:     None,
                reply_markup: None,
            },
        ),
    })
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn split_command(text: &str, bot_username: &str) -> (Option<String>, String) {
    if !text.starts_with('/') { return (None, text.to_string()); }
    let (cmd_raw, rest) = text.split_once(' ')
        .map(|(c, r)| (c, r.trim()))
        .unwrap_or((text, ""));
    let cmd = if let Some(pos) = cmd_raw.find('@') {
        let suffix = &cmd_raw[pos + 1..];
        if suffix.eq_ignore_ascii_case(bot_username) { &cmd_raw[..pos] } else { cmd_raw }
    } else { cmd_raw };
    (Some(cmd.to_ascii_lowercase()), rest.to_string())
}

fn sender_user_id(msg: &layer_client::update::IncomingMessage) -> Option<i64> {
    match msg.sender_id() {
        Some(tl::enums::Peer::User(u)) => Some(u.user_id),
        _                              => None,
    }
}

fn eval_expr(expr: &str) -> Result<String, String> {
    for op in ['+', '-', '*', '/'] {
        let from = if op == '-' { 1 } else { 0 };
        if let Some(pos) = expr[from..].rfind(op).map(|p| p + from) {
            let lhs: f64 = expr[..pos].trim().parse()
                .map_err(|_| format!("cannot parse '{}'", expr[..pos].trim()))?;
            let rhs: f64 = expr[pos+1..].trim().parse()
                .map_err(|_| format!("cannot parse '{}'", expr[pos+1..].trim()))?;
            let result = match op {
                '+' => lhs + rhs,
                '-' => lhs - rhs,
                '*' => lhs * rhs,
                '/' => { if rhs == 0.0 { return Err("Division by zero".into()); } lhs / rhs }
                _   => unreachable!(),
            };
            return Ok(if result.fract() == 0.0 && result.abs() < 1e15 {
                format!("{}", result as i64)
            } else {
                format!("{result:.6}").trim_end_matches('0').trim_end_matches('.').to_string()
            });
        }
    }
    expr.trim().parse::<f64>().map(|n| format!("{n}"))
        .map_err(|_| format!("cannot evaluate '{expr}'"))
}
