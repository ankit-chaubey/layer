//! High-level update types delivered by [`crate::Client::next_update`].
//!
//! Every update the Telegram server pushes is classified into one of the
//! variants of [`Update`].  The raw constructor ID and bytes are always
//! available via [`Update::Raw`] for anything not yet wrapped.

use layer_tl_types as tl;
use layer_tl_types::{Cursor, Deserializable};

use crate::{Client, InvocationError as Error};

// ─── IncomingMessage ─────────────────────────────────────────────────────────

/// A new or edited message.
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    /// The underlying TL message object.
    pub raw: tl::enums::Message,
}

impl IncomingMessage {
    pub(crate) fn from_raw(raw: tl::enums::Message) -> Self {
        Self { raw }
    }

    /// The message text (or caption for media messages).
    pub fn text(&self) -> Option<&str> {
        match &self.raw {
            tl::enums::Message::Message(m) => {
                if m.message.is_empty() { None } else { Some(&m.message) }
            }
            _ => None,
        }
    }

    /// Unique message ID within the chat.
    pub fn id(&self) -> i32 {
        match &self.raw {
            tl::enums::Message::Message(m) => m.id,
            tl::enums::Message::Service(m) => m.id,
            tl::enums::Message::Empty(m)   => m.id,
        }
    }

    /// The peer (chat) this message was sent in.
    pub fn peer_id(&self) -> Option<&tl::enums::Peer> {
        match &self.raw {
            tl::enums::Message::Message(m) => Some(&m.peer_id),
            tl::enums::Message::Service(m) => Some(&m.peer_id),
            _ => None,
        }
    }

    /// The sender, if available (not set for channel posts).
    pub fn sender_id(&self) -> Option<&tl::enums::Peer> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.from_id.as_ref(),
            tl::enums::Message::Service(m) => m.from_id.as_ref(),
            _ => None,
        }
    }

    /// `true` if the message was sent by the logged-in account.
    pub fn outgoing(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Message(m) => m.out,
            tl::enums::Message::Service(m) => m.out,
            _ => false,
        }
    }

    /// Reply to this message with plain text.
    pub async fn reply(&self, client: &mut Client, text: impl Into<String>) -> Result<(), Error> {
        let peer = match self.peer_id() {
            Some(p) => p.clone(),
            None    => return Err(Error::Deserialize("cannot reply: unknown peer".into())),
        };
        client.send_message_to_peer(peer, &text.into()).await
    }
}

// ─── MessageDeletion ─────────────────────────────────────────────────────────

/// One or more messages were deleted.
#[derive(Debug, Clone)]
pub struct MessageDeletion {
    /// IDs of the deleted messages.
    pub message_ids: Vec<i32>,
    /// Channel ID, if the deletion happened in a channel / supergroup.
    pub channel_id:  Option<i64>,
}

// ─── CallbackQuery ───────────────────────────────────────────────────────────

/// A user pressed an inline keyboard button on a bot message.
#[derive(Debug, Clone)]
pub struct CallbackQuery {
    pub query_id:        i64,
    pub user_id:         i64,
    pub message_id:      Option<i32>,
    pub chat_instance:   i64,
    /// Raw `data` bytes from the button.
    pub data_raw:        Option<Vec<u8>>,
    /// Game short name (if a game button was pressed).
    pub game_short_name: Option<String>,
}

impl CallbackQuery {
    /// Button data as a UTF-8 string, if valid.
    pub fn data(&self) -> Option<&str> {
        self.data_raw.as_ref().and_then(|d| std::str::from_utf8(d).ok())
    }

    /// Answer the callback query (removes the loading indicator on the client).
    pub async fn answer(
        &self,
        client: &mut Client,
        text:   Option<&str>,
    ) -> Result<(), Error> {
        client.answer_callback_query(self.query_id, text, false).await.map(|_| ())
    }

    /// Answer with a popup alert.
    pub async fn answer_alert(
        &self,
        client: &mut Client,
        text:   &str,
    ) -> Result<(), Error> {
        client.answer_callback_query(self.query_id, Some(text), true).await.map(|_| ())
    }
}

