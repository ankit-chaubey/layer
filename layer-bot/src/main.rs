//! layer-bot — Showcase bot for the **layer** MTProto library.
//!
//! Demonstrates: commands, inline keyboards, callback queries, inline mode,
//! Markdown entities, concurrent update handling, and utility features.
//!
//! # Setup
//! 1. Fill API_ID, API_HASH, BOT_TOKEN below.
//! 2. `cargo run -p layer-bot`
//!
//! # Commands
//! /start   /help    /ping    /id      /info    /time    /about
//! /calc    /echo    /upper   /lower   /reverse /count   /len
//! /roll    /flip    /random  /password /uuid   /layer
//! /b64enc  /b64dec  /rot13   /hex     /unhex   /morse   /unmorse
//! /hash    /fact    /joke
//! Inline mode: @bot <text>

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use chrono::Utc;
use rand::{Rng, thread_rng};
use sha2::{Digest, Sha256};

use layer_client::{Client, Config, InputMessage, parsers::parse_markdown, update::Update};
use layer_tl_types as tl;

// ── Credentials ───────────────────────────────────────────────────────────────
const API_ID: i32 = 0; // https://my.telegram.org
const API_HASH: &str = "";
const BOT_TOKEN: &str = ""; // from @BotFather
// ─────────────────────────────────────────────────────────────────────────────

static MSG_COUNT: AtomicU64 = AtomicU64::new(0);
static START_TS: AtomicU64 = AtomicU64::new(0);

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "layer_client=warn,layer_bot=info");
        }
    }
    env_logger::init();
    if let Err(e) = run().await {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if API_ID == 0 || API_HASH.is_empty() || BOT_TOKEN.is_empty() {
        eprintln!("Set API_ID, API_HASH and BOT_TOKEN at the top of src/main.rs");
        std::process::exit(1);
    }

    START_TS.store(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        Ordering::Relaxed,
    );

    println!("🔌 Connecting…");
    let (client, _shutdown) = Client::connect(Config {
        api_id: API_ID,
        api_hash: API_HASH.to_string(),
        ..Default::default()
    })
    .await?;

    if !client.is_authorized().await? {
        println!("🤖 Signing in as bot…");
        client.bot_sign_in(BOT_TOKEN).await?;
        client.save_session().await?;
    }

    let me = client.get_me().await?;
    println!(
        "✅ @{} (id={}) ready — listening… (Ctrl+C to quit)\n",
        me.username.as_deref().unwrap_or("bot"),
        me.id
    );

    let client = Arc::new(client);
    let me = Arc::new(me);
    let bot_id = me.id;
    let mut stream = client.stream_updates();

    while let Some(upd) = stream.next().await {
        let c = client.clone();
        let me = me.clone();
        tokio::spawn(async move { dispatch(upd, c, me, bot_id).await });
    }

    Ok(())
}

// ─── Dispatcher ───────────────────────────────────────────────────────────────

