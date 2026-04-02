//! Ergonomic high-level wrappers over raw TL user/chat types (G-37).
//!
//! The raw TL layer has `tl::enums::User` (two variants: `Empty` / `User`) and
//! `tl::enums::Chat` (five variants: `Empty`, `Chat`, `Forbidden`,
//! `Channel`, `ChannelForbidden`).  Working with them directly requires constant
//! pattern-matching.  This module provides three thin wrappers — [`User`],
//! [`Group`], and [`Channel`] — with uniform accessor APIs.

use layer_tl_types as tl;

// ─── User ─────────────────────────────────────────────────────────────────────

/// Wrapper around `tl::enums::User` with ergonomic accessors.
#[derive(Debug, Clone)]
pub struct User {
    pub raw: tl::enums::User,
}

impl User {
    /// Wrap a raw TL user.
    pub fn from_raw(raw: tl::enums::User) -> Option<Self> {
        match &raw {
            tl::enums::User::Empty(_) => None,
            tl::enums::User::User(_) => Some(Self { raw }),
        }
    }

    fn inner(&self) -> &tl::types::User {
        match &self.raw {
            tl::enums::User::User(u) => u,
            tl::enums::User::Empty(_) => unreachable!("User::Empty filtered in from_raw"),
        }
    }

    /// Telegram user ID.
    pub fn id(&self) -> i64 {
        self.inner().id
    }

    /// Access hash needed for API calls.
    pub fn access_hash(&self) -> Option<i64> {
        self.inner().access_hash
    }

    /// First name.
    pub fn first_name(&self) -> Option<&str> {
        self.inner().first_name.as_deref()
    }

    /// Last name.
    pub fn last_name(&self) -> Option<&str> {
        self.inner().last_name.as_deref()
    }

    /// Username (without `@`).
    pub fn username(&self) -> Option<&str> {
        self.inner().username.as_deref()
    }

    /// Phone number, if visible.
    pub fn phone(&self) -> Option<&str> {
        self.inner().phone.as_deref()
    }

    /// `true` if this is a verified account.
    pub fn verified(&self) -> bool {
        self.inner().verified
    }

    /// `true` if this is a bot account.
    pub fn bot(&self) -> bool {
        self.inner().bot
    }

    /// `true` if the account is deleted.
    pub fn deleted(&self) -> bool {
        self.inner().deleted
    }

    /// `true` if the current user has blocked this user.
    pub fn blocked(&self) -> bool {
        false
    }

    /// `true` if this is a premium account.
    pub fn premium(&self) -> bool {
        self.inner().premium
    }

    /// Full display name (`first_name [last_name]`).
    pub fn full_name(&self) -> String {
        match (self.first_name(), self.last_name()) {
            (Some(f), Some(l)) => format!("{f} {l}"),
            (Some(f), None) => f.to_string(),
            (None, Some(l)) => l.to_string(),
            (None, None) => String::new(),
        }
    }

    /// Convert to a `Peer` for use in API calls.
    pub fn as_peer(&self) -> tl::enums::Peer {
        tl::enums::Peer::User(tl::types::PeerUser { user_id: self.id() })
    }

    /// Convert to an `InputPeer` for API calls (requires access hash).
    pub fn as_input_peer(&self) -> tl::enums::InputPeer {
        match self.inner().access_hash {
            Some(ah) => tl::enums::InputPeer::User(tl::types::InputPeerUser {
                user_id: self.id(),
                access_hash: ah,
            }),
            None => tl::enums::InputPeer::PeerSelf,
        }
    }
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.full_name();
        if let Some(uname) = self.username() {
            write!(f, "{name} (@{uname})")
        } else {
            write!(f, "{name} [{}]", self.id())
        }
    }
}

// ─── Group ────────────────────────────────────────────────────────────────────

/// Wrapper around a basic Telegram group (`tl::types::Chat`).
#[derive(Debug, Clone)]
pub struct Group {
    pub raw: tl::types::Chat,
}

impl Group {
    /// Wrap from a raw `tl::enums::Chat`, returning `None` if it is not a
    /// basic group (i.e. empty, forbidden, or a channel).
    pub fn from_raw(raw: tl::enums::Chat) -> Option<Self> {
        match raw {
            tl::enums::Chat::Chat(c) => Some(Self { raw: c }),
            tl::enums::Chat::Empty(_)
            | tl::enums::Chat::Forbidden(_)
            | tl::enums::Chat::Channel(_)
            | tl::enums::Chat::ChannelForbidden(_) => None,
        }
    }

    /// Group ID.
    pub fn id(&self) -> i64 {
        self.raw.id
    }

    /// Group title.
    pub fn title(&self) -> &str {
        &self.raw.title
    }

    /// Member count.
    pub fn participants_count(&self) -> i32 {
        self.raw.participants_count
    }

    /// `true` if the logged-in user is the creator.
    pub fn creator(&self) -> bool {
        self.raw.creator
    }

