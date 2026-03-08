# Inline Keyboards

Inline keyboards appear as button rows attached below messages. They trigger `Update::CallbackQuery` when pressed.

## Helper functions (recommended pattern)

```rust
use layer_tl_types as tl;

fn inline_kb(rows: Vec<Vec<tl::enums::KeyboardButton>>) -> tl::enums::ReplyMarkup {
    tl::enums::ReplyMarkup::ReplyInlineMarkup(tl::types::ReplyInlineMarkup {
        rows: rows.into_iter().map(|buttons|
            tl::enums::KeyboardButtonRow::KeyboardButtonRow(
                tl::types::KeyboardButtonRow { buttons }
            )
        ).collect(),
    })
}

fn btn_cb(text: &str, data: &str) -> tl::enums::KeyboardButton {
    tl::enums::KeyboardButton::Callback(tl::types::KeyboardButtonCallback {
        requires_password: false,
        style:             None,
        text:              text.into(),
        data:              data.as_bytes().to_vec(),
    })
}

fn btn_url(text: &str, url: &str) -> tl::enums::KeyboardButton {
    tl::enums::KeyboardButton::Url(tl::types::KeyboardButtonUrl {
        style: None,
        text:  text.into(),
        url:   url.into(),
    })
}
```

## Send with keyboard

```rust
let kb = inline_kb(vec![
    vec![btn_cb("✅ Yes", "confirm:yes"), btn_cb("❌ No", "confirm:no")],
    vec![btn_url("🌐 Docs", "https://github.com/ankit-chaubey/layer")],
]);

let (text, entities) = parse_markdown("**Do you want to proceed?**");
let msg = InputMessage::text(text)
    .entities(entities)
    .reply_markup(kb);

client.send_message_to_peer_ex(peer, &msg).await?;
```

## All button types

| Type | Constructor | Description |
|---|---|---|
| Callback | `KeyboardButtonCallback` | Triggers `CallbackQuery` with custom data |
| URL | `KeyboardButtonUrl` | Opens a URL in the browser |
| Web App | `KeyboardButtonSimpleWebView` | Opens a Telegram Web App |
| Switch Inline | `KeyboardButtonSwitchInline` | Opens inline mode with a query |
| Request Phone | `KeyboardButtonRequestPhone` | Requests the user's phone number |
| Request Location | `KeyboardButtonRequestGeoLocation` | Requests location |
| Request Poll | `KeyboardButtonRequestPoll` | Opens poll creator |
| Request Peer | `KeyboardButtonRequestPeer` | Requests peer selection |
| Game | `KeyboardButtonGame` | Opens a Telegram game |
| Buy | `KeyboardButtonBuy` | Purchase button for payments |
| Copy | `KeyboardButtonCopy` | Copies text to clipboard |

### Switch Inline button

Opens the bot's inline mode in the current or another chat:

```rust
tl::enums::KeyboardButton::SwitchInline(tl::types::KeyboardButtonSwitchInline {
    same_peer:  false, // false = let user pick any chat
    text:       "🔍 Search with me".into(),
    query:      "default query".into(),
    peer_types: None,
})
```

### Web App button

```rust
tl::enums::KeyboardButton::SimpleWebView(tl::types::KeyboardButtonSimpleWebView {
    text: "Open App".into(),
    url:  "https://myapp.example.com".into(),
})
```

## Reply keyboard (replaces user's keyboard)

```rust
let reply_kb = tl::enums::ReplyMarkup::ReplyKeyboardMarkup(
    tl::types::ReplyKeyboardMarkup {
        resize:      true,       // shrink to fit buttons
        single_use:  true,       // hide after one tap
        selective:   false,      // show to everyone
        persistent:  false,      // don't keep after message
        placeholder: Some("Choose an option…".into()),
        rows: vec![
            tl::enums::KeyboardButtonRow::KeyboardButtonRow(
                tl::types::KeyboardButtonRow {
                    buttons: vec![
                        tl::enums::KeyboardButton::KeyboardButton(
                            tl::types::KeyboardButton { text: "🍕 Pizza".into() }
                        ),
                        tl::enums::KeyboardButton::KeyboardButton(
                            tl::types::KeyboardButton { text: "🍔 Burger".into() }
                        ),
                    ]
                }
            ),
            tl::enums::KeyboardButtonRow::KeyboardButtonRow(
                tl::types::KeyboardButtonRow {
                    buttons: vec![
                        tl::enums::KeyboardButton::KeyboardButton(
                            tl::types::KeyboardButton { text: "❌ Cancel".into() }
                        ),
                    ]
                }
            ),
        ],
    }
);
```

The user's choices arrive as plain text `NewMessage` updates.

## Remove keyboard

```rust
let remove = tl::enums::ReplyMarkup::ReplyKeyboardHide(
    tl::types::ReplyKeyboardHide { selective: false }
);
let msg = InputMessage::text("Keyboard removed.").reply_markup(remove);
```

## Button data format

Telegram limits callback button data to **64 bytes**. Use compact, parseable formats:

```rust
// Good — structured, compact
"vote:yes"
"page:3"
"item:42:delete"
"menu:settings:notifications"

// Bad — verbose
"user_clicked_the_settings_button"
```
