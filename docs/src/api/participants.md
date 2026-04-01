# Participants & Members

Methods for fetching, banning, promoting, and managing chat members.

## Get participants

```rust
// Get up to 200 participants from a channel/supergroup
let members = client.get_participants(
    tl::enums::Peer::Channel(tl::types::PeerChannel { channel_id: 123 }),
    200,  // limit (0 = use default)
).await?;

for member in members {
    println!("{} ({:?})", member.user.first_name.as_deref().unwrap_or("?"), member.status);
}
```

### ParticipantStatus variants

| Variant | Meaning |
|---|---|
| `Member` | Regular member |
| `Creator` | The group/channel creator |
| `Admin` | Has admin rights |
| `Restricted` | Partially banned (some rights removed) |
| `Banned` | Fully banned |
| `Left` | Has left the group |

---

## Kick from basic group

```rust
// Removes the user from a basic group (not a supergroup/channel)
client.kick_participant(chat_id, user_id).await?;
```

For channels/supergroups, use `ban_participant` instead.

---

## Ban from channel

```rust
// Permanent ban (until_date = 0)
client.ban_participant(
    tl::enums::Peer::Channel(tl::types::PeerChannel { channel_id: 123 }),
    user_id,
    0,  // until_date: 0 = permanent
).await?;

// Temporary ban — expires at unix timestamp
let expires = chrono::Utc::now().timestamp() as i32 + 86400; // 24h
client.ban_participant(peer, user_id, expires).await?;
```

### What ban_participant does

Sets `ChatBannedRights` with `view_messages: true`, which is the Telegram way of banning — it prevents the user from reading or sending any messages.

For selective restrictions (e.g. no stickers, no media), use the raw API with `channels.editBanned`.

---

## Promote / demote admin

```rust
// Grant admin rights
client.promote_participant(channel_peer, user_id, true).await?;

// Remove admin rights
client.promote_participant(channel_peer, user_id, false).await?;
```

The default promotion grants: `change_info`, `post_messages`, `edit_messages`, `delete_messages`, `ban_users`, `invite_users`, `pin_messages`, `manage_call`.

For custom rights, use `channels.editAdmin` via `client.invoke()`:

```rust
use layer_tl_types::{functions, types, enums};

client.invoke(&functions::channels::EditAdmin {
    flags: 0,
    channel: enums::InputChannel::InputChannel(types::InputChannel {
        channel_id, access_hash,
    }),
    user_id: enums::InputUser::InputUser(types::InputUser {
        user_id, access_hash: user_hash,
    }),
    admin_rights: enums::ChatAdminRights::ChatAdminRights(types::ChatAdminRights {
        change_info:            true,
        post_messages:          true,
        edit_messages:          false,
        delete_messages:        true,
        ban_users:              true,
        invite_users:           true,
        pin_messages:           true,
        add_admins:             false,  // can they add other admins?
        anonymous:              false,
        manage_call:            true,
        other:                  false,
        manage_topics:          false,
        post_stories:           false,
        edit_stories:           false,
        delete_stories:         false,
        manage_direct_messages: false,
        manage_ranks:           false,   // Layer 223: custom rank management
    }),
    rank: Some("Moderator".into()),  // Layer 223: rank is now Option<String>
}).await?;
```

---

## Get profile photos

```rust
let photos = client.get_profile_photos(peer, 10).await?;

for photo in &photos {
    if let tl::enums::Photo::Photo(p) = photo {
        println!("Photo ID: {}", p.id);
    }
}
```

---

## Send a reaction

```rust
// React with 👍
client.send_reaction(peer, message_id, "👍").await?;

// Remove reaction
client.send_reaction(peer, message_id, "").await?;

// Custom emoji reaction (premium)
// Use the raw API: messages.sendReaction with ReactionCustomEmoji
```

---

## ChatAdminRights — Layer 223 fields

```rust
types::ChatAdminRights {
    change_info:            bool, // can change group info
    post_messages:          bool, // can post in channels
    edit_messages:          bool, // can edit any message
    delete_messages:        bool, // can delete messages
    ban_users:              bool, // can ban members
    invite_users:           bool, // can invite members
    pin_messages:           bool, // can pin messages
    add_admins:             bool, // can promote admins
    anonymous:              bool, // post as channel anonymously
    manage_call:            bool, // can start/manage calls
    other:                  bool, // other rights
    manage_topics:          bool, // can manage forum topics
    post_stories:           bool, // can post stories
    edit_stories:           bool, // can edit stories
    delete_stories:         bool, // can delete stories
    manage_direct_messages: bool, // can manage DM links
    manage_ranks:           bool, // ✨ NEW in Layer 223
}
```

## ChatBannedRights — Layer 223 fields

```rust
types::ChatBannedRights {
    view_messages:    bool, // ban completely (can't read)
    send_messages:    bool, // can't send text
    send_media:       bool, // can't send media
    send_stickers:    bool,
    send_gifs:        bool,
    send_games:       bool,
    send_inline:      bool, // can't use inline bots
    embed_links:      bool, // can't embed link previews
    send_polls:       bool,
    change_info:      bool, // can't change group info
    invite_users:     bool, // can't invite others
    pin_messages:     bool, // can't pin messages
    manage_topics:    bool,
    send_photos:      bool,
    send_videos:      bool,
    send_roundvideos: bool,
    send_audios:      bool,
    send_voices:      bool,
    send_docs:        bool,
    send_plain:       bool, // can't send plain text
    edit_rank:        bool, // ✨ NEW in Layer 223
    until_date:       i32,  // 0 = permanent
}
```
