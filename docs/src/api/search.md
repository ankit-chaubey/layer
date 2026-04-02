# Search

`layer-client` provides two search builders: `SearchBuilder` for searching within a single peer, and `GlobalSearchBuilder` for searching across all dialogs at once.

## SearchBuilder — per-peer search

```rust
let results = client
    .search("@somechannel", "rust async")
    .limit(20)
    .fetch()
    .await?;

for msg in results {
    if let tl::enums::Message::Message(m) = msg {
        println!("[{}] {}", m.id, m.message);
    }
}
```

### All builder methods

| Method | Default | Description |
|---|---|---|
| `.limit(n)` | 20 | Maximum number of results |
| `.min_date(ts)` | — | Only messages after this unix timestamp |
| `.max_date(ts)` | — | Only messages before this unix timestamp |
| `.offset_id(id)` | 0 | Start from this message ID (for pagination) |
| `.add_offset(n)` | 0 | Skip this many results from the start |
| `.max_id(id)` | 0 | Upper bound message ID |
| `.min_id(id)` | 0 | Lower bound message ID |
| `.from_peer(peer)` | — | Only messages from this sender |
| `.top_msg_id(id)` | — | Restrict to a specific forum topic |
| `.filter(f)` | Empty | Filter by media type (see below) |
| `.fetch()` | — | Execute and return `Vec<tl::enums::Message>` |

### Date range example

```rust
use chrono::{Utc, Duration};

let one_week_ago = (Utc::now() - Duration::days(7)).timestamp() as i32;

let results = client
    .search("@mychannel", "error")
    .min_date(one_week_ago)
    .limit(50)
    .fetch()
    .await?;
```

### Filter by media type

```rust
use layer_tl_types::{enums, types};

// Only photo messages
let photos = client
    .search("@channel", "")
    .filter(enums::MessagesFilter::InputMessagesFilterPhotos)
    .limit(30)
    .fetch()
    .await?;

// Only documents
let docs = client
    .search("@channel", "report")
    .filter(enums::MessagesFilter::InputMessagesFilterDocument)
    .limit(30)
    .fetch()
    .await?;
```

### Common MessagesFilter values

| Filter | Matches |
|---|---|
| `InputMessagesFilterEmpty` | All messages (default) |
| `InputMessagesFilterPhotos` | Photos |
| `InputMessagesFilterVideo` | Videos |
| `InputMessagesFilterDocument` | Documents |
| `InputMessagesFilterAudio` | Audio files |
| `InputMessagesFilterVoice` | Voice messages |
| `InputMessagesFilterUrl` | Messages containing URLs |
| `InputMessagesFilterMyMentions` | Messages where you were mentioned |
| `InputMessagesFilterPinned` | Pinned messages |

### Search from a specific sender

```rust
let sender = client.resolve_to_input_peer("@alice").await?;

let results = client
    .search("@groupchat", "hello")
    .from_peer(sender)
    .fetch()
    .await?;
```

### Pagination

```rust
let mut offset_id = 0;
let page_size = 50;

loop {
    let page = client
        .search("@channel", "query")
        .offset_id(offset_id)
        .limit(page_size)
        .fetch()
        .await?;

    if page.is_empty() {
        break;
    }

    for msg in &page {
        // process…
    }

    // Get the lowest message ID for the next page
    offset_id = page.iter()
        .filter_map(|m| if let tl::enums::Message::Message(m) = m { Some(m.id) } else { None })
        .min()
        .unwrap_or(0);
}
```

---

## GlobalSearchBuilder — search everywhere

```rust
let results = client
    .search_global_builder("layer rust")
    .limit(10)
    .fetch()
    .await?;

for msg in results {
    if let tl::enums::Message::Message(m) = msg {
        println!("[peer {:?}] [{}] {}", m.peer_id, m.id, m.message);
    }
}
```

### All global builder methods

| Method | Default | Description |
|---|---|---|
| `.limit(n)` | 20 | Maximum number of results |
| `.min_date(ts)` | — | Only messages after this timestamp |
| `.max_date(ts)` | — | Only messages before this timestamp |
| `.offset_rate(r)` | 0 | Pagination: rate value from last result |
| `.offset_id(id)` | 0 | Pagination: message ID from last result |
| `.folder_id(id)` | — | Restrict to a specific folder |
| `.broadcasts_only(v)` | false | Only search channels |
| `.groups_only(v)` | false | Only search groups |
| `.users_only(v)` | false | Only search private chats |
| `.filter(f)` | Empty | Filter by media type |
| `.fetch()` | — | Execute and return results |

### Filter by dialog type

```rust
// Search only in channels
let results = client
    .search_global_builder("announcement")
    .broadcasts_only(true)
    .limit(20)
    .fetch()
    .await?;

// Search only in groups
let results = client
    .search_global_builder("discussion")
    .groups_only(true)
    .fetch()
    .await?;
```

### Convenience one-liner

For a quick global search without the builder:

```rust
let results = client.search_global("rust async", 10).await?;
```

---

## Simple per-peer search (no builder)

For basic cases without date filters:

```rust
let results = client.search_messages(peer, "query", 20).await?;
```
