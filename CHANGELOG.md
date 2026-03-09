# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.3] — 2025-03-09

### Added

#### Ergonomics
- **`dispatch!` macro** — pattern-match updates without giant `match` blocks.
  Each arm is `VariantName(binding) [if guard] => { body }`. Expands to a plain
  `match` — zero overhead.
  ```rust
  dispatch!(client, update,
      NewMessage(msg) if !msg.outgoing() => { /* … */ },
      CallbackQuery(cb) => { client.answer_callback_query(…).await?; },
      _ => {}
  );
  ```

- **`keyboard` module** — `InlineKeyboard` / `ReplyKeyboard` / `Button` builder
  API. Replaces 20+ lines of raw TL with a fluent chain:
  ```rust
  let kb = InlineKeyboard::new()
      .row([Button::callback("✅ Yes", b"yes"), Button::callback("❌ No", b"no")])
      .row([Button::url("Docs", "https://docs.rs/layer-client")]);
  let msg = InputMessage::text("Choose:").keyboard(kb);
  ```
  `InputMessage::keyboard()` accepts anything `Into<ReplyMarkup>`, including
  `InlineKeyboard` and `ReplyKeyboard` directly.

- **`pub use layer_tl_types as tl`** — users can now write `use layer_client::tl`
  instead of adding a separate `layer-tl-types` dependency.

- **`download_media_to_file(location, path)`** — convenience wrapper that
  downloads a media attachment and writes it directly to a file path.

- **`DialogIter::total()` / `MessageIter::total()`** — returns the server-reported
  total count from the first page response (`messages.DialogsSlice` /
  `messages.Slice` / `messages.ChannelMessages`). `None` until the first `next()`
  call.

#### Reliability
- **Graceful shutdown via `ShutdownToken`** — `Client::connect` now returns
  `(Client, ShutdownToken)`. Calling `shutdown.cancel()` cleanly drains all
  pending RPC calls (they receive `InvocationError::Dropped`) and stops the
  reader task. Prevents data loss on Ctrl+C.
  ```rust
  let (client, shutdown) = Client::connect(config).await?;
  // later, e.g. in a signal handler:
  shutdown.cancel();
  ```

- **`catch_up` config flag** — setting `Config { catch_up: true, .. }` replays
  any missed updates via `updates.getDifference` immediately after connecting.
  Equivalent to grammers' `UpdatesConfiguration { catch_up: true }`.

- **`PingDelayDisconnect` keepalive** — the reader now sends
  `ping_delay_disconnect { disconnect_delay: 75 }` every 60 seconds instead of
  a plain `Ping`. This instructs Telegram to send a clean EOF after 75 s of
  silence, turning silent stale sockets into detectable errors.

- **Exponential backoff reconnect** — on disconnect the reader retries with
  500 ms → 1 s → 2 s → … → 30 s cap, indefinitely. Previously a single failed
  reconnect attempt killed the reader task permanently.

- **`signal_network_restored()`** — new `Client` method; call it from Android
  `ConnectivityManager` or iOS `NWPathMonitor` callbacks to skip the backoff
  delay and attempt reconnect immediately.

- **Pending RPCs fail-fast on disconnect** — callers receive
  `InvocationError::Io(ConnectionReset)` immediately when the connection drops
  (instead of waiting for the 30 s timeout). `AutoSleep` retries them once the
  reconnect succeeds.

### Changed
- **`Client::connect` return type** changed from `Result<Client, _>` to
  `Result<(Client, ShutdownToken), _>`. Update call sites:
  ```rust
  // before
  let client = Client::connect(config).await?;
  // after
  let (client, _shutdown) = Client::connect(config).await?;
  ```

### Fixed
- `#[non_exhaustive]` was already present on `Update`; confirmed correct.

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