// ─── InlineQuery ─────────────────────────────────────────────────────────────

/// A user is typing an inline query (`@bot something`).
#[derive(Debug, Clone)]
pub struct InlineQuery {
    pub query_id: i64,
    pub user_id:  i64,
    pub query:    String,
    pub offset:   String,
}

impl InlineQuery {
    /// The text the user typed after the bot username.
    pub fn query(&self) -> &str { &self.query }
}

// ─── RawUpdate ───────────────────────────────────────────────────────────────

/// A TL update that has no dedicated high-level variant yet.
#[derive(Debug, Clone)]
pub struct RawUpdate {
    /// Constructor ID of the inner update.
    pub constructor_id: u32,
}

// ─── Update ───────────────────────────────────────────────────────────────────

/// A high-level event received from Telegram.
///
/// See [`crate::Client::next_update`] for usage.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Update {
    /// A new message (personal chat, group, channel, or bot command).
    NewMessage(IncomingMessage),
    /// An existing message was edited.
    MessageEdited(IncomingMessage),
    /// One or more messages were deleted.
    MessageDeleted(MessageDeletion),
    /// An inline keyboard button was pressed on a bot message.
    CallbackQuery(CallbackQuery),
    /// A user typed an inline query for the bot.
    InlineQuery(InlineQuery),
    /// A raw TL update not mapped to any of the above variants.
    Raw(RawUpdate),
}

// ─── MTProto update container IDs ────────────────────────────────────────────

const ID_UPDATES_TOO_LONG:      u32 = 0xe317af7e;
const ID_UPDATE_SHORT_MESSAGE:  u32 = 0x313bc7f8;
const ID_UPDATE_SHORT_CHAT_MSG: u32 = 0x4d6deea5;
const ID_UPDATE_SHORT:          u32 = 0x78d4dec1;
const ID_UPDATES:               u32 = 0x74ae4240;
const ID_UPDATES_COMBINED:      u32 = 0x725b04c3;

// ─── Parser ──────────────────────────────────────────────────────────────────

/// Parse raw update container bytes into high-level [`Update`] values.
///
/// Returns an empty vector for unknown or unhandled containers.
pub(crate) fn parse_updates(bytes: &[u8]) -> Vec<Update> {
    if bytes.len() < 4 {
        return vec![];
    }
    let cid = u32::from_le_bytes(bytes[..4].try_into().unwrap());

    match cid {
        ID_UPDATES_TOO_LONG => {
            log::warn!("updatesTooLong received — some updates may be missed; call getDifference if gap-free delivery is required");
            vec![]
        }

        // updateShortMessage — single DM
        ID_UPDATE_SHORT_MESSAGE => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::types::UpdateShortMessage::deserialize(&mut cur) {
                Ok(m) => {
                    vec![Update::NewMessage(make_short_dm(m))]
                }
                Err(e) => { log::warn!("updateShortMessage parse error: {e}"); vec![] }
            }
        }

        // updateShortChatMessage — single group message
        ID_UPDATE_SHORT_CHAT_MSG => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::types::UpdateShortChatMessage::deserialize(&mut cur) {
                Ok(m) => {
                    vec![Update::NewMessage(make_short_chat(m))]
                }
                Err(e) => { log::warn!("updateShortChatMessage parse error: {e}"); vec![] }
            }
        }

        // updateShort — wraps a single Update
        ID_UPDATE_SHORT => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::types::UpdateShort::deserialize(&mut cur) {
                Ok(u) => {
                    from_single_update(u.update)
                }
                Err(e) => { log::warn!("updateShort parse error: {e}"); vec![] }
            }
        }

        // updates / updatesCombined — batch of updates
        ID_UPDATES => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::enums::Updates::deserialize(&mut cur) {
                Ok(tl::enums::Updates::Updates(u)) => {
                    u.updates.into_iter().flat_map(from_single_update).collect()
                }
                Err(e) => { log::warn!("Updates parse error: {e}"); vec![] }
                _ => vec![],
            }
        }

        ID_UPDATES_COMBINED => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::enums::Updates::deserialize(&mut cur) {
                Ok(tl::enums::Updates::Combined(u)) => {
                    u.updates.into_iter().flat_map(from_single_update).collect()
                }
                Err(e) => { log::warn!("UpdatesCombined parse error: {e}"); vec![] }
                _ => vec![],
            }
        }

        _ => vec![], // Not an updates container (handled elsewhere by dispatch_body)
    }
}

