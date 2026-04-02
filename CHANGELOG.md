# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.4.5] — 2026-04-02

### Added

#### Session
- **`StringSessionBackend`** — portable, string-encoded session backend. Encode an entire session as a base64 string, store it wherever you like (env var, DB column, clipboard), then restore it on the next run.
- **`export_session_string()`** — new `Client` method that serialises the live session (auth key + DC + peer cache) to a printable string. Complements `StringSessionBackend`.
- **`LibSqlBackend`** — session backend backed by a libsql/Turso database. Enable with `features = ["libsql-session"]`. Drop-in replacement for `SqliteBackend` for remote or embedded databases.

#### Updates
- **`Update::ChatAction`** — new typed update variant wrapping `ChatActionUpdate`. Fires when a user starts/stops typing, uploading, or recording.
- **`Update::UserStatus`** — new typed variant wrapping `UserStatusUpdate`. Fires when a contact's online status changes.
- **`sync_update_state()`** — forces an immediate `updates.getState` round-trip and reconciles local pts/seq counters. Useful after long disconnects.

#### Client
- **`Client::with_string_session(s)`** — constructor shorthand for connecting with a `StringSessionBackend`.
- **`disconnect()`** is now part of the primary documented API surface.

#### Docs
- Comprehensive rewrite of all documentation pages to reflect the full 0.4.x API surface.
- New pages: Session Backends, Search, Reactions, Admin Rights, Typing Guard.
- Every code example audited and updated for 0.4.5 API.
- `SUMMARY.md` expanded with new sections.

### Fixed
- `send_chat_action` with `top_msg_id` (forum topics) no longer panics on basic groups.
- `iter_participants` no longer silently truncates at 200 members — pagination is now correct.
- `GlobalSearchBuilder::fetch` no longer returns duplicates when total exceeds per-page limit.
- `DownloadIter` last partial chunk was sometimes zero-padded; now trimmed to exact file size.
- `ban_participant` with temporary timestamp no longer overflows on 32-bit targets.
- `answer_inline_query` with empty results no longer triggers RPC 400 — empty response sent correctly.
- `PossibleGapBuffer` memory leak: buffered updates now released when gap is resolved via `getDifference`.

### Changed
- `Update` enum is now `#[non_exhaustive]` — match arms must include `_ => {}` fallback.
- Minimum supported Rust edition remains **2024**.
- `layer-tl-types` LAYER constant stays at **224** (no schema change in this patch).

---

## [0.4.0] — 2026-04-01

### Added / Changed

#### MTProto core
- Implement `bad_msg` auto-resend — messages flagged by `bad_msg_notification` are automatically re-queued and retransmitted.
- Add `seq_no` correction for error codes 32 and 33 — client-side sequence counter is adjusted and the offending request is re-sent.
- Implement outgoing `MsgsAck` handling — acknowledged message IDs are tracked and periodically flushed.
- Add message container batching — multiple pending requests are coalesced into a single `msg_container` frame where possible.
- Add `gzip_packed` support for large outgoing requests — payloads above the threshold are compressed before sending.
- Implement `future_salts` fetch — the client proactively requests future salts and rotates them before expiry.
- Add `time_offset` correction — clock skew between client and server is measured and applied to all outgoing `msg_id` values.
- Implement `msg_resend_req` handling — server-requested message resends are fulfilled from the sent-body cache.

#### Connection / session
- Track sent message bodies for potential resend.
- Add `pending_ack` system — received content-related messages generate acknowledgements that are batched and sent on a timer.
- Improve session lifecycle — `new_session_created` resets PTS/seq state; `DestroySession` is handled gracefully.
- Add `disconnect()` method for graceful teardown without a `ShutdownToken`.

#### Update engine
- Implement pts / seq / qts tracking with gap detection.
- Add per-channel `getChannelDifference` loop for catching up missed channel updates.
- Implement update deadline detection — stale update sequences trigger a `getDifference` after a configurable timeout.
- Add `PossibleGapBuffer` — updates that arrive out of order are held until the gap is confirmed or filled.

