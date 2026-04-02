# Admin & Ban Rights

`layer-client` provides typed builders for promoting administrators and restricting users.

---

## Promoting an admin — AdminRightsBuilder

```rust
use layer_client::participants::AdminRightsBuilder;

client.set_admin_rights(
    peer,
    user_id,
    AdminRightsBuilder::new()
        .post_messages(true)
        .edit_messages(true)
        .delete_messages(true)
        .invite_users(true)
        .pin_messages(true)
        .rank("Editor"),  // optional custom title
).await?;
```

### All AdminRightsBuilder methods

| Method | Default | Description |
|---|---|---|
| `.change_info(v)` | false | Edit channel title, description, photo |
| `.post_messages(v)` | false | Post in the channel |
| `.edit_messages(v)` | false | Edit any message |
| `.delete_messages(v)` | false | Delete any message |
| `.ban_users(v)` | false | Restrict other members |
| `.invite_users(v)` | false | Add members |
| `.pin_messages(v)` | false | Pin and unpin messages |
| `.add_admins(v)` | false | Promote other admins (requires self to have this right) |
| `.anonymous(v)` | false | Post as the channel (anonymous) |
| `.manage_call(v)` | false | Start/manage video chats |
| `.manage_topics(v)` | false | Create/edit/delete forum topics |
| `.rank(r)` | — | Custom admin title shown in the member list |
| `AdminRightsBuilder::full_admin()` | — | All rights enabled |

### Full admin shorthand

```rust
client.set_admin_rights(
    peer,
    user_id,
    AdminRightsBuilder::full_admin(),
).await?;
```

### Remove admin rights

Pass an empty builder to revoke all rights:

```rust
client.set_admin_rights(
    peer,
    user_id,
    AdminRightsBuilder::new(),  // all false = remove admin
).await?;
```

---

## Restricting a member — BanRightsBuilder

```rust
use layer_client::participants::BanRightsBuilder;

// Mute a user (disable text, stickers, GIFs, inline)
client.set_banned_rights(
    peer,
    user_id,
    BanRightsBuilder::new()
        .send_messages(false)
        .send_stickers(false)
        .send_gifs(false)
        .send_inline(false),
).await?;
```

### All BanRightsBuilder methods

| Method | Default | Description |
|---|---|---|
| `.view_messages(v)` | true | Can see the chat at all |
| `.send_messages(v)` | true | Can send text messages |
| `.send_media(v)` | true | Can send photos, videos, etc. |
| `.send_stickers(v)` | true | Can send stickers |
| `.send_gifs(v)` | true | Can send GIFs |
| `.send_games(v)` | true | Can send games |
| `.send_inline(v)` | true | Can use inline bots |
| `.embed_links(v)` | true | Can include link previews |
| `.send_polls(v)` | true | Can create polls |
| `.change_info(v)` | true | Can edit group info |
| `.invite_users(v)` | true | Can add members |
| `.pin_messages(v)` | true | Can pin messages |
| `.until_date(ts)` | 0 (permanent) | Restriction expires at this unix timestamp |
| `BanRightsBuilder::full_ban()` | — | All rights false (full kick/ban) |

### Temporary restriction (mute for 24 hours)

```rust
let expires = chrono::Utc::now().timestamp() as i32 + 86_400; // 24h

client.set_banned_rights(
    peer,
    user_id,
    BanRightsBuilder::new()
        .send_messages(false)
        .send_media(false)
        .until_date(expires),
).await?;
```

### Full ban (kick + block from re-joining)

```rust
client.set_banned_rights(
    peer,
    user_id,
    BanRightsBuilder::full_ban(),
).await?;
```

### Lift all restrictions (unban)

Pass a builder with all defaults (everything true, no until_date):

```rust
client.set_banned_rights(
    peer,
    user_id,
    BanRightsBuilder::new(),  // all true = no restrictions
).await?;
```

---

## Check effective permissions

```rust
let perms = client.get_permissions(peer, user_id).await?;
println!("can send messages: {}", perms.can_send_messages());
println!("is admin: {}", perms.is_admin());
println!("is creator: {}", perms.is_creator());
```

---

## Kick from basic group

For legacy basic groups (not supergroups/channels), use the kick method:

```rust
client.kick_participant(chat_id, user_id).await?;
```

This removes the user from the group. They can be added back by any member.

---

## Participant status

`get_participants` returns `Participant` structs with a `status` field:

```rust
let members = client.get_participants(peer, 0).await?;

for member in members {
    match member.status {
        ParticipantStatus::Creator  => println!("{} is the creator", member.user.first_name.as_deref().unwrap_or("?")),
        ParticipantStatus::Admin    => println!("{} is an admin", member.user.first_name.as_deref().unwrap_or("?")),
        ParticipantStatus::Member   => {}
        ParticipantStatus::Banned   => println!("{} is banned", member.user.first_name.as_deref().unwrap_or("?")),
        ParticipantStatus::Restricted => println!("{} is restricted", member.user.first_name.as_deref().unwrap_or("?")),
        ParticipantStatus::Left     => println!("{} has left", member.user.first_name.as_deref().unwrap_or("?")),
    }
}
```