/// Convert a single `tl::enums::Update` into a `Vec<Update>` (usually 0 or 1 element).
fn from_single_update(upd: tl::enums::Update) -> Vec<Update> {
    use tl::enums::Update::*;
    match upd {
        NewMessage(u) => vec![Update::NewMessage(IncomingMessage::from_raw(u.message))],
        NewChannelMessage(u) => vec![Update::NewMessage(IncomingMessage::from_raw(u.message))],
        EditMessage(u) => vec![Update::MessageEdited(IncomingMessage::from_raw(u.message))],
        EditChannelMessage(u) => vec![Update::MessageEdited(IncomingMessage::from_raw(u.message))],
        DeleteMessages(u) => vec![Update::MessageDeleted(MessageDeletion { message_ids: u.messages, channel_id: None })],
        DeleteChannelMessages(u) => vec![Update::MessageDeleted(MessageDeletion { message_ids: u.messages, channel_id: Some(u.channel_id) })],
        BotCallbackQuery(u) => vec![Update::CallbackQuery(CallbackQuery {
            query_id:        u.query_id,
            user_id:         u.user_id,
            message_id:      Some(u.msg_id),
            chat_instance:   u.chat_instance,
            data_raw:        u.data,
            game_short_name: u.game_short_name,
        })],
        InlineBotCallbackQuery(u) => vec![Update::CallbackQuery(CallbackQuery {
            query_id:        u.query_id,
            user_id:         u.user_id,
            message_id:      None,
            chat_instance:   u.chat_instance,
            data_raw:        u.data,
            game_short_name: u.game_short_name,
        })],
        BotInlineQuery(u) => vec![Update::InlineQuery(InlineQuery {
            query_id: u.query_id,
            user_id:  u.user_id,
            query:    u.query,
            offset:   u.offset,
        })],
        other => {
            // Use the TL constructor ID as the raw update identifier
            let cid = tl_constructor_id(&other);
            vec![Update::Raw(RawUpdate { constructor_id: cid })]
        }
    }
}

/// Extract constructor ID from a `tl::enums::Update` variant.
fn tl_constructor_id(upd: &tl::enums::Update) -> u32 {
    use tl::enums::Update::*;
    match upd {
        UserStatus(_)               => 0x1bfbd823,
        ContactsReset               => 0xdeaf4e67,
        NewEncryptedMessage(_)      => 0x12bcbd9a,
        EncryptedChatTyping(_)      => 0x1710f156,
        Encryption(_)               => 0xb4a2e88d,
        EncryptedMessagesRead(_)    => 0x38fe25b7,
        ChatParticipants(_)         => 0x07761198,
        NewMessage(_)               => 0x1f2b0afd,
        MessageId(_)                => 0x4e90bfd6,
        ReadMessagesContents(_)     => 0x68c13933,
        DeleteMessages(_)           => 0xa20db0e5,
        UserTyping(_)               => 0x5c486927,
        ChatUserTyping(_)           => 0x9a65ea1f,
        ChatParticipantAdd(_)       => 0xea4cb65b,
        ChatParticipantDelete(_)    => 0x6e5f2de1,
        DcOptions(_)                => 0x8e5e9873,
        NotifySettings(_)           => 0xbec268ef,
        ServiceNotification(_)      => 0xebe46819,
        Privacy(_)                  => 0xee3b272a,
        UserPhone(_)                => 0x05492a13,
        ReadHistoryInbox(_)         => 0x9961fd5c,
        ReadHistoryOutbox(_)        => 0x2f2f21bf,
        WebPage(_)                  => 0x7f891213,
        EditMessage(_)              => 0xe40370a3,
        EditChannelMessage(_)       => 0x1b3f4df7,
        NewChannelMessage(_)        => 0x62ba04d9,
        DeleteChannelMessages(_)    => 0xc32d5b12,
        ChannelMessageViews(_)      => 0x98a12b4b,
        BotCallbackQuery(_)         => 0xe9ff1938,
        InlineBotCallbackQuery(_)   => 0x691e9f68,
        BotInlineQuery(_)           => 0x54826690,
        BotInlineSend(_)            => 0x0e48f964,
        _                           => 0x00000000,
    }
}