#### Client API
- `send_reaction(peer, msg_id, reaction)` — send a message reaction.
- `set_admin_rights(peer, user, rights)` — promote a user with an `AdminRightsBuilder`.
- `set_banned_rights(peer, user, rights)` — restrict a user with a `BanRights` builder.
- `iter_participants(peer)` — lazy async iterator over all chat/channel members.
- `get_profile_photos(user)` — fetch a user's profile photo list.
- `get_permissions(peer, user)` — retrieve the effective permissions of a user in a chat.
- `edit_inline_message(inline_msg_id, message)` — edit a message sent via inline mode.

#### Search
- Introduce `SearchBuilder` — fluent builder for per-peer message search with date filters, limits, and peer filters.
- Introduce `GlobalSearchBuilder` — search across all dialogs simultaneously.

#### Updates
- Extended `IncomingMessage` accessors: `date()`, `via_bot_id()`, `grouped_id()`, `reactions()`, `reply_markup()`.
- Add `markdown_text()` and `html_text()` — return message text with entities re-encoded as Markdown / HTML.
- Improved `CallbackQuery` fields: expose `game_short_name`, `chat_instance`, and `data` as typed helpers.
- Add `InlineSend::edit_message()` — directly edit the message that triggered an inline result.

#### Keyboard
- Add missing button constructors: `request_phone`, `request_geo`, `request_poll`, `request_quiz`, `game`, `buy`, `copy_text`.

#### Typing
- Add topic typing support — `send_chat_action` now accepts an optional `top_msg_id` for forum topics.

#### Internal
- Various refactors and architecture improvements across the crate.

---

## [0.2.3] — 2025-03-09

### Added

#### Ergonomics
- **`dispatch!` macro** — pattern-match updates without giant `match` blocks.
- **`keyboard` module** — `InlineKeyboard` / `ReplyKeyboard` / `Button` builder API.
- **`pub use layer_tl_types as tl`** — write `use layer_client::tl` instead of a separate dep.
- **`download_media_to_file(location, path)`** — convenience download-to-path wrapper.
- **`DialogIter::total()` / `MessageIter::total()`** — server-reported total count.

#### Reliability
- **Graceful shutdown via `ShutdownToken`** — `Client::connect` returns `(Client, ShutdownToken)`.
- **`catch_up` config flag** — replays missed updates via `getDifference` on connect.
- **`PingDelayDisconnect` keepalive** — 60-second ping with 75-second server disconnect delay.
- **Exponential backoff reconnect** — 500 ms → 1 s → … → 30 s cap, indefinitely.
- **`signal_network_restored()`** — skip backoff and reconnect immediately.
- **Pending RPCs fail-fast on disconnect** — callers receive `InvocationError::Io` immediately.

### Changed
- `Client::connect` return type changed from `Result<Client, _>` to `Result<(Client, ShutdownToken), _>`.

---

## [0.2.2] — 2025-02-15

### Added
- `InMemoryBackend` session storage.
- `SqliteBackend` behind `features = ["sqlite-session"]`.
- SOCKS5 proxy support via `Config::socks5`.
- Obfuscated MTProto transport (`TransportKind::Obfuscated`).
- HTML ↔ entity parsing (`parsers::parse_html`, `parsers::generate_html`).
- Markdown ↔ entity parsing (`parsers::parse_markdown`, `parsers::generate_markdown`).
- PTS gap detection and `get_difference` recovery (`pts` module).
- Multi-DC connection pool (`dc_pool` module).
- Paginated `DialogIter` and `MessageIter`.
- `TypingGuard` RAII typing indicator.
- Scheduled messages: `get_scheduled_messages`, `delete_scheduled_messages`.
- Media upload/download: `upload_file`, `send_file`, `send_album`, `download_media`, `iter_download`.
- `inline_iter` for inline bot results.
- `participants` iterator.

---

## [0.2.1] — 2025-01-20

### Added
- Initial public release on crates.io.
- User login (phone + code + 2FA SRP) and bot token login.
- Peer access-hash caching.
- FLOOD_WAIT auto-retry with configurable `RetryPolicy`.
- Typed async update stream: `NewMessage`, `MessageEdited`, `MessageDeleted`,
  `CallbackQuery`, `InlineQuery`, `InlineSend`, `Raw`.
- Send / edit / delete / forward / pin messages.
- Search messages (per-chat and global).
- DC migration and session persistence.
