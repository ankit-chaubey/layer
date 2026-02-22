//! High-level update types delivered by [`crate::Client::stream_updates`].
//!
//! Every update the Telegram server pushes is classified into one of the
//! variants of [`Update`].  The raw constructor ID is always available
//! via [`Update::Raw`] for anything not yet wrapped.

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

    /// The peer (chat) this message belongs to.
    pub fn peer_id(&self) -> Option<&tl::enums::Peer> {
        match &self.raw {
            tl::enums::Message::Message(m) => Some(&m.peer_id),
            tl::enums::Message::Service(m) => Some(&m.peer_id),
            _ => None,
        }
    }

    /// The sender peer, if available (not set for anonymous channel posts).
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

    /// Unix timestamp when the message was sent.
    pub fn date(&self) -> i32 {
        match &self.raw {
            tl::enums::Message::Message(m) => m.date,
            tl::enums::Message::Service(m) => m.date,
            _ => 0,
        }
    }

    /// Unix timestamp of the last edit, if the message has been edited.
    pub fn edit_date(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.edit_date,
            _ => None,
        }
    }

    /// `true` if the logged-in user was mentioned in this message.
    pub fn mentioned(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Message(m) => m.mentioned,
            tl::enums::Message::Service(m) => m.mentioned,
            _ => false,
        }
    }

    /// `true` if the message was sent silently (no notification).
    pub fn silent(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Message(m) => m.silent,
            tl::enums::Message::Service(m) => m.silent,
            _ => false,
        }
    }

    /// `true` if this is a channel post (no sender).
    pub fn post(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Message(m) => m.post,
            _ => false,
        }
    }

    /// `true` if this message is currently pinned.
    pub fn pinned(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Message(m) => m.pinned,
            _ => false,
        }
    }

    /// Number of times the message has been forwarded (channels only).
    pub fn forward_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.forwards,
            _ => None,
        }
    }

    /// View count for channel posts.
    pub fn view_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.views,
            _ => None,
        }
    }

    /// Reply count (number of replies in a thread).
    pub fn reply_count(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(m) => {
                m.replies.as_ref().map(|r| match r {
                    tl::enums::MessageReplies::MessageReplies(x) => x.replies,
                })
            }
            _ => None,
        }
    }

    /// ID of the message this one is replying to.
    pub fn reply_to_message_id(&self) -> Option<i32> {
        match &self.raw {
            tl::enums::Message::Message(m) => {
                m.reply_to.as_ref().and_then(|r| match r {
                    tl::enums::MessageReplyHeader::MessageReplyHeader(h) => h.reply_to_msg_id,
                    _ => None,
                })
            }
            _ => None,
        }
    }

    /// Fetch the message that this one is replying to.
    ///
    /// Returns `None` if this message is not a reply or if the peer is unknown.
    /// Unlike [`reply_to_message_id`] this actually performs an API call to
    /// retrieve the full message object.
    ///
    /// [`reply_to_message_id`]: IncomingMessage::reply_to_message_id
    pub async fn reply_to_message(
        &self,
        client: &Client,
    ) -> Result<Option<IncomingMessage>, Error> {
        let reply_id = match self.reply_to_message_id() {
            Some(id) => id,
            None     => return Ok(None),
        };
        let peer = match self.peer_id() {
            Some(p) => p.clone(),
            None    => return Ok(None),
        };
        let msgs = client.get_messages_by_id(peer, &[reply_id]).await?;
        Ok(msgs.into_iter().next())
    }

    /// The message's send time as a [`chrono::DateTime<chrono::Utc>`].
    ///
    /// This is a typed wrapper around the raw `date()` Unix timestamp.
    pub fn date_utc(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::TimeZone;
        let ts = self.date();
        if ts == 0 { return None; }
        chrono::Utc.timestamp_opt(ts as i64, 0).single()
    }

    /// The last edit time as a [`chrono::DateTime<chrono::Utc>`], if edited.
    pub fn edit_date_utc(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::TimeZone;
        self.edit_date().and_then(|ts| chrono::Utc.timestamp_opt(ts as i64, 0).single())
    }

    /// The media attached to this message, if any.
    pub fn media(&self) -> Option<&tl::enums::MessageMedia> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.media.as_ref(),
            _ => None,
        }
    }

    /// Formatting entities (bold, italic, code, links, etc).
    pub fn entities(&self) -> Option<&Vec<tl::enums::MessageEntity>> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.entities.as_ref(),
            _ => None,
        }
    }

    /// Group ID for album messages (multiple media in one).
    pub fn grouped_id(&self) -> Option<i64> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.grouped_id,
            _ => None,
        }
    }

    /// Reply markup (inline keyboards, etc).
    pub fn reply_markup(&self) -> Option<&tl::enums::ReplyMarkup> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.reply_markup.as_ref(),
            _ => None,
        }
    }

    /// Forward info header, if this message was forwarded.
    pub fn forward_header(&self) -> Option<&tl::enums::MessageFwdHeader> {
        match &self.raw {
            tl::enums::Message::Message(m) => m.fwd_from.as_ref(),
            _ => None,
        }
    }

    /// `true` if forwarding this message is restricted.
    pub fn noforwards(&self) -> bool {
        match &self.raw {
            tl::enums::Message::Message(m) => m.noforwards,
            _ => false,
        }
    }

    /// Reply to this message with plain text.
    pub async fn reply(&self, client: &mut Client, text: impl Into<String>) -> Result<(), Error> {
        let peer = match self.peer_id() {
            Some(p) => p.clone(),
            None    => return Err(Error::Deserialize("cannot reply: unknown peer".into())),
        };
        let msg_id = self.id();
        client.send_message_to_peer_ex(peer, &crate::InputMessage::text(text.into())
            .reply_to(Some(msg_id))).await
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
    /// Peer of the chat the user sent the inline query from, if available.
    pub peer:     Option<tl::enums::Peer>,
}

