# Reactions

Reactions are emoji responses attached to messages. They appear below messages and include a count of how many users chose each reaction.

## Send a reaction

```rust
use layer_client::reactions::Reaction;

// Emoji reaction
client.send_reaction(peer, message_id, Reaction::emoticon("👍")).await?;

// Custom emoji reaction (premium)
client.send_reaction(peer, message_id, Reaction::custom_emoji(document_id)).await?;

// Remove all reactions from a message
client.send_reaction(peer, message_id, Reaction::remove()).await?;
```

## Big reaction

Send a "big" reaction (plays a full-screen animation):

```rust
client.send_reaction(
    peer,
    message_id,
    Reaction::emoticon("🔥").big(),
).await?;
```

## Add to recent

Keep the reaction in the user's recently-used list:

```rust
client.send_reaction(
    peer,
    message_id,
    Reaction::emoticon("❤️").add_to_recent(),
).await?;
```

## Reading reactions from a message

When you receive a message, reactions are available via `msg.reactions()`:

```rust
Update::NewMessage(msg) => {
    if let Some(reactions) = msg.reactions() {
        // reactions is &tl::enums::MessageReactions
        if let tl::enums::MessageReactions::MessageReactions(r) = reactions {
            for result in &r.results {
                if let tl::enums::ReactionCount::ReactionCount(rc) = result {
                    let count = rc.count;
                    match &rc.reaction {
                        tl::enums::Reaction::Emoji(e) => {
                            println!("{}: {}", e.emoticon, count);
                        }
                        tl::enums::Reaction::CustomEmoji(e) => {
                            println!("custom emoji {}: {}", e.document_id, count);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
```

## Reaction builder reference

| Method | Description |
|---|---|
| `Reaction::emoticon("👍")` | Standard emoji reaction |
| `Reaction::custom_emoji(doc_id)` | Custom emoji (Premium) |
| `Reaction::remove()` | Remove all reactions |
| `.big()` | Full-screen animation |
| `.add_to_recent()` | Add to user's recent list |

## Raw API: get who reacted

To see which users chose a specific reaction on a message:

```rust
use layer_tl_types::{functions, enums, types};

let result = client.invoke(&functions::messages::GetMessageReactionsList {
    peer:      peer_input,
    id:        message_id,
    reaction:  Some(enums::Reaction::Emoji(types::ReactionEmoji {
        emoticon: "👍".into(),
    })),
    offset:    None,
    limit:     50,
}).await?;

if let enums::messages::MessageReactionsList::MessageReactionsList(list) = result {
    for reaction_with_peer in &list.reactions {
        if let enums::MessagePeerReaction::MessagePeerReaction(r) = reaction_with_peer {
            println!("peer: {:?}", r.peer_id);
        }
    }
}
```