async fn dispatch(upd: Update, client: Arc<Client>, me: Arc<tl::types::User>, bot_id: i64) {
    match upd {
        // ── Slash-commands ─────────────────────────────────────────────────
        Update::NewMessage(msg) => {
            if msg.outgoing() {
                return;
            }
            if sender_uid(&msg) == Some(bot_id) {
                return;
            }

            let text = msg.text().unwrap_or("").trim().to_string();
            if !text.starts_with('/') {
                return;
            }

            let Some(peer) = msg.peer_id().cloned() else {
                return;
            };
            let msg_id = msg.id();
            let uid = sender_uid(&msg);
            MSG_COUNT.fetch_add(1, Ordering::Relaxed);

            let (cmd, arg) = split_cmd(&text, me.username.as_deref().unwrap_or(""));

            println!(
                "📨 [msg={}{}] {}",
                msg_id,
                uid.map(|id| format!(" uid={id}")).unwrap_or_default(),
                &text[..text.len().min(80)]
            );

            match cmd.as_deref() {
                // ── Core ────────────────────────────────────────────────────
                Some("/start") => h_start(&client, peer, msg_id).await,
                Some("/help") => h_help(&client, peer, msg_id).await,
                Some("/ping") => h_ping(&client, peer, msg_id).await,
                Some("/id") => h_id(&client, peer.clone(), msg_id, uid, &peer).await,
                Some("/info") => h_info(&client, peer, msg_id, &me).await,
                Some("/time") => h_time(&client, peer, msg_id).await,
                Some("/about") => h_about(&client, peer, msg_id).await,
                Some("/layer") => h_layer(&client, peer, msg_id).await,
                Some("/stats") => h_stats(&client, peer, msg_id).await,

                // ── Text transforms ─────────────────────────────────────────
                Some("/echo") => h_echo(&client, peer, msg_id, &arg).await,
                Some("/upper") => {
                    h_transform(&client, peer, msg_id, &arg, |s| s.to_uppercase()).await
                }
                Some("/lower") => {
                    h_transform(&client, peer, msg_id, &arg, |s| s.to_lowercase()).await
                }
                Some("/reverse") => {
                    h_transform(&client, peer, msg_id, &arg, |s| s.chars().rev().collect()).await
                }
                Some("/rot13") => h_transform(&client, peer, msg_id, &arg, rot13).await,
                Some("/count") => h_count(&client, peer, msg_id, &arg).await,
                Some("/len") => h_len(&client, peer, msg_id, &arg).await,

                // ── Encoding ────────────────────────────────────────────────
                Some("/b64enc") => h_b64enc(&client, peer, msg_id, &arg).await,
                Some("/b64dec") => h_b64dec(&client, peer, msg_id, &arg).await,
                Some("/hex") => h_hex(&client, peer, msg_id, &arg).await,
                Some("/unhex") => h_unhex(&client, peer, msg_id, &arg).await,
                Some("/hash") => h_hash(&client, peer, msg_id, &arg).await,
                Some("/morse") => h_morse(&client, peer, msg_id, &arg, true).await,
                Some("/unmorse") => h_morse(&client, peer, msg_id, &arg, false).await,

                // ── Math / random ────────────────────────────────────────────
                Some("/calc") => h_calc(&client, peer, msg_id, &arg).await,
                Some("/roll") => h_roll(&client, peer, msg_id, &arg).await,
                Some("/flip") => h_flip(&client, peer, msg_id).await,
                Some("/random") => h_random(&client, peer, msg_id, &arg).await,

                // ── Generators ──────────────────────────────────────────────
                Some("/password") => h_password(&client, peer, msg_id, &arg).await,
                Some("/uuid") => h_uuid(&client, peer, msg_id).await,

                // ── Fun ─────────────────────────────────────────────────────
                Some("/fact") => h_fact(&client, peer, msg_id).await,
                Some("/joke") => h_joke(&client, peer, msg_id).await,

                _ => {
                    let _ = client
                        .send_message_to_peer_ex(
                            peer,
                            &InputMessage::text("❓ Unknown command. /help for the full list.")
                                .reply_to(Some(msg_id)),
                        )
                        .await;
                }
            }
        }

        // ── Callback queries (button taps) ─────────────────────────────────
        Update::CallbackQuery(cb) => {
            let data = cb.data().unwrap_or("").to_string();
            let qid = cb.query_id;
            println!("🔘 callback [qid={qid}] data={data}");
            match data.as_str() {
                "cb:ping" => {
                    let _ = client
                        .answer_callback_query(qid, Some("🏓 Pong!"), false)
                        .await;
                }
                "cb:time" => {
                    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                    let _ = client.answer_callback_query(qid, Some(&now), false).await;
                }
                "cb:about" => {
                    let _ = client
                        .answer_callback_query(
                            qid,
                            Some("Built with layer — Rust MTProto 🦀"),
                            true,
                        )
                        .await;
                }
                "cb:stats" => {
                    let uptime = uptime_str();
                    let msgs = MSG_COUNT.load(Ordering::Relaxed);
                    let _ = client
                        .answer_callback_query(
                            qid,
                            Some(&format!("⏱ {uptime} | 📨 {msgs} messages")),
                            false,
                        )
                        .await;
                }
                "cb:layer" => {
                    let _ = client
                        .answer_callback_query(
                            qid,
                            Some(&format!("Layer {} · layer-client 0.4.5 🦀", tl::LAYER)),
                            false,
                        )
                        .await;
                }
                "cb:help" => {
                    let _ = client
                        .answer_callback_query(qid, Some("Send /help to see all commands"), false)
                        .await;
                }
                "cb:joke" => {
                    let j = random_joke();
                    let _ = client.answer_callback_query(qid, Some(j), true).await;
                }
                "cb:fact" => {
                    let f = random_fact();
                    let _ = client.answer_callback_query(qid, Some(f), true).await;
                }
                _ => {
                    let _ = client
                        .answer_callback_query(qid, Some("🤷 Unknown action"), false)
                        .await;
                }
            }
        }

        // ── Inline queries (@bot <text>) ────────────────────────────────────
        Update::InlineQuery(iq) => {
            let q = iq.query().trim().to_string();
            let qid = iq.query_id;
            println!("🔍 inline [qid={qid}] q={q:?}");

            let results = if q.is_empty() {
                // Default menu when user just types @bot with no text
                vec![
                    inline_article(
                        "1",
                        "🕐 Current UTC time",
                        &Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    ),
                    inline_article(
                        "2",
                        "📡 Layer version",
                        &format!("MTProto Layer {} · layer-client 🦀", tl::LAYER),
                    ),
                    inline_article(
                        "3",
                        "🎲 Random number (1–100)",
                        &thread_rng().gen_range(1..=100_u32).to_string(),
                    ),
                    inline_article(
                        "4",
                        "🪙 Coin flip",
                        if thread_rng().gen_bool(0.5) {
                            "Heads 🪙"
                        } else {
                            "Tails 🟡"
                        },
                    ),
                    inline_article("5", "😂 Random joke", random_joke()),
                    inline_article("6", "💡 Random fact", random_fact()),
                ]
            } else {
                // Transform the query text
                let rev: String = q.chars().rev().collect();
                let rot = rot13(&q);
                let b64 = B64.encode(q.as_bytes());
                let hex_out: String = q.bytes().map(|b| format!("{b:02x}")).collect();
                let sha_hex = sha256_hex(q.as_bytes());
                let word_cnt = q.split_whitespace().count();
                let char_cnt = q.chars().count();

                vec![
                    inline_article(
                        "u",
                        &format!("⬆️  UPPER: {}", q.to_uppercase()),
                        &q.to_uppercase(),
                    ),
                    inline_article(
                        "l",
                        &format!("⬇️  lower: {}", q.to_lowercase()),
                        &q.to_lowercase(),
                    ),
                    inline_article("r", &format!("🔃 Reversed: {rev}"), &rev),
                    inline_article("t", &format!("🔄 ROT-13: {rot}"), &rot),
                    inline_article("b", &format!("📦 Base64: {b64}"), &b64),
                    inline_article("h", &format!("🔣 Hex: {hex_out}"), &hex_out),
                    inline_article("s", &format!("🔐 SHA-256: {sha_hex}"), &sha_hex),
                    inline_article(
                        "c",
                        &format!("📊 Stats: {char_cnt} chars, {word_cnt} words"),
                        &format!("{char_cnt} characters, {word_cnt} words"),
                    ),
                ]
            };

            let _ = client
                .answer_inline_query(qid, results, 30, false, None)
                .await;
        }

        _ => {}
    }
}