impl InlineQuery {
    /// The text the user typed after the bot username.
    pub fn query(&self) -> &str { &self.query }
}

// ─── InlineSend ──────────────────────────────────────────────────────────────

/// A user chose an inline result and sent it.
#[derive(Debug, Clone)]
pub struct InlineSend {
    pub user_id:  i64,
    pub query:    String,
    pub id:       String,
    /// Message ID of the sent message, if available.
    pub msg_id:   Option<tl::enums::InputBotInlineMessageId>,
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
    /// A user chose an inline result and sent it (bots only).
    InlineSend(InlineSend),
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
pub(crate) fn parse_updates(bytes: &[u8]) -> Vec<Update> {
    if bytes.len() < 4 {
        return vec![];
    }
    let cid = u32::from_le_bytes(bytes[..4].try_into().unwrap());

    match cid {
        ID_UPDATES_TOO_LONG => {
            log::warn!("[layer] updatesTooLong — call client.get_difference() to recover missed updates");
            vec![]
        }

        ID_UPDATE_SHORT_MESSAGE => {
            let mut cur = Cursor::from_slice(&bytes[4..]); // skip constructor prefix
            match tl::types::UpdateShortMessage::deserialize(&mut cur) {
                Ok(m)  => vec![Update::NewMessage(make_short_dm(m))],
                Err(e) => { log::debug!("[layer] updateShortMessage parse error (unknown constructor or newer layer): {e}"); vec![] }
            }
        }

        ID_UPDATE_SHORT_CHAT_MSG => {
            let mut cur = Cursor::from_slice(&bytes[4..]); // skip constructor prefix
            match tl::types::UpdateShortChatMessage::deserialize(&mut cur) {
                Ok(m)  => vec![Update::NewMessage(make_short_chat(m))],
                Err(e) => { log::debug!("[layer] updateShortChatMessage parse error (unknown constructor or newer layer): {e}"); vec![] }
            }
        }

        ID_UPDATE_SHORT => {
            let mut cur = Cursor::from_slice(&bytes[4..]); // skip constructor prefix
            match tl::types::UpdateShort::deserialize(&mut cur) {
                Ok(m)  => from_single_update(m.update),
                Err(e) => { log::debug!("[layer] updateShort parse error (unknown constructor or newer layer): {e}"); vec![] }
            }
        }

        ID_UPDATES => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::enums::Updates::deserialize(&mut cur) {
                Ok(tl::enums::Updates::Updates(u)) => {
                    u.updates.into_iter().flat_map(from_single_update).collect()
                }
                Err(e) => { log::debug!("[layer] Updates parse error (unknown constructor or newer layer): {e}"); vec![] }
                _ => vec![],
            }
        }

        ID_UPDATES_COMBINED => {
            let mut cur = Cursor::from_slice(bytes);
            match tl::enums::Updates::deserialize(&mut cur) {
                Ok(tl::enums::Updates::Combined(u)) => {
                    u.updates.into_iter().flat_map(from_single_update).collect()
                }
                Err(e) => { log::debug!("[layer] UpdatesCombined parse error (unknown constructor or newer layer): {e}"); vec![] }
                _ => vec![],
            }
        }

        _ => vec![],
    }
}