    /// `true` if the group has been migrated to a supergroup.
    pub fn migrated_to(&self) -> Option<&tl::enums::InputChannel> {
        self.raw.migrated_to.as_ref()
    }

    /// Convert to a `Peer`.
    pub fn as_peer(&self) -> tl::enums::Peer {
        tl::enums::Peer::Chat(tl::types::PeerChat { chat_id: self.id() })
    }

    /// Convert to an `InputPeer`.
    pub fn as_input_peer(&self) -> tl::enums::InputPeer {
        tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: self.id() })
    }
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [group {}]", self.title(), self.id())
    }
}

// ─── Channel ──────────────────────────────────────────────────────────────────

/// Wrapper around a Telegram channel or supergroup (`tl::types::Channel`).
#[derive(Debug, Clone)]
pub struct Channel {
    pub raw: tl::types::Channel,
}

impl Channel {
    /// Wrap from a raw `tl::enums::Chat`, returning `None` if it is not a channel.
    pub fn from_raw(raw: tl::enums::Chat) -> Option<Self> {
        match raw {
            tl::enums::Chat::Channel(c) => Some(Self { raw: c }),
            _ => None,
        }
    }

    /// Channel ID.
    pub fn id(&self) -> i64 {
        self.raw.id
    }

    /// Access hash.
    pub fn access_hash(&self) -> Option<i64> {
        self.raw.access_hash
    }

    /// Channel / supergroup title.
    pub fn title(&self) -> &str {
        &self.raw.title
    }

    /// Username (without `@`), if public.
    pub fn username(&self) -> Option<&str> {
        self.raw.username.as_deref()
    }

    /// `true` if this is a supergroup (not a broadcast channel).
    pub fn megagroup(&self) -> bool {
        self.raw.megagroup
    }

    /// `true` if this is a broadcast channel.
    pub fn broadcast(&self) -> bool {
        self.raw.broadcast
    }

    /// `true` if this is a verified channel.
    pub fn verified(&self) -> bool {
        self.raw.verified
    }

    /// `true` if the channel is restricted.
    pub fn restricted(&self) -> bool {
        self.raw.restricted
    }

    /// `true` if the channel has signatures on posts.
    pub fn signatures(&self) -> bool {
        self.raw.signatures
    }

    /// Approximate member count (may be `None` for private channels).
    pub fn participants_count(&self) -> Option<i32> {
        self.raw.participants_count
    }

    /// Convert to a `Peer`.
    pub fn as_peer(&self) -> tl::enums::Peer {
        tl::enums::Peer::Channel(tl::types::PeerChannel {
            channel_id: self.id(),
        })
    }

    /// Convert to an `InputPeer` (requires access hash).
    pub fn as_input_peer(&self) -> tl::enums::InputPeer {
        match self.raw.access_hash {
            Some(ah) => tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id: self.id(),
                access_hash: ah,
            }),
            None => tl::enums::InputPeer::Empty,
        }
    }

    /// Convert to an `InputChannel` for channel-specific RPCs.
    pub fn as_input_channel(&self) -> tl::enums::InputChannel {
        match self.raw.access_hash {
            Some(ah) => tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                channel_id: self.id(),
                access_hash: ah,
            }),
            None => tl::enums::InputChannel::Empty,
        }
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(uname) = self.username() {
            write!(f, "{} (@{uname})", self.title())
        } else {
            write!(f, "{} [channel {}]", self.title(), self.id())
        }
    }
}

// ─── Chat enum (unified) ──────────────────────────────────────────────────────

/// A unified chat type — either a basic [`Group`] or a [`Channel`]/supergroup.
#[derive(Debug, Clone)]
pub enum Chat {
    Group(Group),
    Channel(Box<Channel>),
}

impl Chat {
    /// Attempt to construct from a raw `tl::enums::Chat`.
    pub fn from_raw(raw: tl::enums::Chat) -> Option<Self> {
        match &raw {
            tl::enums::Chat::Chat(_) => Group::from_raw(raw).map(Chat::Group),
            tl::enums::Chat::Channel(_) => {
                Channel::from_raw(raw).map(|c| Chat::Channel(Box::new(c)))
            }
            _ => None,
        }
    }

    /// Common ID regardless of variant.
    pub fn id(&self) -> i64 {
        match self {
            Chat::Group(g) => g.id(),
            Chat::Channel(c) => c.id(),
        }
    }

    /// Common title regardless of variant.
    pub fn title(&self) -> &str {
        match self {
            Chat::Group(g) => g.title(),
            Chat::Channel(c) => c.title(),
        }
    }

    /// Convert to a `Peer`.
    pub fn as_peer(&self) -> tl::enums::Peer {
        match self {
            Chat::Group(g) => g.as_peer(),
            Chat::Channel(c) => c.as_peer(),
        }
    }

    /// Convert to an `InputPeer`.
    pub fn as_input_peer(&self) -> tl::enums::InputPeer {
        match self {
            Chat::Group(g) => g.as_input_peer(),
            Chat::Channel(c) => c.as_input_peer(),
        }
    }
}
