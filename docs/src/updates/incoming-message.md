# IncomingMessage

`IncomingMessage` wraps `tl::enums::Message` and provides convenient typed accessors. It's the type carried by `Update::NewMessage` and `Update::MessageEdited`.

## All accessors

| Method | Returns | Description |
|---|---|---|
| `id()` | `i32` | Unique message ID within the chat |
| `text()` | `Option<&str>` | Plain text content |
| `peer_id()` | `Option<&Peer>` | The chat this message belongs to |
| `sender_id()` | `Option<&Peer>` | Who sent it (None for anonymous channels) |
| `outgoing()` | `bool` | Sent by the logged-in account |
| `date()` | `i32` | Unix timestamp of creation |
| `date_utc()` | `Option<DateTime<Utc>>` | Parsed chrono datetime |
| `edit_date()` | `Option<i32>` | Unix timestamp of last edit |
| `edit_date_utc()` | `Option<DateTime<Utc>>` | Parsed edit datetime |
| `mentioned()` | `bool` | The account was @mentioned |
| `silent()` | `bool` | Sent without notification |
| `post()` | `bool` | Posted by a channel (not a user) |
| `pinned()` | `bool` | This is a pin service message |
| `noforwards()` | `bool` | Cannot be forwarded or screenshotted |
| `reply_to_message_id()` | `Option<i32>` | ID of the replied-to message |
| `forward_count()` | `Option<i32>` | Times this message was forwarded |
| `view_count()` | `Option<i32>` | View count (channels only) |
| `reply_count()` | `Option<i32>` | Comment count |
| `grouped_id()` | `Option<i64>` | Album group ID |
| `media()` | `Option<&MessageMedia>` | Attached media |
| `entities()` | `Option<&Vec<MessageEntity>>` | Text formatting regions |
| `reply_markup()` | `Option<&ReplyMarkup>` | Inline keyboard |
| `forward_header()` | `Option<&MessageFwdHeader>` | Forward origin info |
| `raw` | `tl::enums::Message` | The underlying raw TL type |

---

## Getting sender user ID

```rust
fn user_id(msg: &IncomingMessage) -> Option<i64> {
    match msg.sender_id()? {
        tl::enums::Peer::User(u) => Some(u.user_id),
        _ => None,
    }
}
```

## Determining chat type

```rust
match msg.peer_id() {
    Some(tl::enums::Peer::User(u))    => {
        println!("Private DM with user {}", u.user_id);
    }
    Some(tl::enums::Peer::Chat(c))    => {
        println!("Basic group {}", c.chat_id);
    }
    Some(tl::enums::Peer::Channel(c)) => {
        println!("Channel or supergroup {}", c.channel_id);
    }
    None => {
        println!("Unknown peer");
    }
}
```

## Accessing media

```rust
if let Some(media) = msg.media() {
    match media {
        tl::enums::MessageMedia::Photo(p)     => println!("📷 Photo"),
        tl::enums::MessageMedia::Document(d)  => println!("📎 Document"),
        tl::enums::MessageMedia::Geo(g)       => println!("📍 Location"),
        tl::enums::MessageMedia::Contact(c)   => println!("👤 Contact"),
        tl::enums::MessageMedia::Poll(p)      => println!("📊 Poll"),
        tl::enums::MessageMedia::WebPage(w)   => println!("🔗 Web preview"),
        tl::enums::MessageMedia::Sticker(s)   => println!("🩷 Sticker"),
        tl::enums::MessageMedia::Dice(d)      => println!("🎲 Dice"),
        tl::enums::MessageMedia::Game(g)      => println!("🎮 Game"),
        _ => println!("Other media"),
    }
}
```

## Accessing entities

```rust
if let Some(entities) = msg.entities() {
    for entity in entities {
        match entity {
            tl::enums::MessageEntity::Bold(e)   => {
                let bold_text = &msg.text().unwrap_or("")[e.offset as usize..][..e.length as usize];
                println!("Bold: {bold_text}");
            }
            tl::enums::MessageEntity::BotCommand(e) => {
                let cmd = &msg.text().unwrap_or("")[e.offset as usize..][..e.length as usize];
                println!("Command: {cmd}");
            }
            tl::enums::MessageEntity::Url(e) => {
                let url = &msg.text().unwrap_or("")[e.offset as usize..][..e.length as usize];
                println!("URL: {url}");
            }
            _ => {}
        }
    }
}
```

## Forward info

```rust
if let Some(fwd) = msg.forward_header() {
    if let tl::enums::MessageFwdHeader::MessageFwdHeader(h) = fwd {
        println!("Forwarded at: {}", h.date);
        if let Some(tl::enums::Peer::Channel(c)) = &h.from_id {
            println!("From channel: {}", c.channel_id);
        }
    }
}
```

## Reply to previous message

```rust
// Quick reply with text
msg.reply(&mut client, "Got it!").await?;

// Reply with full InputMessage (formatted, keyboard, etc.)
if let Some(peer) = msg.peer_id() {
    let (t, e) = parse_markdown("**Acknowledged** ✅");
    client.send_message_to_peer_ex(peer.clone(), &InputMessage::text(t)
        .entities(e)
        .reply_to(Some(msg.id()))
    ).await?;
}
```

## Accessing raw TL fields

For fields not exposed by accessors, use `.raw` directly:

```rust
if let tl::enums::Message::Message(raw) = &msg.raw {
    // Layer 223 additions
    println!("from_rank: {:?}",             raw.from_rank);
    println!("suggested_post: {:?}",        raw.suggested_post);
    println!("paid_message_stars: {:?}",    raw.paid_message_stars);
    println!("schedule_repeat_period: {:?}",raw.schedule_repeat_period);
    println!("summary_from_language: {:?}", raw.summary_from_language);

    // Standard fields
    println!("grouped_id: {:?}",            raw.grouped_id);
    println!("restriction_reason: {:?}",    raw.restriction_reason);
    println!("ttl_period: {:?}",            raw.ttl_period);
    println!("effect: {:?}",                raw.effect);
    println!("factcheck: {:?}",             raw.factcheck);
}
```
