# Inline Mode

Inline mode lets users type `@yourbot query` in any chat — the bot responds with a list of results the user can tap to send.

## Enable inline mode

In [@BotFather](https://t.me/BotFather):
1. Send `/mybots` → select your bot
2. **Bot Settings** → **Inline Mode** → **Turn on**

## Handling inline queries

```rust
Update::InlineQuery(iq) => {
    let query  = iq.query().to_string();
    let qid    = iq.query_id;

    let results = build_results(&query);

    client.answer_inline_query(
        qid,
        results,
        300,   // cache_time in seconds
        false, // is_personal (true = don't share cache across users)
        None,  // next_offset (for pagination)
    ).await?;
}
```

## Building results

### Article result (text message)

```rust
fn article(id: &str, title: &str, description: &str, text: &str)
    -> tl::enums::InputBotInlineResult
{
    tl::enums::InputBotInlineResult::InputBotInlineResult(
        tl::types::InputBotInlineResult {
            id:          id.into(),
            r#type:      "article".into(),
            title:       Some(title.into()),
            description: Some(description.into()),
            url:         None,
            thumb:       None,
            content:     None,
            send_message: tl::enums::InputBotInlineMessage::Text(
                tl::types::InputBotInlineMessageText {
                    no_webpage:   false,
                    invert_media: false,
                    message:      text.into(),
                    entities:     None,
                    reply_markup: None,
                }
            ),
        }
    )
}
```

### Multiple results for a query

```rust
fn build_results(q: &str) -> Vec<tl::enums::InputBotInlineResult> {
    if q.is_empty() {
        // Default suggestions when query is blank
        return vec![
            article("time", "🕐 Current Time",
                &chrono::Utc::now().format("%H:%M UTC").to_string(),
                &chrono::Utc::now().to_rfc2822()),
            article("help", "📖 Help", "See all commands", "/help"),
        ];
    }

    vec![
        article("u", &format!("UPPER: {}", q.to_uppercase()),
            "Uppercase version", &q.to_uppercase()),
        article("l", &format!("lower: {}", q.to_lowercase()),
            "Lowercase version", &q.to_lowercase()),
        article("r", &format!("Reversed"),
            "Reversed text", &q.chars().rev().collect::<String>()),
        article("c", "📊 Character count",
            &format!("{} chars, {} words", q.len(), q.split_whitespace().count()),
            &format!("{} characters • {} words • {} lines",
                q.chars().count(), q.split_whitespace().count(), q.lines().count())),
    ]
}
```

## InlineQuery fields

| Field / Method | Type | Description |
|---|---|---|
| `iq.query()` | `&str` | The text the user typed |
| `iq.query_id` | `i64` | Unique ID for this query |
| `iq.offset()` | `&str` | Pagination offset |
| `iq.peer_type` | varies | Type of chat where query was issued |

## InlineSend — when a result is chosen

```rust
Update::InlineSend(is) => {
    // Fired when the user picks one of your results
    println!("Result chosen: {}", is.id());
    // Use this for logging, stats, or post-send actions
}
```

## Pagination

For large result sets, implement pagination using `next_offset`:

```rust
let page: usize = iq.offset().parse().unwrap_or(0);
let items = get_items_page(page, 10);
let next  = if items.len() == 10 { Some(format!("{}", page + 1)) } else { None };

client.answer_inline_query(
    iq.query_id,
    items,
    60,
    false,
    next.as_deref(),
).await?;
```
