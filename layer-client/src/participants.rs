//! Chat participant and member management.
//!
//! Provides [`Client::get_participants`], kick, ban, and admin rights management.

use layer_tl_types as tl;
use layer_tl_types::{Cursor, Deserializable};

use crate::{Client, InvocationError};

// ─── Participant ──────────────────────────────────────────────────────────────

/// A member of a chat, group or channel.
#[derive(Debug, Clone)]
pub struct Participant {
    /// The user object.
    pub user: tl::types::User,
    /// Their role/status in the chat.
    pub status: ParticipantStatus,
}

/// The role of a participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticipantStatus {
    /// Regular member.
    Member,
    /// The channel/group creator.
    Creator,
    /// Admin (may have custom title).
    Admin,
    /// Restricted / banned user.
    Restricted,
    /// Left the group.
    Left,
    /// Kicked (banned) from the group.
    Banned,
}

// ─── Client methods ───────────────────────────────────────────────────────────

impl Client {
    /// Fetch all participants of a chat, group or channel.
    ///
    /// For channels this uses `channels.getParticipants`; for basic groups it
    /// uses `messages.getFullChat`.
    ///
    /// Returns up to `limit` participants; pass `0` for the default (200 for channels).
    pub async fn get_participants(
        &self,
        peer:  tl::enums::Peer,
        limit: i32,
    ) -> Result<Vec<Participant>, InvocationError> {
        match &peer {
            tl::enums::Peer::Channel(c) => {
                let cache       = self.inner.peer_cache.lock().await;
                let access_hash = cache.channels.get(&c.channel_id).copied().unwrap_or(0);
                drop(cache);
                self.get_channel_participants(c.channel_id, access_hash, limit).await
            }
            tl::enums::Peer::Chat(c) => {
                self.get_chat_participants(c.chat_id).await
            }
            _ => Err(InvocationError::Deserialize("get_participants: peer must be a chat or channel".into())),
        }
    }

