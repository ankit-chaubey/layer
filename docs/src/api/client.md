# Client Methods — Full Reference

All methods on `Client`. Every method is `async` and returns `Result<T, InvocationError>` unless noted.

---

## Connection & Session

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">Client::connect(config: Config) → Result&lt;Client, InvocationError&gt;</span>
</div>
<div class="api-card-body">
Opens a TCP connection to Telegram, performs the full 3-step DH key exchange, and loads any existing session from disk. This is the entry point for all layer usage.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.is_authorized() → Result&lt;bool, InvocationError&gt;</span>
</div>
<div class="api-card-body">
Returns <code>true</code> if the current session has a logged-in user or bot. Use this to skip the login flow on subsequent runs.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.save_session() → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">
Writes the current session (auth key + DC info + peer cache) to disk. Call this after a successful login.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.sign_out() → Result&lt;bool, InvocationError&gt;</span>
</div>
<div class="api-card-body">
Revokes the auth key on Telegram's servers and deletes the local session file. The returned <code>bool</code> indicates whether session teardown was confirmed.
</div>
</div>

---

## Authentication

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.request_login_code(phone: &str) → Result&lt;LoginToken, InvocationError&gt;</span>
</div>
<div class="api-card-body">Sends a verification code to <code>phone</code> via SMS or Telegram app. Returns a <code>LoginToken</code> that must be passed to <code>sign_in</code>.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.sign_in(token: &LoginToken, code: &str) → Result&lt;String, SignInError&gt;</span>
</div>
<div class="api-card-body">
Submits the verification code. Returns the user's full name on success, or <code>SignInError::PasswordRequired(PasswordToken)</code> if 2FA is enabled.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.check_password(token: PasswordToken, password: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Completes the SRP 2FA verification. The password is never transmitted — only a zero-knowledge proof.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.bot_sign_in(token: &str) → Result&lt;String, InvocationError&gt;</span>
</div>
<div class="api-card-body">Logs in using a bot token from @BotFather. Returns the bot's username.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.get_me() → Result&lt;tl::types::User, InvocationError&gt;</span>
</div>
<div class="api-card-body">Fetches the full <code>User</code> object for the logged-in account. Contains id, username, first_name, last_name, phone, bot flag, verified flag, and more.</div>
</div>

---

