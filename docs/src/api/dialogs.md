# Dialogs & Message History

## List dialogs (conversations)

```rust
// Fetch the 50 most recent dialogs
let dialogs = client.get_dialogs(50).await?;

for dialog in &dialogs {
    println!(
        "[{}] {} unread — top msg {}",
        dialog.title(),
        dialog.unread_count(),
        dialog.top_message(),
    );
}
```

### Dialog fields

| Method | Returns | Description |
|---|---|---|
| `dialog.title()` | `String` | Name of the chat/channel/user |
| `dialog.peer()` | `Option<&Peer>` | The peer identifier |
| `dialog.unread_count()` | `i32` | Number of unread messages |
| `dialog.top_message()` | `i32` | ID of the last message |

---

## Paginating dialogs (all)

For iterating **all** dialogs beyond the first page:

```rust
let mut iter = client.iter_dialogs();

while let Some(dialog) = iter.next(&client).await? {
    println!("{} — {} unread", dialog.title(), dialog.unread_count());
}
```

The iterator automatically requests more pages from Telegram as needed.

---

## Paginating messages

```rust
let peer = client.resolve_peer("@somechannel").await?;
let mut iter = client.iter_messages(peer);

let mut count = 0;
while let Some(msg) = iter.next(&client).await? {
    println!("[{}] {}", msg.id(), msg.text().unwrap_or("(media)"));
    count += 1;
    if count >= 500 { break; }
}
```

---

## Get message history (basic)

```rust
// Newest 50 messages
let messages = client.get_messages(peer, 50, 0).await?;

// Next page: pass the last message's ID as offset
let last_id = messages.last()
    .and_then(|m| if let tl::enums::Message::Message(m) = m { Some(m.id) } else { None })
    .unwrap_or(0);

let older = client.get_messages(peer, 50, last_id).await?;
```

---

## Scheduled messages

```rust
// Fetch messages scheduled to be sent
let scheduled = client.get_scheduled_messages(peer).await?;

for msg in &scheduled {
    if let tl::enums::Message::Message(m) = msg {
        println!("Scheduled: {} at {}", m.message, m.date);
    }
}

// Delete a scheduled message
client.delete_scheduled_messages(peer, vec![msg_id]).await?;
```

---

## Search within a chat

```rust
let results = client.search_messages(
    peer,
    "error log",  // search query
    20,           // limit
).await?;

for msg in &results {
    if let tl::enums::Message::Message(m) = msg {
        println!("[{}] {}", m.id, m.message);
    }
}
```

## Global search

```rust
let results = client.search_global("layer rust telegram", 10).await?;
```

---

## Mark as read / unread management

```rust
// Mark all messages in a chat as read
client.mark_as_read(peer).await?;

// Clear all @mentions in a group
client.clear_mentions(peer).await?;
```

---

## Get pinned message

```rust
if let Some(msg) = client.get_pinned_message(peer).await? {
    if let tl::enums::Message::Message(m) = msg {
        println!("Pinned: {}", m.message);
    }
}
```