    async fn get_channel_participants(
        &self,
        channel_id:  i64,
        access_hash: i64,
        limit:       i32,
    ) -> Result<Vec<Participant>, InvocationError> {
        let limit = if limit <= 0 { 200 } else { limit };
        let req = tl::functions::channels::GetParticipants {
            channel: tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                channel_id, access_hash,
            }),
            filter:  tl::enums::ChannelParticipantsFilter::ChannelParticipantsRecent,
            offset:  0,
            limit,
            hash:    0,
        };
        let body    = self.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let raw     = match tl::enums::channels::ChannelParticipants::deserialize(&mut cur)? {
            tl::enums::channels::ChannelParticipants::ChannelParticipants(p) => p,
            tl::enums::channels::ChannelParticipants::NotModified => return Ok(vec![]),
        };

        // Build user map
        let user_map: std::collections::HashMap<i64, tl::types::User> = raw.users.into_iter()
            .filter_map(|u| match u { tl::enums::User::User(u) => Some((u.id, u)), _ => None })
            .collect();

        // Cache them
        {
            let mut cache = self.inner.peer_cache.lock().await;
            for u in user_map.values() {
                if let Some(h) = u.access_hash { cache.users.insert(u.id, h); }
            }
        }

        let mut result = Vec::new();
        for p in raw.participants {
            let (user_id, status) = match &p {
                tl::enums::ChannelParticipant::ChannelParticipant(x) => (x.user_id, ParticipantStatus::Member),
                tl::enums::ChannelParticipant::ParticipantSelf(x)    => (x.user_id, ParticipantStatus::Member),
                tl::enums::ChannelParticipant::Creator(x)            => (x.user_id, ParticipantStatus::Creator),
                tl::enums::ChannelParticipant::Admin(x)              => (x.user_id, ParticipantStatus::Admin),
                tl::enums::ChannelParticipant::Banned(x)             => (x.peer.user_id_or(0), ParticipantStatus::Banned),
                tl::enums::ChannelParticipant::Left(x)               => (x.peer.user_id_or(0), ParticipantStatus::Left),
            };
            if let Some(user) = user_map.get(&user_id).cloned() {
                result.push(Participant { user, status });
            }
        }
        Ok(result)
    }

    async fn get_chat_participants(&self, chat_id: i64) -> Result<Vec<Participant>, InvocationError> {
        let req  = tl::functions::messages::GetFullChat { chat_id };
        let body = self.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let full: tl::types::messages::ChatFull = match tl::enums::messages::ChatFull::deserialize(&mut cur)? {
            tl::enums::messages::ChatFull::ChatFull(c) => c,
        };

        let user_map: std::collections::HashMap<i64, tl::types::User> = full.users.into_iter()
            .filter_map(|u| match u { tl::enums::User::User(u) => Some((u.id, u)), _ => None })
            .collect();

        {
            let mut cache = self.inner.peer_cache.lock().await;
            for u in user_map.values() {
                if let Some(h) = u.access_hash { cache.users.insert(u.id, h); }
            }
        }

        let participants = match &full.full_chat {
            tl::enums::ChatFull::ChatFull(cf) => match &cf.participants {
                tl::enums::ChatParticipants::ChatParticipants(p) => p.participants.clone(),
                tl::enums::ChatParticipants::Forbidden(_)        => vec![],
            },
            tl::enums::ChatFull::ChannelFull(_) => {
                return Err(InvocationError::Deserialize(
                    "get_chat_participants: peer is a channel, use get_participants with a Channel peer instead".into()
                ));
            }
        };

        let mut result = Vec::new();
        for p in participants {
            let (user_id, status) = match p {
                tl::enums::ChatParticipant::ChatParticipant(x) => (x.user_id, ParticipantStatus::Member),
                tl::enums::ChatParticipant::Creator(x)          => (x.user_id, ParticipantStatus::Creator),
                tl::enums::ChatParticipant::Admin(x)            => (x.user_id, ParticipantStatus::Admin),
            };
            if let Some(user) = user_map.get(&user_id).cloned() {
                result.push(Participant { user, status });
            }
        }
        Ok(result)
    }

    /// Kick a user from a basic group (chat). For channels, use [`ban_participant`].
    pub async fn kick_participant(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), InvocationError> {
        let cache       = self.inner.peer_cache.lock().await;
        let access_hash = cache.users.get(&user_id).copied().unwrap_or(0);
        drop(cache);
        let req = tl::functions::messages::DeleteChatUser {
            revoke_history: false,
            chat_id,
            user_id: tl::enums::InputUser::InputUser(tl::types::InputUser { user_id, access_hash }),
        };
        self.rpc_call_raw_pub(&req).await?;
        Ok(())
    }

    /// Ban a user from a channel or supergroup.
    ///
    /// Pass `until_date = 0` for a permanent ban.
    pub async fn ban_participant(
        &self,
        channel:    tl::enums::Peer,
        user_id:    i64,
        until_date: i32,
    ) -> Result<(), InvocationError> {
        let (channel_id, ch_hash) = match &channel {
            tl::enums::Peer::Channel(c) => {
                let h = self.inner.peer_cache.lock().await.channels.get(&c.channel_id).copied().unwrap_or(0);
                (c.channel_id, h)
            }
            _ => return Err(InvocationError::Deserialize("ban_participant: peer must be a channel".into())),
        };
        let user_hash = self.inner.peer_cache.lock().await.users.get(&user_id).copied().unwrap_or(0);

        let req = tl::functions::channels::EditBanned {
            channel: tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                channel_id: channel_id, access_hash: ch_hash,
            }),
            participant: tl::enums::InputPeer::User(tl::types::InputPeerUser {
                user_id, access_hash: user_hash,
            }),
            banned_rights: tl::enums::ChatBannedRights::ChatBannedRights(tl::types::ChatBannedRights {
                view_messages:   true,
                send_messages:   true,
                send_media:      true,
                send_stickers:   true,
                send_gifs:       true,
                send_games:      true,
                send_inline:     true,
                embed_links:     true,
                send_polls:      true,
                change_info:     true,
                invite_users:    true,
                pin_messages:    true,
                manage_topics:   false,
                send_photos:     false,
                send_videos:     false,
                send_roundvideos: false,
                send_audios:     false,
                send_voices:     false,
                send_docs:       false,
                send_plain:      false,
                until_date,
            }),
        };
        self.rpc_call_raw_pub(&req).await?;
        Ok(())
    }

    /// Promote (or demote) a user to admin in a channel or supergroup.
    ///
    /// Pass `promote = true` to grant admin rights, `false` to remove them.
    pub async fn promote_participant(
        &self,
        channel: tl::enums::Peer,
        user_id: i64,
        promote: bool,
    ) -> Result<(), InvocationError> {
        let (channel_id, ch_hash) = match &channel {
            tl::enums::Peer::Channel(c) => {
                let h = self.inner.peer_cache.lock().await.channels.get(&c.channel_id).copied().unwrap_or(0);
                (c.channel_id, h)
            }
            _ => return Err(InvocationError::Deserialize("promote_participant: peer must be a channel".into())),
        };
        let user_hash = self.inner.peer_cache.lock().await.users.get(&user_id).copied().unwrap_or(0);

        let rights = if promote {
            tl::types::ChatAdminRights {
                change_info:            true,
                post_messages:          true,
                edit_messages:          true,
                delete_messages:        true,
                ban_users:              true,
                invite_users:           true,
                pin_messages:           true,
                add_admins:             false,
                anonymous:              false,
                manage_call:            true,
                other:                  false,
                manage_topics:          false,
                post_stories:           false,
                edit_stories:           false,
                delete_stories:         false,
                manage_direct_messages: false,
            }
        } else {
            tl::types::ChatAdminRights {
                change_info:            false,
                post_messages:          false,
                edit_messages:          false,
                delete_messages:        false,
                ban_users:              false,
                invite_users:           false,
                pin_messages:           false,
                add_admins:             false,
                anonymous:              false,
                manage_call:            false,
                other:                  false,
                manage_topics:          false,
                post_stories:           false,
                edit_stories:           false,
                delete_stories:         false,
                manage_direct_messages: false,
            }
        };

        let req = tl::functions::channels::EditAdmin {
            channel: tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                channel_id, access_hash: ch_hash,
            }),
            user_id: tl::enums::InputUser::InputUser(tl::types::InputUser { user_id, access_hash: user_hash }),
            admin_rights: tl::enums::ChatAdminRights::ChatAdminRights(rights),
            rank: String::new(),
        };
        self.rpc_call_raw_pub(&req).await?;
        Ok(())
    }

    /// Iterate profile photos of a user or channel.
    ///
    /// Returns a list of photo objects (up to `limit`).
    pub async fn get_profile_photos(
        &self,
        peer:  tl::enums::Peer,
        limit: i32,
    ) -> Result<Vec<tl::enums::Photo>, InvocationError> {
        let input_peer = {
            let cache = self.inner.peer_cache.lock().await;
            cache.peer_to_input(&peer)
        };

        let req = tl::functions::photos::GetUserPhotos {
            user_id: match &input_peer {
                tl::enums::InputPeer::User(u) => tl::enums::InputUser::InputUser(
                    tl::types::InputUser { user_id: u.user_id, access_hash: u.access_hash }
                ),
                tl::enums::InputPeer::PeerSelf => tl::enums::InputUser::UserSelf,
                _ => return Err(InvocationError::Deserialize("get_profile_photos: peer must be a user".into())),
            },
            offset: 0,
            max_id: 0,
            limit,
        };
        let body    = self.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        match tl::enums::photos::Photos::deserialize(&mut cur)? {
            tl::enums::photos::Photos::Photos(p)  => Ok(p.photos),
            tl::enums::photos::Photos::Slice(p)   => Ok(p.photos),
        }
    }

    /// Search for a peer (user, group, or channel) by name prefix.
    ///
    /// Searches contacts, dialogs, and globally. Returns combined results.
    pub async fn search_peer(
        &self,
        query: &str,
    ) -> Result<Vec<tl::enums::Peer>, InvocationError> {
        let req  = tl::functions::contacts::Search {
            q:   query.to_string(),
            limit: 20,
        };
        let body = self.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let found = match tl::enums::contacts::Found::deserialize(&mut cur)? {
            tl::enums::contacts::Found::Found(f) => f,
        };

        self.cache_users_slice_pub(&found.users).await;
        self.cache_chats_slice_pub(&found.chats).await;

        let mut peers = Vec::new();
        for r in found.my_results.iter().chain(found.results.iter()) {
            peers.push(r.clone());
        }
        Ok(peers)
    }

    /// Send a reaction to a message.
    ///
    /// `reaction` should be an emoji string like `"👍"` or an empty string to remove.
    pub async fn send_reaction(
        &self,
        peer:       tl::enums::Peer,
        message_id: i32,
        reaction:   &str,
    ) -> Result<(), InvocationError> {
        let input_peer = {
            let cache = self.inner.peer_cache.lock().await;
            cache.peer_to_input(&peer)
        };

        let reactions = if reaction.is_empty() {
            vec![]
        } else {
            vec![tl::enums::Reaction::Emoji(tl::types::ReactionEmoji {
                emoticon: reaction.to_string(),
            })]
        };

        let req = tl::functions::messages::SendReaction {
            big:        false,
            add_to_recent: false,
            peer:       input_peer,
            msg_id:     message_id,
            reaction:   Some(reactions),
        };
        self.rpc_call_raw_pub(&req).await?;
        Ok(())
    }
}

// ─── Helper extension for Peer ────────────────────────────────────────────────

trait PeerUserIdExt {
    fn user_id_or(&self, default: i64) -> i64;
}

impl PeerUserIdExt for tl::enums::Peer {
    fn user_id_or(&self, default: i64) -> i64 {
        match self {
            tl::enums::Peer::User(u) => u.user_id,
            _ => default,
        }
    }
}
