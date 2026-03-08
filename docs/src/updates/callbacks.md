# Callback Queries

Callback queries are fired when users press inline keyboard buttons on bot messages.

## Full handling example

```rust
Update::CallbackQuery(cb) => {
    let data   = cb.data().unwrap_or("").to_string();
    let qid    = cb.query_id;

    // Parse structured data
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    match parts.as_slice() {
        ["vote", choice] => {
            record_vote(choice);
            cb.answer(&client, &format!("Voted: {choice}")).await?;
        }
        ["page", n] => {
            let page: usize = n.parse().unwrap_or(0);
            // Edit the original message to show new page
            client.edit_message(
                peer_from_cb(&cb),
                cb.msg_id,
                &format_page(page),
            ).await?;
            client.answer_callback_query(qid, None, false).await?;
        }
        ["confirm"] => {
            cb.answer_alert(&client, "Are you sure? This is permanent.").await?;
        }
        _ => {
            client.answer_callback_query(qid, Some("Unknown action"), false).await?;
        }
    }
}
```

## CallbackQuery fields

| Field / Method | Type | Description |
|---|---|---|
| `cb.query_id` | `i64` | Unique query ID — must be answered |
| `cb.msg_id` | `i32` | ID of the message that has the button |
| `cb.data()` | `Option<&str>` | The data string set in the button |
| `cb.sender_id()` | `Option<&Peer>` | Who pressed the button |
| `cb.answer(client, text)` | `async` | Toast notification to user |
| `cb.answer_alert(client, text)` | `async` | Modal alert popup to user |

## answer vs answer_alert

```rust
// Toast (brief notification at bottom of screen)
cb.answer(&client, "✅ Done!").await?;

// Alert popup (requires user to dismiss)
cb.answer_alert(&client, "⚠️ This will delete everything!").await?;

// Silent acknowledge (no visible notification)
client.answer_callback_query(cb.query_id, None, false).await?;
```

## Editing message after a button press

A common pattern is updating the message content when a button is pressed:

```rust
Update::CallbackQuery(cb) => {
    if cb.data() == Some("next_page") {
        // Edit the message text
        client.edit_message(
            tl::enums::Peer::User(/* resolved peer */),
            cb.msg_id,
            "Updated content after button press",
        ).await?;

        // Always acknowledge
        client.answer_callback_query(cb.query_id, None, false).await?;
    }
}
```

## Button data format tips

Keep button data under 64 bytes (Telegram's limit). For structured data use short prefixes:

```rust
// Good — compact, parseable
"vote:yes"
"page:3"
"item:42:delete"

// Bad — too verbose
"user_wants_to_vote_for_option_yes"
```
