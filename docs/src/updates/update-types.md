# Update Types

All Telegram events flow through `stream_updates()` as variants of the `Update` enum. Here is every variant, what it carries, and how to handle it.

```rust
use layer_client::update::Update;

let mut updates = client.stream_updates();
while let Some(update) = updates.next().await {
    match update {
        Update::NewMessage(msg)     => { /* ... */ }
        Update::MessageEdited(msg)  => { /* ... */ }
        Update::MessageDeleted(del) => { /* ... */ }
        Update::CallbackQuery(cb)   => { /* ... */ }
        Update::InlineQuery(iq)     => { /* ... */ }
        Update::InlineSend(is)      => { /* ... */ }
        Update::Raw(raw)            => { /* ... */ }
        _ => {}
    }
}
```

---

## NewMessage

Fires for every new message received in any chat the account participates in.

```rust
Update::NewMessage(msg) => {
    // Filter out your own sent messages
    if msg.outgoing() { return; }

    let text     = msg.text().unwrap_or("");
    let msg_id   = msg.id();
    let date     = msg.date_utc();  // chrono::DateTime<Utc>
    let is_post  = msg.post();      // from a channel
    let has_media = msg.media().is_some();

    println!("[{msg_id}] {text}");
}
```

**Key accessors on `IncomingMessage`:**

| Method | Returns | Notes |
|---|---|---|
| `id()` | `i32` | Unique message ID within the chat |
| `text()` | `Option<&str>` | Plain text content |
| `peer_id()` | `Option<&Peer>` | The chat it was sent in |
| `sender_id()` | `Option<&Peer>` | Who sent it |
| `outgoing()` | `bool` | Sent by the logged-in account |
| `date()` | `i32` | Unix timestamp |
| `date_utc()` | `Option<DateTime<Utc>>` | Parsed chrono datetime |
| `edit_date()` | `Option<i32>` | When last edited |
| `media()` | `Option<&MessageMedia>` | Attached media |
| `entities()` | `Option<&Vec<MessageEntity>>` | Formatting regions |
| `mentioned()` | `bool` | Account was @mentioned |
| `silent()` | `bool` | Sent without notification |
| `pinned()` | `bool` | A pin notification |
| `post()` | `bool` | From a channel |
| `noforwards()` | `bool` | Cannot be forwarded |
| `reply_to_message_id()` | `Option<i32>` | ID of replied-to message |
| `reply_markup()` | `Option<&ReplyMarkup>` | Keyboard attached to this message |
| `forward_count()` | `Option<i32>` | How many times forwarded |
| `view_count()` | `Option<i32>` | View count (channels) |
| `reply_count()` | `Option<i32>` | Comment count |
| `grouped_id()` | `Option<i64>` | Album group ID |
| `forward_header()` | `Option<&MessageFwdHeader>` | Forward origin info |

---

## MessageEdited

Same structure as `NewMessage` — carries the updated version of the message.

```rust
Update::MessageEdited(msg) => {
    println!("Message {} was edited: {}", msg.id(), msg.text().unwrap_or(""));
    if let Some(edit_time) = msg.edit_date_utc() {
        println!("Edited at: {edit_time}");
    }
}
```

---

## MessageDeleted

Contains only message IDs — content is gone by the time this fires.

```rust
Update::MessageDeleted(del) => {
    println!("Deleted {} messages", del.messages().len());
    println!("IDs: {:?}", del.messages());

    // For channel deletions, the channel ID is available
    if let Some(ch_id) = del.channel_id() {
        println!("In channel: {ch_id}");
    }
}
```

---

## CallbackQuery

Fired when a user presses an inline keyboard button on a bot message.

```rust
Update::CallbackQuery(cb) => {
    let data    = cb.data().unwrap_or("");
    let qid     = cb.query_id;
    let from    = cb.sender_id();
    let msg_id  = cb.msg_id;

    match data {
        "action:confirm" => {
            // answer() shows a brief toast to the user
            cb.answer(&client, "✅ Confirmed!").await?;
        }
        "action:cancel" => {
            // answer_alert() shows a modal popup
            cb.answer_alert(&client, "❌ Cancelled").await?;
        }
        _ => {
            // Must always answer — otherwise spinner shows forever
            client.answer_callback_query(qid, None, false).await?;
        }
    }
}
```

> **WARNING:** You **must** call `answer_callback_query` for every `CallbackQuery`. If you don't, the button shows a loading spinner to the user indefinitely.

---

## InlineQuery

Fired when a user types `@yourbot something` in any chat.

```rust
Update::InlineQuery(iq) => {
    let query  = iq.query();   // the typed text
    let qid    = iq.query_id;
    let offset = iq.offset();  // for pagination

    let results = vec![
        make_article("1", "Result title", "Result text"),
    ];

    // cache_time = seconds to cache the results (0 = no cache)
    // is_personal = true if results differ per user
    // next_offset = Some("page2") for pagination
    client.answer_inline_query(qid, results, 300, false, None).await?;
}
```

---

## InlineSend

Fired when the user selects one of your inline results and it gets sent.

```rust
Update::InlineSend(is) => {
    println!("User chose result id: {}", is.id());
    // Useful for analytics or follow-up actions
}
```

---

## Raw

Any TL update variant not mapped to one of the above. Carries the constructor ID for identification.

```rust
Update::Raw(raw) => {
    println!("Unhandled update: 0x{:08x}", raw.constructor_id);

    // You can decode it manually using the TL types
    // if you know the constructor:
    // let upd: tl::enums::Update = ...;
}
```

Use this as a catch-all for new update types as the Telegram API evolves, or to handle specialized updates like `updateBotChatInviteRequester`, `updateBotStopped`, etc.

---

## Concurrent handling pattern

```rust
use std::sync::Arc;

let client = Arc::new(client);
let mut updates = client.stream_updates();

while let Some(update) = updates.next().await {
    let c = client.clone();
    tokio::spawn(async move {
        if let Err(e) = handle(update, &c).await {
            eprintln!("Error: {e}");
        }
    });
}
```

This ensures slow handlers don't block the receive loop.