// ─── Short message helpers ────────────────────────────────────────────────────

fn make_short_dm(m: tl::types::UpdateShortMessage) -> IncomingMessage {
    let msg = tl::types::Message {
        out:               m.out,
        mentioned:         m.mentioned,
        media_unread:      m.media_unread,
        silent:            m.silent,
        post:              false,
        from_scheduled:    false,
        legacy:            false,
        edit_hide:         false,
        pinned:            false,
        noforwards:        false,
        invert_media:      false,
        offline:           false,
        video_processing_pending: false,
        id:                m.id,
        from_id:           Some(tl::enums::Peer::User(tl::types::PeerUser { user_id: m.user_id })),
        peer_id:           tl::enums::Peer::User(tl::types::PeerUser { user_id: m.user_id }),
        saved_peer_id:     None,
        fwd_from:          m.fwd_from,
        via_bot_id:        m.via_bot_id,
        via_business_bot_id: None,
        reply_to:          m.reply_to,
        date:              m.date,
        message:           m.message,
        media:             None,
        reply_markup:      None,
        entities:          m.entities,
        views:             None,
        forwards:          None,
        replies:           None,
        edit_date:         None,
        post_author:       None,
        grouped_id:        None,
        reactions:         None,
        restriction_reason: None,
        ttl_period:        None,
        quick_reply_shortcut_id: None,
        effect:            None,
        factcheck:         None,
        report_delivery_until_date: None,
        paid_message_stars: None,
        suggested_post:    None,
        from_boosts_applied: None,
        paid_suggested_post_stars: false,
        paid_suggested_post_ton: false,
        schedule_repeat_period: None,
        summary_from_language: None,
    };
    IncomingMessage { raw: tl::enums::Message::Message(msg) }
}

fn make_short_chat(m: tl::types::UpdateShortChatMessage) -> IncomingMessage {
    let msg = tl::types::Message {
        out:               m.out,
        mentioned:         m.mentioned,
        media_unread:      m.media_unread,
        silent:            m.silent,
        post:              false,
        from_scheduled:    false,
        legacy:            false,
        edit_hide:         false,
        pinned:            false,
        noforwards:        false,
        invert_media:      false,
        offline:           false,
        video_processing_pending: false,
        id:                m.id,
        from_id:           Some(tl::enums::Peer::User(tl::types::PeerUser { user_id: m.from_id })),
        peer_id:           tl::enums::Peer::Chat(tl::types::PeerChat { chat_id: m.chat_id }),
        saved_peer_id:     None,
        fwd_from:          m.fwd_from,
        via_bot_id:        m.via_bot_id,
        via_business_bot_id: None,
        reply_to:          m.reply_to,
        date:              m.date,
        message:           m.message,
        media:             None,
        reply_markup:      None,
        entities:          m.entities,
        views:             None,
        forwards:          None,
        replies:           None,
        edit_date:         None,
        post_author:       None,
        grouped_id:        None,
        reactions:         None,
        restriction_reason: None,
        ttl_period:        None,
        quick_reply_shortcut_id: None,
        effect:            None,
        factcheck:         None,
        report_delivery_until_date: None,
        paid_message_stars: None,
        suggested_post:    None,
        from_boosts_applied: None,
        paid_suggested_post_stars: false,
        paid_suggested_post_ton: false,
        schedule_repeat_period: None,
        summary_from_language: None,
    };
    IncomingMessage { raw: tl::enums::Message::Message(msg) }
}