/// Convert a single `tl::enums::Update` into a `Vec<Update>`.
pub fn from_single_update_pub(upd: tl::enums::Update) -> Vec<Update> {
    from_single_update(upd)
}

/// Convert a single `tl::enums::Update` into a `Vec<Update>`.
fn from_single_update(upd: tl::enums::Update) -> Vec<Update> {
    use tl::enums::Update::*;
    match upd {
        NewMessage(u) => vec![Update::NewMessage(IncomingMessage::from_raw(u.message))],
        NewChannelMessage(u) => vec![Update::NewMessage(IncomingMessage::from_raw(u.message))],
        EditMessage(u) => vec![Update::MessageEdited(IncomingMessage::from_raw(u.message))],
        EditChannelMessage(u) => vec![Update::MessageEdited(IncomingMessage::from_raw(u.message))],
        DeleteMessages(u) => vec![Update::MessageDeleted(MessageDeletion {
            message_ids: u.messages,
            channel_id: None,
        })],
        DeleteChannelMessages(u) => vec![Update::MessageDeleted(MessageDeletion {
            message_ids: u.messages,
            channel_id: Some(u.channel_id),
        })],
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
            peer:     None,
        })],
        BotInlineSend(u) => vec![Update::InlineSend(InlineSend {
            user_id: u.user_id,
            query:   u.query,
            id:      u.id,
            msg_id:  u.msg_id,
        })],
        other => {
            let cid = tl_constructor_id(&other);
            vec![Update::Raw(RawUpdate { constructor_id: cid })]
        }
    }
}

