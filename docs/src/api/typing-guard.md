# Typing Guard

`TypingGuard` is a RAII wrapper that keeps a "typing…" or "uploading…" indicator alive for the duration of an operation and automatically cancels it on drop.

## Basic usage

```rust
use layer_client::TypingGuard;

async fn process_message(client: &Client, peer: tl::enums::Peer) {
    // Start "typing…" — cancelled automatically when guard drops
    let _guard = TypingGuard::typing(client, peer).await;

    // Do your slow work here
    let response = compute_heavy_reply().await;

    // _guard drops here → typing indicator cancelled automatically
    client.send_message_to_peer(peer, &response).await.ok();
}
```

## Action types

```rust
use layer_tl_types::{enums, types};

// Typing a text message
let _guard = TypingGuard::typing(client, peer).await;

// Uploading a photo
let _guard = TypingGuard::upload_photo(client, peer).await;

// Uploading a document / file
let _guard = TypingGuard::upload_document(client, peer).await;

// Recording a voice message
let _guard = TypingGuard::record_audio(client, peer).await;

// Recording a video note
let _guard = TypingGuard::record_round(client, peer).await;

// Choose a sticker
let _guard = TypingGuard::choose_sticker(client, peer).await;

// Custom action (raw TL type)
let _guard = TypingGuard::custom(
    client,
    peer,
    enums::SendMessageAction::SendMessageTypingAction,
).await;
```

## Forum topic typing (top_msg_id)

For forum supergroups, pass the topic's `top_msg_id` to restrict the indicator to that topic:

```rust
client.send_chat_action(
    peer,
    enums::SendMessageAction::SendMessageTypingAction,
    Some(topic_msg_id),  // top_msg_id
).await?;
```

`TypingGuard` sends this on an interval (default 5 seconds) so the indicator doesn't expire while you're working.

## Manual send_chat_action

Without `TypingGuard`, you can fire a one-shot indicator:

```rust
client.send_chat_action(
    peer,
    tl::enums::SendMessageAction::SendMessageTypingAction,
    None,  // no topic
).await?;
```

The indicator automatically expires after ~5 seconds on Telegram's end. `TypingGuard` keeps refreshing it on an interval.

## How TypingGuard works

On creation, `TypingGuard`:
1. Sends the action immediately
2. Spawns a background task that re-sends the action every ~4.5 seconds

On drop, it cancels the background task and optionally sends a `SendMessageCancelAction` to clear the indicator immediately.

## Example: file upload with progress indicator

```rust
async fn send_file_with_progress(
    client: &Client,
    peer: tl::enums::Peer,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Show "uploading document…" while uploading
    let _guard = TypingGuard::upload_document(client, peer.clone()).await;

    let uploaded = client.upload_file(path).await?;

    // Guard drops here, cancelling the indicator
    drop(_guard);

    // Now send the file — no typing indicator
    client.send_file(peer, uploaded.as_document_media(), None).await?;

    Ok(())
}
```
