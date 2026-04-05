# Update Types

All Telegram events flow through `stream_updates()` as variants of the `Update` enum. Every variant is strongly typed — no raw JSON or untagged maps.

```rust
use layer_client::update::Update;

let mut updates = client.stream_updates();
while let Some(update) = updates.next().await {
    match update {
        Update::NewMessage(msg)       => { /* new message arrived */ }
        Update::MessageEdited(msg)    => { /* message was edited */ }
        Update::MessageDeleted(del)   => { /* message(s) were deleted */ }
        Update::CallbackQuery(cb)     => { /* inline button pressed */ }
        Update::InlineQuery(iq)       => { /* @bot inline query */ }
        Update::InlineSend(is)        => { /* inline result chosen */ }
        Update::ChatAction(action)    => { /* user typing / uploading */ }
        Update::UserStatus(status)    => { /* contact online status */ }
        Update::Raw(raw)              => { /* unrecognised update */ }
        _ => {}   // required: Update is #[non_exhaustive]
    }
}
```

> **Note:** As of v0.4.6, `Update` is `#[non_exhaustive]`. Your match arms **must** include a `_ => {}` fallback or the code will fail to compile when new variants are added.

---

## NewMessage

Fires for every new message the account receives in any chat.

```rust
Update::NewMessage(msg) => {
    if msg.outgoing() { return; }  // skip messages you sent

    let text    = msg.text().unwrap_or("");
    let msg_id  = msg.id();
    let peer    = msg.peer_id();   // the chat it arrived in
    let sender  = msg.sender_id(); // who sent it

    println!("[{msg_id}] {text}");
}
```

See [IncomingMessage](./incoming-message.md) for the full list of accessors.

---

## MessageEdited

Same structure as `NewMessage` — carries the new version of the edited message.

```rust
Update::MessageEdited(msg) => {
    println!("Edited [{id}]: {text}",
        id   = msg.id(),
        text = msg.text().unwrap_or(""),
    );
    if let Some(when) = msg.edit_date_utc() {
        println!("  Edited at: {when}");
    }
}
```

---

## MessageDeleted

Contains only the message IDs, not the content (which is gone).

```rust
Update::MessageDeleted(del) => {
    println!("Deleted IDs: {:?}", del.messages());

    // For channel deletions, channel_id is set
    if let Some(ch_id) = del.channel_id() {
        println!("  In channel: {ch_id}");
    }
}
```

---

## CallbackQuery

Fires when a user presses an inline keyboard button.

```rust
Update::CallbackQuery(cb) => {
    let data  = cb.data().unwrap_or("");
    let qid   = cb.query_id;

    match data {
        "yes" => {
            // edit the message, then acknowledge
            client.edit_message(peer, cb.msg_id, "You said yes!").await?;
            client.answer_callback_query(qid, None, false).await?;
        }
        "no" => {
            cb.answer(&client, "Cancelled.").await?;
        }
        _ => {
            client.answer_callback_query(qid, Some("Unknown"), false).await?;
        }
    }
}
```

See [Callback Queries](./callbacks.md) for full reference.

---

## InlineQuery

Fires when a user types `@yourbot <query>` in any chat.

```rust
Update::InlineQuery(iq) => {
    let q   = iq.query();
    let qid = iq.query_id;

    let results = build_results(q);

    client.answer_inline_query(
        qid,
        results,
        300,   // cache seconds
        false, // is_personal
        None,  // next_offset
    ).await?;
}
```

See [Inline Mode](./inline-mode.md) for result builders.

---

## InlineSend

Fires when the user actually sends a chosen inline result.

```rust
Update::InlineSend(is) => {
    let result_id   = is.result_id();
    let query       = is.query();
    let inline_msg  = is.message_id(); // Option — present only if inline_feedback is on

    println!("User sent inline result '{result_id}' for query '{query}'");
}
```

To edit the sent inline message:

```rust
if let Some(inline_msg_id) = is.message_id() {
    client.edit_inline_message(
        inline_msg_id,
        "Updated content!",
    ).await?;
}
```

---

## ChatAction — New in v0.4.6

Fires when a user starts or stops typing, uploading, recording, etc. in a chat the account is in.

```rust
Update::ChatAction(action) => {
    let user   = action.user_id();    // Option<i64>
    let peer   = action.peer();       // the chat
    let action = action.action();     // tl::enums::SendMessageAction

    match action {
        tl::enums::SendMessageAction::SendMessageTypingAction => {
            println!("user {:?} is typing in {:?}", user, peer);
        }
        tl::enums::SendMessageAction::SendMessageUploadPhotoAction(_) => {
            println!("user {:?} is uploading a photo", user);
        }
        tl::enums::SendMessageAction::SendMessageRecordAudioAction => {
            println!("user {:?} is recording audio", user);
        }
        tl::enums::SendMessageAction::SendMessageCancelAction => {
            println!("user {:?} stopped", user);
        }
        _ => {}
    }
}
```

---

## UserStatus — New in v0.4.6

Fires when a contact's online/offline status changes. Only received for contacts or people in mutual chats (depending on their privacy settings).

```rust
Update::UserStatus(status) => {
    let user_id = status.user_id();    // i64
    let online  = status.status();     // tl::enums::UserStatus

    match online {
        tl::enums::UserStatus::UserStatusOnline(s) => {
            println!("user {user_id} went online (expires {})", s.expires);
        }
        tl::enums::UserStatus::UserStatusOffline(s) => {
            println!("user {user_id} went offline (was online {})", s.was_online);
        }
        tl::enums::UserStatus::UserStatusRecently => {
            println!("user {user_id}: seen recently");
        }
        tl::enums::UserStatus::UserStatusLastWeek(_) => {
            println!("user {user_id}: seen last week");
        }
        tl::enums::UserStatus::UserStatusLastMonth(_) => {
            println!("user {user_id}: seen last month");
        }
        _ => {}
    }
}
```

---

## Raw

Any update that doesn't map to a named variant is passed through as `Update::Raw`:

```rust
Update::Raw(raw) => {
    // raw.constructor_id() — the TL constructor ID (u32)
    // raw.bytes()          — the raw serialised bytes
    println!("Unhandled update: 0x{:08x}", raw.constructor_id());
}
```

Use this as an escape hatch to handle updates that `layer-client` doesn't yet have a typed variant for.

---

## Concurrent handling

Spawn each update in its own Tokio task to prevent one slow handler from blocking others:

```rust
use std::sync::Arc;

let client = Arc::new(client);
let mut updates = client.stream_updates();

while let Some(update) = updates.next().await {
    let client = client.clone();
    tokio::spawn(async move {
        if let Err(e) = handle(update, &client).await {
            eprintln!("Handler error: {e}");
        }
    });
}

async fn handle(
    update: Update,
    client: &Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match update {
        Update::NewMessage(msg) if !msg.outgoing() => {
            // …
        }
        _ => {}
    }
    Ok(())
}
```