// ─── Handler implementations ─────────────────────────────────────────────────

async fn h_start(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = "👋 **Welcome to layer-bot!**\n\n\
        A production-grade showcase bot built with **layer** — \
        a pure-Rust async Telegram MTProto library 🦀\n\n\
        Use the buttons below to explore, or send /help for every command.";
    let kb = keyboard(vec![
        vec![btn_cb("🏓 Ping", "cb:ping"), btn_cb("🕐 Time", "cb:time")],
        vec![
            btn_cb("📊 Stats", "cb:stats"),
            btn_cb("📡 Layer", "cb:layer"),
        ],
        vec![btn_cb("😂 Joke", "cb:joke"), btn_cb("💡 Fact", "cb:fact")],
        vec![btn_cb("📖 Help", "cb:help"), btn_cb("ℹ️ About", "cb:about")],
        vec![btn_url(
            "⭐ Star on GitHub",
            "https://github.com/ankit-chaubey/layer",
        )],
    ]);
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_markup(kb)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_help(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = "📖 **layer-bot Commands**\n\n\
        **Core**\n\
        /ping — latency 🏓\n\
        /id — user & chat IDs\n\
        /info — bot info\n\
        /time — UTC date & time\n\
        /stats — uptime & message count\n\
        /layer — MTProto layer version\n\
        /about — about this bot\n\n\
        **Text tools**\n\
        /echo `<text>` — echo\n\
        /upper `<text>` — UPPERCASE\n\
        /lower `<text>` — lowercase\n\
        /reverse `<text>` — reverse\n\
        /rot13 `<text>` — ROT-13 cipher\n\
        /count `<text>` — char/word/line stats\n\
        /len `<text>` — length\n\n\
        **Encoding & hashing**\n\
        /b64enc `<text>` — Base64 encode\n\
        /b64dec `<text>` — Base64 decode\n\
        /hex `<text>` — text → hex\n\
        /unhex `<hex>` — hex → text\n\
        /hash `<text>` — SHA-256\n\
        /morse `<text>` — text → Morse\n\
        /unmorse `<morse>` — Morse → text\n\n\
        **Math & random**\n\
        /calc `<expr>` — calculator (+−×÷%)\n\
        /roll `[N]` — roll N-sided die (default 6)\n\
        /flip — coin flip 🪙\n\
        /random `[min] [max]` — random number\n\n\
        **Generators**\n\
        /password `[length]` — random password\n\
        /uuid — random UUID v4\n\n\
        **Fun**\n\
        /fact — random interesting fact\n\
        /joke — random joke\n\n\
        **Inline mode**\n\
        Type `@bot <text>` in any chat for instant transforms.";
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_ping(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let t = Instant::now();
    let ok = client
        .invoke(&tl::functions::Ping {
            ping_id: 0xDEAD_BEEF,
        })
        .await
        .is_ok();
    let rtt = t.elapsed().as_millis();
    let text = if ok {
        format!("🏓 **Pong!** `{rtt}ms`\n\n_Measured from handler entry to MTProto response._")
    } else {
        "🏓 Pong! _(timeout)_".into()
    };
    let kb = keyboard(vec![vec![
        btn_cb("🔁 Ping again", "cb:ping"),
        btn_cb("🕐 Time", "cb:time"),
    ]]);
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_markup(kb)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_id(
    client: &Client,
    peer: tl::enums::Peer,
    reply_to: i32,
    uid: Option<i64>,
    chat_peer: &tl::enums::Peer,
) {
    let user_str = match uid {
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
        **Your ID:** {user_str}\n\
        **Chat ID:** {chat_str}\n\
        **Msg ID:** `{reply_to}`"
    );
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_info(client: &Client, peer: tl::enums::Peer, reply_to: i32, me: &tl::types::User) {
    let f = me.first_name.as_deref().unwrap_or("");
    let l = me.last_name.as_deref().unwrap_or("");
    let un = me.username.as_deref().unwrap_or("(none)");
    let text = format!(
        "🤖 **Bot Info**\n\n\
        **Name:** {} {}\n\
        **Username:** @{un}\n\
        **ID:** `{}`\n\
        **Bot:** {}\n\
        **Verified:** {}\n\
        **Layer:** `{}`",
        f,
        l,
        me.id,
        if me.bot { "✅" } else { "❌" },
        if me.verified { "✅" } else { "❌" },
        tl::LAYER,
    );
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_time(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let now = Utc::now();
    let text = format!(
        "🕐 **Date & Time**\n\n\
        **Date:** {}\n\
        **Time:** `{}` UTC\n\
        **Unix:** `{}`",
        now.format("%A, %B %d %Y"),
        now.format("%H:%M:%S"),
        now.timestamp(),
    );
    let kb = keyboard(vec![vec![btn_cb("🔁 Refresh", "cb:time")]]);
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_markup(kb)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_stats(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let msgs = MSG_COUNT.load(Ordering::Relaxed);
    let uptime = uptime_str();
    let text = format!(
        "📊 **Bot Stats**\n\n\
        **Uptime:** {uptime}\n\
        **Messages handled:** `{msgs}`\n\
        **MTProto Layer:** `{}`\n\
        **Library:** layer-client 0.4.5 🦀",
        tl::LAYER
    );
    let kb = keyboard(vec![vec![btn_cb("🔁 Refresh", "cb:stats")]]);
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_markup(kb)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_layer(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = format!(
        "📡 **layer library**\n\n\
        **MTProto Layer:** `{}`\n\
        **Crate:** `layer-client 0.4.5`\n\
        **Language:** Rust 🦀\n\
        **Features:** User auth · Bot auth · Inline keyboards · \
        Callback queries · Inline mode · Markdown entities · \
        FLOOD\\_WAIT retry · DC migration · Session persistence\n\n\
        Built by **Ankit Chaubey** (vasu)",
        tl::LAYER
    );
    let kb = keyboard(vec![vec![
        btn_url("⭐ GitHub", "https://github.com/ankit-chaubey/layer"),
        btn_cb("📡 Version", "cb:layer"),
    ]]);
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_markup(kb)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_about(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let text = format!(
        "ℹ️ **About layer-bot**\n\n\
        This bot is a **showcase** for the **layer** MTProto library — \
        a pure-Rust async Telegram client.\n\n\
        **What it demonstrates:**\n\
        • Slash-commands with argument parsing\n\
        • Inline keyboards & callback queries\n\
        • Inline mode results\n\
        • Markdown entities (bold, code, italic)\n\
        • Concurrent update handling via `tokio::spawn`\n\
        • Utility commands (encoding, hashing, math, random)\n\n\
        **MTProto Layer:** `{}`\n\
        **Author:** Ankit Chaubey (vasu)",
        tl::LAYER
    );
    let kb = keyboard(vec![
        vec![btn_url(
            "⭐ Star on GitHub",
            "https://github.com/ankit-chaubey/layer",
        )],
        vec![
            btn_cb("📊 Stats", "cb:stats"),
            btn_cb("📡 Layer", "cb:layer"),
        ],
    ]);
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_markup(kb)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_echo(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "💬 Usage: `/echo <text>`".into()
    } else {
        format!("💬 {arg}")
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_transform<F: Fn(&str) -> String>(
    client: &Client,
    peer: tl::enums::Peer,
    reply_to: i32,
    arg: &str,
    f: F,
) {
    let text = if arg.is_empty() {
        "Usage: `<command> <text>`".into()
    } else {
        format!("`{}`", f(arg))
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_count(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "📊 Usage: `/count <text>`".into()
    } else {
        format!(
            "📊 **Text Stats**\n\n\
            **Chars:** `{}`\n\
            **Bytes:** `{}`\n\
            **Words:** `{}`\n\
            **Lines:** `{}`",
            arg.chars().count(),
            arg.len(),
            arg.split_whitespace().count(),
            arg.lines().count(),
        )
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_len(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "📏 Usage: `/len <text>`".into()
    } else {
        format!(
            "📏 **Length:** `{}` chars / `{}` bytes",
            arg.chars().count(),
            arg.len()
        )
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_b64enc(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "📦 Usage: `/b64enc <text>`".into()
    } else {
        format!("📦 **Base64:**\n`{}`", B64.encode(arg.as_bytes()))
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_b64dec(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "📦 Usage: `/b64dec <base64>`".into()
    } else {
        match B64.decode(arg.trim()) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => format!("📦 **Decoded:**\n`{s}`"),
                Err(_) => "❌ Decoded bytes are not valid UTF-8.".into(),
            },
            Err(_) => "❌ Invalid Base64 input.".into(),
        }
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_hex(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "🔣 Usage: `/hex <text>`".into()
    } else {
        let h: String = arg
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("🔣 **Hex:**\n`{h}`")
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_unhex(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "🔣 Usage: `/unhex <hex>`".into()
    } else {
        let stripped: String = arg.chars().filter(|c| !c.is_whitespace()).collect();
        if stripped.len() % 2 != 0 {
            "❌ Odd number of hex digits.".into()
        } else {
            match (0..stripped.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&stripped[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
            {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(s) => format!("🔣 **Decoded:**\n`{s}`"),
                    Err(_) => "❌ Hex decodes to non-UTF-8 bytes.".into(),
                },
                Err(_) => "❌ Invalid hex input.".into(),
            }
        }
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_hash(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "🔐 Usage: `/hash <text>`".into()
    } else {
        let sha = sha256_hex(arg.as_bytes());
        format!("🔐 **SHA-256**\n`{sha}`")
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_morse(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str, encode: bool) {
    let text = if arg.is_empty() {
        if encode {
            "🔔 Usage: `/morse <text>`".into()
        } else {
            "🔔 Usage: `/unmorse <morse code>`".into()
        }
    } else if encode {
        match text_to_morse(arg) {
            Ok(m) => format!("🔔 **Morse:**\n`{m}`"),
            Err(e) => format!("❌ {e}"),
        }
    } else {
        match morse_to_text(arg) {
            Ok(t) => format!("🔔 **Decoded:**\n`{t}`"),
            Err(e) => format!("❌ {e}"),
        }
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_calc(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let text = if arg.is_empty() {
        "🧮 Usage: `/calc <expr>`  e.g. `/calc 12 * 7`".into()
    } else {
        match eval_expr(arg.trim()) {
            Ok(v) => format!("🧮 `{arg}` = **{v}**"),
            Err(e) => format!("❌ {e}"),
        }
    };
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_roll(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let sides: u32 = arg.trim().parse().unwrap_or(6).max(2).min(1_000_000);
    let roll = thread_rng().gen_range(1..=sides);
    let text = format!("🎲 Rolling a **d{sides}**… → **{roll}**");
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_flip(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let result = if thread_rng().gen_bool(0.5) {
        "🪙 **Heads!**"
    } else {
        "🟡 **Tails!**"
    };
    let (plain, ents) = parse_markdown(result);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_random(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let parts: Vec<i64> = arg
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let (lo, hi) = match parts.as_slice() {
        [a, b] => (*a.min(b), *a.max(b)),
        [a] => (1, *a),
        _ => (1, 100),
    };
    let n = thread_rng().gen_range(lo..=hi);
    let text = format!("🎰 Random `{lo}..={hi}` → **{n}**");
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_password(client: &Client, peer: tl::enums::Peer, reply_to: i32, arg: &str) {
    let len: usize = arg.trim().parse().unwrap_or(16).clamp(4, 128);
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*-_=+?";
    let pwd: String = (0..len)
        .map(|_| {
            let idx = thread_rng().gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    let text = format!("🔑 **Password ({len} chars):**\n`{pwd}`");
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_uuid(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let uuid = random_uuid();
    let text = format!("🆔 **UUID v4:**\n`{uuid}`");
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_fact(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let fact = random_fact();
    let text = format!("💡 **Fun Fact**\n\n{fact}");
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

async fn h_joke(client: &Client, peer: tl::enums::Peer, reply_to: i32) {
    let joke = random_joke();
    let text = format!("😂 **Joke**\n\n{joke}");
    let (plain, ents) = parse_markdown(&text);
    let _ = client
        .send_message_to_peer_ex(
            peer,
            &InputMessage::text(plain)
                .entities(ents)
                .reply_to(Some(reply_to)),
        )
        .await;
}

// ─── Keyboard builders ────────────────────────────────────────────────────────

fn keyboard(rows: Vec<Vec<tl::enums::KeyboardButton>>) -> tl::enums::ReplyMarkup {
    tl::enums::ReplyMarkup::ReplyInlineMarkup(tl::types::ReplyInlineMarkup {
        rows: rows
            .into_iter()
            .map(|row| {
                tl::enums::KeyboardButtonRow::KeyboardButtonRow(tl::types::KeyboardButtonRow {
                    buttons: row,
                })
            })
            .collect(),
    })
}

fn btn_cb(text: &str, data: &str) -> tl::enums::KeyboardButton {
    tl::enums::KeyboardButton::Callback(tl::types::KeyboardButtonCallback {
        requires_password: false,
        style: None,
        text: text.to_string(),
        data: data.as_bytes().to_vec(),
    })
}

fn btn_url(text: &str, url: &str) -> tl::enums::KeyboardButton {
    tl::enums::KeyboardButton::Url(tl::types::KeyboardButtonUrl {
        style: None,
        text: text.to_string(),
        url: url.to_string(),
    })
}

// ─── Inline result builder ────────────────────────────────────────────────────

fn inline_article(id: &str, title: &str, content: &str) -> tl::enums::InputBotInlineResult {
    tl::enums::InputBotInlineResult::InputBotInlineResult(tl::types::InputBotInlineResult {
        id: id.to_string(),
        r#type: "article".to_string(),
        title: Some(title.to_string()),
        description: Some(content.to_string()),
        url: None,
        thumb: None,
        content: None,
        send_message: tl::enums::InputBotInlineMessage::Text(
            tl::types::InputBotInlineMessageText {
                no_webpage: false,
                invert_media: false,
                message: content.to_string(),
                entities: None,
                reply_markup: None,
            },
        ),
    })
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn split_cmd(text: &str, bot_username: &str) -> (Option<String>, String) {
    if !text.starts_with('/') {
        return (None, text.to_string());
    }
    let (cmd_raw, rest) = text
        .split_once(' ')
        .map(|(c, r)| (c, r.trim()))
        .unwrap_or((text, ""));
    let cmd = if let Some(pos) = cmd_raw.find('@') {
        let suffix = &cmd_raw[pos + 1..];
        if suffix.eq_ignore_ascii_case(bot_username) {
            &cmd_raw[..pos]
        } else {
            cmd_raw
        }
    } else {
        cmd_raw
    };
    (Some(cmd.to_ascii_lowercase()), rest.to_string())
}

fn sender_uid(msg: &layer_client::update::IncomingMessage) -> Option<i64> {
    match msg.sender_id()? {
        tl::enums::Peer::User(u) => Some(u.user_id),
        _ => None,
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='m' | 'A'..='M' => (c as u8 + 13) as char,
            'n'..='z' | 'N'..='Z' => (c as u8 - 13) as char,
            other => other,
        })
        .collect()
}

fn random_uuid() -> String {
    let mut rng = thread_rng();
    let mut b = [0u8; 16];
    rng.fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant bits
    let node: u64 = (b[10] as u64) << 40
        | (b[11] as u64) << 32
        | (b[12] as u64) << 24
        | (b[13] as u64) << 16
        | (b[14] as u64) << 8
        | (b[15] as u64);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        u16::from_be_bytes([b[4], b[5]]),
        u16::from_be_bytes([b[6], b[7]]),
        u16::from_be_bytes([b[8], b[9]]),
        node,
    )
}

fn uptime_str() -> String {
    let start = START_TS.load(Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now.saturating_sub(start);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h}h {m}m {s}s")
}

// ── Morse code ────────────────────────────────────────────────────────────────

const MORSE_TABLE: &[(&str, &str)] = &[
    ("a", ".-"),
    ("b", "-..."),
    ("c", "-.-."),
    ("d", "-.."),
    ("e", "."),
    ("f", "..-."),
    ("g", "--."),
    ("h", "...."),
    ("i", ".."),
    ("j", ".---"),
    ("k", "-.-"),
    ("l", ".-.."),
    ("m", "--"),
    ("n", "-."),
    ("o", "---"),
    ("p", ".--."),
    ("q", "--.-"),
    ("r", ".-."),
    ("s", "..."),
    ("t", "-"),
    ("u", "..-"),
    ("v", "...-"),
    ("w", ".--"),
    ("x", "-..-"),
    ("y", "-.--"),
    ("z", "--.."),
    ("0", "-----"),
    ("1", ".----"),
    ("2", "..---"),
    ("3", "...--"),
    ("4", "....-"),
    ("5", "....."),
    ("6", "-...."),
    ("7", "--..."),
    ("8", "---.."),
    ("9", "----."),
];

fn text_to_morse(text: &str) -> Result<String, String> {
    let mut out = Vec::new();
    for ch in text.to_lowercase().chars() {
        if ch == ' ' {
            out.push("/".to_string());
            continue;
        }
        let key = ch.to_string();
        match MORSE_TABLE.iter().find(|(k, _)| *k == key.as_str()) {
            Some((_, m)) => out.push(m.to_string()),
            None => return Err(format!("No Morse for character '{ch}'")),
        }
    }
    Ok(out.join(" "))
}

fn morse_to_text(morse: &str) -> Result<String, String> {
    let mut out = String::new();
    for word in morse.trim().split(" / ") {
        for code in word.split_whitespace() {
            match MORSE_TABLE.iter().find(|(_, m)| *m == code) {
                Some((ch, _)) => out.push_str(ch),
                None => return Err(format!("Unknown Morse code '{code}'")),
            }
        }
        out.push(' ');
    }
    Ok(out.trim().to_string())
}

// ── Calculator ────────────────────────────────────────────────────────────────

fn eval_expr(expr: &str) -> Result<String, String> {
    for op in ['+', '-', '*', '/', '%'] {
        let from = if op == '-' { 1 } else { 0 };
        if let Some(pos) = expr[from..].rfind(op).map(|p| p + from) {
            let lhs: f64 = expr[..pos]
                .trim()
                .parse()
                .map_err(|_| format!("bad left operand: '{}'", expr[..pos].trim()))?;
            let rhs: f64 = expr[pos + 1..]
                .trim()
                .parse()
                .map_err(|_| format!("bad right operand: '{}'", expr[pos + 1..].trim()))?;
            let res = match op {
                '+' => lhs + rhs,
                '-' => lhs - rhs,
                '*' => lhs * rhs,
                '/' => {
                    if rhs == 0.0 {
                        return Err("Division by zero".into());
                    }
                    lhs / rhs
                }
                '%' => {
                    if rhs == 0.0 {
                        return Err("Modulo by zero".into());
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

// ── Static content ────────────────────────────────────────────────────────────

fn random_fact() -> &'static str {
    const FACTS: &[&str] = &[
        "Honey never spoils. Archaeologists found 3000-year-old honey in Egyptian tombs still perfectly edible.",
        "A day on Venus is longer than a year on Venus.",
        "Octopuses have three hearts and blue blood.",
        "The Eiffel Tower grows up to 15 cm in summer due to thermal expansion.",
        "Bananas are berries, but strawberries are not.",
        "There are more stars in the universe than grains of sand on all Earth's beaches.",
        "A group of flamingos is called a flamboyance.",
        "Cleopatra lived closer in time to the Moon landing than to the construction of the Great Pyramid.",
        "Sharks are older than trees — they predate them by around 50 million years.",
        "The shortest war in history lasted 38–45 minutes (Anglo-Zanzibar War, 1896).",
        "Wombat poop is cube-shaped.",
        "A bolt of lightning is 5× hotter than the surface of the Sun.",
        "The longest recorded flight of a chicken is 13 seconds.",
        "There are more possible chess games than atoms in the observable universe.",
        "Crows can recognise and remember human faces.",
    ];
    FACTS[thread_rng().gen_range(0..FACTS.len())]
}

fn random_joke() -> &'static str {
    const JOKES: &[&str] = &[
        "Why do programmers prefer dark mode? Because light attracts bugs.",
        "Why did the function break up with the loop? It just kept going in circles.",
        "A SQL query walks into a bar, walks up to two tables and asks... 'Can I join you?'",
        "There are 10 types of people: those who understand binary and those who don't.",
        "Why do Java developers wear glasses? Because they don't C#.",
        "A byte walks into a bar looking rough. Bartender: 'What's wrong?' Byte: 'Bit error.'",
        "Why is assembly code so hard to understand? It's all gibberish to me — no, really, it literally is.",
        "How many programmers does it take to change a light bulb? None, that's a hardware problem.",
        "I told my wife she should embrace her mistakes. She hugged me.",
        "Debugging is like being the detective in a crime movie where you're also the murderer.",
        "It's not a bug — it's an undocumented feature.",
        "Why did the developer quit? They didn't get arrays.",
        "I have a joke about UDP, but I'm not sure you'll get it.",
        "I have a joke about TCP, but I'll keep sending it until you acknowledge it.",
        "Why did the Rust developer cross the road? To avoid garbage collection.",
    ];
    JOKES[thread_rng().gen_range(0..JOKES.len())]
}