/// Extract constructor ID from a `tl::enums::Update` variant.
fn tl_constructor_id(upd: &tl::enums::Update) -> u32 {
    use tl::enums::Update::*;
    match upd {
        AttachMenuBots => 0x17b7a20b,
        AutoSaveSettings => 0xec05b097,
        BotBusinessConnect(_) => 0x8ae5c97a,
        BotCallbackQuery(_) => 0xb9cfc48d,
        BotChatBoost(_) => 0x904dd49c,
        BotChatInviteRequester(_) => 0x11dfa986,
        BotCommands(_) => 0x4d712f2e,
        BotDeleteBusinessMessage(_) => 0xa02a982e,
        BotEditBusinessMessage(_) => 0x7df587c,
        BotInlineQuery(_) => 0x496f379c,
        BotInlineSend(_) => 0x12f12a07,
        BotMenuButton(_) => 0x14b85813,
        BotMessageReaction(_) => 0xac21d3ce,
        BotMessageReactions(_) => 0x9cb7759,
        BotNewBusinessMessage(_) => 0x9ddb347c,
        BotPrecheckoutQuery(_) => 0x8caa9a96,
        BotPurchasedPaidMedia(_) => 0x283bd312,
        BotShippingQuery(_) => 0xb5aefd7d,
        BotStopped(_) => 0xc4870a49,
        BotWebhookJson(_) => 0x8317c0c3,
        BotWebhookJsonquery(_) => 0x9b9240a6,
        BusinessBotCallbackQuery(_) => 0x1ea2fda7,
        Channel(_) => 0x635b4c09,
        ChannelAvailableMessages(_) => 0xb23fc698,
        ChannelMessageForwards(_) => 0xd29a27f4,
        ChannelMessageViews(_) => 0xf226ac08,
        ChannelParticipant(_) => 0x985d3abb,
        ChannelReadMessagesContents(_) => 0x25f324f7,
        ChannelTooLong(_) => 0x108d941f,
        ChannelUserTyping(_) => 0x8c88c923,
        ChannelViewForumAsMessages(_) => 0x7b68920,
        ChannelWebPage(_) => 0x2f2ba99f,
        Chat(_) => 0xf89a6a4e,
        ChatDefaultBannedRights(_) => 0x54c01850,
        ChatParticipant(_) => 0xd087663a,
        ChatParticipantAdd(_) => 0x3dda5451,
        ChatParticipantAdmin(_) => 0xd7ca61a2,
        ChatParticipantDelete(_) => 0xe32f3d77,
        ChatParticipants(_) => 0x7761198,
        ChatUserTyping(_) => 0x83487af0,
        Config => 0xa229dd06,
        ContactsReset => 0x7084a7be,
        DcOptions(_) => 0x8e5e9873,
        DeleteChannelMessages(_) => 0xc32d5b12,
        DeleteGroupCallMessages(_) => 0x3e85e92c,
        DeleteMessages(_) => 0xa20db0e5,
        DeleteQuickReply(_) => 0x53e6f1ec,
        DeleteQuickReplyMessages(_) => 0x566fe7cd,
        DeleteScheduledMessages(_) => 0xf2a71983,
        DialogFilter(_) => 0x26ffde7d,
        DialogFilterOrder(_) => 0xa5d72105,
        DialogFilters => 0x3504914f,
        DialogPinned(_) => 0x6e6fe51c,
        DialogUnreadMark(_) => 0xb658f23e,
        DraftMessage(_) => 0xedfc111e,
        EditChannelMessage(_) => 0x1b3f4df7,
        EditMessage(_) => 0xe40370a3,
        EmojiGameInfo(_) => 0xfb9c547a,
        EncryptedChatTyping(_) => 0x1710f156,
        EncryptedMessagesRead(_) => 0x38fe25b7,
        Encryption(_) => 0xb4a2e88d,
        FavedStickers => 0xe511996d,
        FolderPeers(_) => 0x19360dc0,
        GeoLiveViewed(_) => 0x871fb939,
        GroupCall(_) => 0x9d2216e0,
        GroupCallChainBlocks(_) => 0xa477288f,
        GroupCallConnection(_) => 0xb783982,
        GroupCallEncryptedMessage(_) => 0xc957a766,
        GroupCallMessage(_) => 0xd8326f0d,
        GroupCallParticipants(_) => 0xf2ebdb4e,
        InlineBotCallbackQuery(_) => 0x691e9052,
        LangPack(_) => 0x56022f4d,
        LangPackTooLong(_) => 0x46560264,
        LoginToken => 0x564fe691,
        MessageExtendedMedia(_) => 0xd5a41724,
        MessageId(_) => 0x4e90bfd6,
        MessagePoll(_) => 0xaca1657b,
        MessagePollVote(_) => 0x24f40e77,
        MessageReactions(_) => 0x1e297bfa,
        MonoForumNoPaidException(_) => 0x9f812b08,
        MoveStickerSetToTop(_) => 0x86fccf85,
        NewAuthorization(_) => 0x8951abef,
        NewChannelMessage(_) => 0x62ba04d9,
        NewEncryptedMessage(_) => 0x12bcbd9a,
        NewMessage(_) => 0x1f2b0afd,
        NewQuickReply(_) => 0xf53da717,
        NewScheduledMessage(_) => 0x39a51dfb,
        NewStickerSet(_) => 0x688a30aa,
        NewStoryReaction(_) => 0x1824e40b,
        NotifySettings(_) => 0xbec268ef,
        PaidReactionPrivacy(_) => 0x8b725fce,
        PeerBlocked(_) => 0xebe07752,
        PeerHistoryTtl(_) => 0xbb9bb9a5,
        PeerLocated(_) => 0xb4afcfb0,
        PeerSettings(_) => 0x6a7e7366,
        PeerWallpaper(_) => 0xae3f101d,
        PendingJoinRequests(_) => 0x7063c3db,
        PhoneCall(_) => 0xab0f6b1e,
        PhoneCallSignalingData(_) => 0x2661bf09,
        PinnedChannelMessages(_) => 0x5bb98608,
        PinnedDialogs(_) => 0xfa0f3ca2,
        PinnedForumTopic(_) => 0x683b2c52,
        PinnedForumTopics(_) => 0xdef143d0,
        PinnedMessages(_) => 0xed85eab5,
        PinnedSavedDialogs(_) => 0x686c85a6,
        Privacy(_) => 0xee3b272a,
        PtsChanged => 0x3354678f,
        QuickReplies(_) => 0xf9470ab2,
        QuickReplyMessage(_) => 0x3e050d0f,
        ReadChannelDiscussionInbox(_) => 0xd6b19546,
        ReadChannelDiscussionOutbox(_) => 0x695c9e7c,
        ReadChannelInbox(_) => 0x922e6e10,
        ReadChannelOutbox(_) => 0xb75f99a9,
        ReadFeaturedEmojiStickers => 0xfb4c496c,
        ReadFeaturedStickers => 0x571d2742,
        ReadHistoryInbox(_) => 0x9e84bc99,
        ReadHistoryOutbox(_) => 0x2f2f21bf,
        ReadMessagesContents(_) => 0xf8227181,
        ReadMonoForumInbox(_) => 0x77b0e372,
        ReadMonoForumOutbox(_) => 0xa4a79376,
        ReadStories(_) => 0xf74e932b,
        RecentEmojiStatuses => 0x30f443db,
        RecentReactions => 0x6f7863f4,
        RecentStickers => 0x9a422c20,
        SavedDialogPinned(_) => 0xaeaf9e74,
        SavedGifs => 0x9375341e,
        SavedReactionTags => 0x39c67432,
        SavedRingtones => 0x74d8be99,
        SentPhoneCode(_) => 0x504aa18f,
        SentStoryReaction(_) => 0x7d627683,
        ServiceNotification(_) => 0xebe46819,
        SmsJob(_) => 0xf16269d4,
        StarGiftAuctionState(_) => 0x48e246c2,
        StarGiftAuctionUserState(_) => 0xdc58f31e,
        StarGiftCraftFail => 0xac072444,
        StarsBalance(_) => 0x4e80a379,
        StarsRevenueStatus(_) => 0xa584b019,
        StickerSets(_) => 0x31c24808,
        StickerSetsOrder(_) => 0xbb2d201,
        StoriesStealthMode(_) => 0x2c084dc1,
        Story(_) => 0x75b3b798,
        StoryId(_) => 0x1bf335b9,
        Theme(_) => 0x8216fba3,
        TranscribedAudio(_) => 0x84cd5a,
        User(_) => 0x20529438,
        UserEmojiStatus(_) => 0x28373599,
        UserName(_) => 0xa7848924,
        UserPhone(_) => 0x5492a13,
        UserStatus(_) => 0xe5bdf8de,
        UserTyping(_) => 0x2a17bf5c,
        WebPage(_) => 0x7f891213,
        WebViewResultSent(_) => 0x1592b79d,
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