## Messaging

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.send_message(peer: &str, text: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">
Send a plain-text message. <code>peer</code> can be <code>"me"</code>, <code>"@username"</code>, or a numeric ID string. For rich formatting, use <code>send_message_to_peer_ex</code>.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.send_to_self(text: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Sends a message to your own Saved Messages. Shorthand for <code>send_message("me", text)</code>.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.send_message_to_peer(peer: Peer, text: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Send a plain text message to a resolved <code>tl::enums::Peer</code>.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.send_message_to_peer_ex(peer: Peer, msg: &InputMessage) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">
Full-featured send with the <a href="./input-message.md"><code>InputMessage</code></a> builder — supports markdown entities, reply-to, inline keyboard, scheduled date, silent flag, and more.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.edit_message(peer: Peer, message_id: i32, new_text: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Edit the text of a previously sent message. Only works on messages sent by the logged-in account.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.delete_messages(ids: Vec&lt;i32&gt;, revoke: bool) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Delete messages by ID. <code>revoke: true</code> deletes for everyone; <code>false</code> deletes only locally.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.forward_messages(from: Peer, to: Peer, ids: Vec&lt;i32&gt;) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Forward messages from one chat to another.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.get_messages(peer: Peer, limit: i32, offset_id: i32) → Result&lt;Vec&lt;tl::enums::Message&gt;, InvocationError&gt;</span>
</div>
<div class="api-card-body">Fetch message history. <code>offset_id = 0</code> starts from the newest message. Returns up to <code>limit</code> messages in reverse chronological order.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.get_messages_by_id(peer: Peer, ids: Vec&lt;i32&gt;) → Result&lt;Vec&lt;tl::enums::Message&gt;, InvocationError&gt;</span>
</div>
<div class="api-card-body">Fetch specific messages by their IDs.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.pin_message(peer: Peer, message_id: i32, silent: bool) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Pin a message. <code>silent: true</code> pins without sending a notification.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.unpin_message(peer: Peer, message_id: i32) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Unpin a specific message.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.unpin_all_messages(peer: Peer) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Remove all pinned messages in a chat.</div>
</div>

---

## Search

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.search_messages(peer: Peer, query: &str, limit: i32) → Result&lt;Vec&lt;tl::enums::Message&gt;, InvocationError&gt;</span>
</div>
<div class="api-card-body">Search for messages in a specific chat matching <code>query</code>.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.search_global(query: &str, limit: i32) → Result&lt;Vec&lt;tl::enums::Message&gt;, InvocationError&gt;</span>
</div>
<div class="api-card-body">Search across all chats and public channels.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.search_peer(query: &str) → Result&lt;Vec&lt;tl::enums::Peer&gt;, InvocationError&gt;</span>
</div>
<div class="api-card-body">Search contacts, dialogs, and global results for a username or name prefix.</div>
</div>

---

## Dialogs & Chats

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.get_dialogs(limit: i32) → Result&lt;Vec&lt;Dialog&gt;, InvocationError&gt;</span>
</div>
<div class="api-card-body">Returns the most recent <code>limit</code> dialogs (conversations). Each <code>Dialog</code> has <code>title()</code>, <code>peer()</code>, <code>unread_count()</code>, <code>top_message()</code>.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-fn">fn</span>
<span class="api-card-sig">client.iter_dialogs() → DialogIter</span>
</div>
<div class="api-card-body">Returns a paginating iterator over all dialogs. Call <code>iter.next(&client).await</code> to get one dialog at a time, automatically fetching more pages.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-fn">fn</span>
<span class="api-card-sig">client.iter_messages(peer: Peer) → MessageIter</span>
</div>
<div class="api-card-body">Returns a paginating iterator over all messages in a chat from newest to oldest.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.mark_as_read(peer: Peer) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Marks all messages in the chat as read.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.delete_dialog(peer: Peer) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Removes the dialog from the chat list (does not delete messages from the server).</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.join_chat(peer: Peer) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Join a public group or channel.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.accept_invite_link(link: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Join via a <code>t.me/+hash</code> invite link.</div>
</div>

---

## Bot-specific

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.answer_callback_query(query_id: i64, text: Option&lt;&str&gt;, alert: bool) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">
Must be called in response to every <code>CallbackQuery</code>. <code>text</code> is the notification shown to the user. <code>alert: true</code> shows it as a modal alert; <code>false</code> shows it as a brief toast.
</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.answer_inline_query(query_id: i64, results: Vec&lt;InputBotInlineResult&gt;, cache_time: i32, is_personal: bool, next_offset: Option&lt;&str&gt;) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Respond to an <code>InlineQuery</code> with a list of results. <code>cache_time</code> is seconds to cache results (300 = 5 min). <code>is_personal: true</code> disables shared caching.</div>
</div>

---

## Reactions & Actions

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.send_reaction(peer: Peer, message_id: i32, reaction: &str) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Add a reaction to a message. <code>reaction</code> is an emoji string like <code>"👍"</code>. Pass an empty string to remove your reaction.</div>
</div>

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.send_chat_action(peer: Peer, action: ChatAction) → Result&lt;(), InvocationError&gt;</span>
</div>
<div class="api-card-body">Show a typing indicator or other status. Actions: <code>Typing</code>, <code>UploadPhoto</code>, <code>RecordVideo</code>, <code>UploadVideo</code>, <code>RecordAudio</code>, <code>UploadAudio</code>, <code>UploadDocument</code>, <code>GeoLocation</code>, <code>ChooseContact</code>.</div>
</div>

---

## Peer Resolution

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.resolve_peer(peer: &str) → Result&lt;tl::enums::Peer, InvocationError&gt;</span>
</div>
<div class="api-card-body">
Resolve a string to a <code>Peer</code>. Supported formats:
<ul>
<li><code>"me"</code> — your own account</li>
<li><code>"@username"</code> — any public username</li>
<li><code>"123456789"</code> — numeric user/chat/channel ID</li>
</ul>
Also caches the access hash for future API calls.
</div>
</div>

---

## Raw API

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-async">async</span>
<span class="api-card-sig">client.invoke&lt;R: RemoteCall&gt;(req: &R) → Result&lt;R::Return, InvocationError&gt;</span>
</div>
<div class="api-card-body">Call any Telegram API function directly. <code>R</code> is a struct from <code>layer_tl_types::functions</code>. See <a href="../advanced/raw-api.md">Raw API Access</a>.</div>
</div>

---

## Updates

<div class="api-card">
<div class="api-card-header">
<span class="api-badge api-badge-fn">fn</span>
<span class="api-card-sig">client.stream_updates() → UpdateStream</span>
</div>
<div class="api-card-body">Returns an <code>UpdateStream</code>. Call <code>.next().await</code> to receive the next <code>Update</code>. The stream never ends unless the connection is dropped.</div>
</div>
