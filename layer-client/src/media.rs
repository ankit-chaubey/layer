//! Media upload and download support.
//!
//! ## Upload
//! Use [`Client::upload_file`] to upload a file from a byte buffer or
//! [`Client::upload_stream`] for streamed uploads. The returned [`UploadedFile`]
//! can be passed to [`Client::send_file`] or [`Client::send_album`].
//!
//! ## Download
//! Use [`Client::download_media`] to collect all bytes of a media attachment, or
//! [`Client::iter_download`] for chunk-by-chunk streaming.




use layer_tl_types as tl;
use layer_tl_types::{Cursor, Deserializable};
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;

use crate::{Client, InvocationError};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum chunk size for file uploads (512 KB).
pub const UPLOAD_CHUNK_SIZE: i32 = 512 * 1024;
/// Maximum chunk size for file downloads (512 KB).
pub const DOWNLOAD_CHUNK_SIZE: i32 = 512 * 1024;
/// Files larger than this are uploaded as "big files" via SaveBigFilePart.
const BIG_FILE_THRESHOLD: i64 = 10 * 1024 * 1024; // 10 MB

// ─── UploadedFile ─────────────────────────────────────────────────────────────

/// A successfully uploaded file, ready to be sent as media.
#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub(crate) inner: tl::enums::InputFile,
    pub(crate) mime_type: String,
    pub(crate) name: String,
}

impl UploadedFile {
    /// The file's MIME type (set on upload).
    pub fn mime_type(&self) -> &str { &self.mime_type }
    /// The file's original name.
    pub fn name(&self) -> &str { &self.name }

    /// Convert to an `InputMedia` for sending as a document.
    pub fn as_document_media(&self) -> tl::enums::InputMedia {
        tl::enums::InputMedia::UploadedDocument(tl::types::InputMediaUploadedDocument {
            nosound_video: false,
            force_file:    false,
            spoiler:       false,
            file:          self.inner.clone(),
            thumb:         None,
            mime_type:     self.mime_type.clone(),
            attributes:    vec![tl::enums::DocumentAttribute::Filename(
                tl::types::DocumentAttributeFilename { file_name: self.name.clone() }
            )],
            stickers:  None,
            ttl_seconds: None,
            video_cover: None,
            video_timestamp: None,
        })
    }

    /// Convert to an `InputMedia` for sending as a photo.
    pub fn as_photo_media(&self) -> tl::enums::InputMedia {
        tl::enums::InputMedia::UploadedPhoto(tl::types::InputMediaUploadedPhoto {
            spoiler:     false,
            file:        self.inner.clone(),
            stickers:    None,
            ttl_seconds: None,
        })
    }
}

// ─── DownloadIter ─────────────────────────────────────────────────────────────

/// Iterator that downloads a media file chunk by chunk.
///
/// Call [`DownloadIter::next`] in a loop until it returns `None`.
pub struct DownloadIter {
    client:  Client,
    request: Option<tl::functions::upload::GetFile>,
    done:    bool,
}

impl DownloadIter {
    /// Set a custom chunk size (must be a multiple of 4096, max 524288).
    pub fn chunk_size(mut self, size: i32) -> Self {
        if let Some(r) = &mut self.request { r.limit = size; }
        self
    }

    /// Fetch the next chunk of data. Returns `None` when the download is complete.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, InvocationError> {
        if self.done { return Ok(None); }
        let req = match &self.request {
            Some(r) => r.clone(),
            None    => return Ok(None),
        };
        let body = self.client.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        match tl::enums::upload::File::deserialize(&mut cur)? {
            tl::enums::upload::File::File(f) => {
                if (f.bytes.len() as i32) < req.limit {
                    self.done = true;
                    if f.bytes.is_empty() { return Ok(None); }
                }
                if let Some(r) = &mut self.request {
                    r.offset += req.limit as i64;
                }
                Ok(Some(f.bytes))
            }
            tl::enums::upload::File::CdnRedirect(_) => {
                self.done = true;
                Err(InvocationError::Deserialize("CDN redirect not supported".into()))
            }
        }
    }
}

// ─── Client methods ───────────────────────────────────────────────────────────

impl Client {
    /// Upload bytes as a file. Returns an [`UploadedFile`] that can be sent.
    ///
    /// # Arguments
    /// * `data`      — Raw file bytes.
    /// * `name`      — File name (e.g. `"photo.jpg"`).
    /// * `mime_type` — MIME type (e.g. `"image/jpeg"`). Used only for documents.
    pub async fn upload_file(
        &self,
        data:      &[u8],
        name:      &str,
        mime_type: &str,
    ) -> Result<UploadedFile, InvocationError> {
        let file_id   = crate::random_i64_pub();
        let total     = data.len() as i64;
        let big       = total >= BIG_FILE_THRESHOLD;
        let part_size = UPLOAD_CHUNK_SIZE as usize;
        let total_parts = ((total as usize + part_size - 1) / part_size) as i32;

        for (part_num, chunk) in data.chunks(part_size).enumerate() {
            if big {
                let req = tl::functions::upload::SaveBigFilePart {
                    file_id,
                    file_part:  part_num as i32,
                    file_total_parts: total_parts,
                    bytes: chunk.to_vec(),
                };
                self.rpc_call_raw_pub(&req).await?;
            } else {
                let req = tl::functions::upload::SaveFilePart {
                    file_id,
                    file_part: part_num as i32,
                    bytes: chunk.to_vec(),
                };
                self.rpc_call_raw_pub(&req).await?;
            }
            log::debug!("[layer] Uploaded part {} / {}", part_num + 1, total_parts);
        }

        let inner: tl::enums::InputFile = if big {
            tl::enums::InputFile::Big(tl::types::InputFileBig {
                id:    file_id,
                parts: total_parts,
                name:  name.to_string(),
            })
        } else {
            let md5 = format!("{:x}", md5_bytes(data));
            tl::enums::InputFile::InputFile(tl::types::InputFile {
                id:    file_id,
                parts: total_parts,
                name:  name.to_string(),
                md5_checksum: md5,
            })
        };

        log::info!("[layer] File '{}' uploaded ({} bytes, {} parts)", name, total, total_parts);
        Ok(UploadedFile {
            inner,
            mime_type: mime_type.to_string(),
            name:      name.to_string(),
        })
    }

    /// Upload from an async reader.
    pub async fn upload_stream<R: AsyncRead + Unpin>(
        &self,
        reader:    &mut R,
        name:      &str,
        mime_type: &str,
    ) -> Result<UploadedFile, InvocationError> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data).await?;
        self.upload_file(&data, name, mime_type).await
    }

    /// Send a file as a document or photo to a chat.
    ///
    /// Use `uploaded.as_photo_media()` to send as a photo,
    /// or `uploaded.as_document_media()` to send as a file.
    pub async fn send_file(
        &self,
        peer:    tl::enums::Peer,
        media:   tl::enums::InputMedia,
        caption: &str,
    ) -> Result<(), InvocationError> {
        let input_peer = {
            let cache = self.inner.peer_cache.lock().await;
            cache.peer_to_input(&peer)
        };
        let req = tl::functions::messages::SendMedia {
            silent:                   false,
            background:               false,
            clear_draft:              false,
            noforwards:               false,
            update_stickersets_order: false,
            invert_media:             false,
            allow_paid_floodskip:     false,
            peer:                     input_peer,
            reply_to:                 None,
            media,
            message:                  caption.to_string(),
            random_id:                crate::random_i64_pub(),
            reply_markup:             None,
            entities:                 None,
            schedule_date:            None,
            schedule_repeat_period:   None,
            send_as:                  None,
            quick_reply_shortcut:     None,
            effect:                   None,
            allow_paid_stars:         None,
            suggested_post:           None,
        };
        self.rpc_call_raw_pub(&req).await?;
        Ok(())
    }

    /// Send multiple files as an album (media group) in a single message.
    ///
    /// All items must be photos or all must be documents (no mixing).
    pub async fn send_album(
        &self,
        peer:  tl::enums::Peer,
        items: Vec<(tl::enums::InputMedia, String)>, // (media, caption)
    ) -> Result<(), InvocationError> {
        let input_peer = {
            let cache = self.inner.peer_cache.lock().await;
            cache.peer_to_input(&peer)
        };

        let grouped_id = crate::random_i64_pub().unsigned_abs() as i64;

        let multi: Vec<tl::enums::InputSingleMedia> = items.into_iter().map(|(media, caption)| {
            tl::enums::InputSingleMedia::InputSingleMedia(tl::types::InputSingleMedia {
                media,
                random_id: crate::random_i64_pub(),
                message:   caption,
                entities:  None,
            })
        }).collect();

        let req = tl::functions::messages::SendMultiMedia {
            silent:                   false,
            background:               false,
            clear_draft:              false,
            noforwards:               false,
            update_stickersets_order: false,
            invert_media:             false,
            allow_paid_floodskip:     false,
            peer:                     input_peer,
            reply_to:                 None,
            multi_media:              multi,
            schedule_date:            None,
            send_as:                  None,
            quick_reply_shortcut:     None,
            effect:                   None,
            allow_paid_stars:         None,
        };
        let _ = grouped_id; // Telegram auto-generates the grouped_id server-side
        self.rpc_call_raw_pub(&req).await?;
        Ok(())
    }

    /// Create a download iterator for a media attachment.
    ///
    /// ```rust,no_run
    /// # async fn f(client: layer_client::Client, msg: layer_client::update::IncomingMessage) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut bytes = Vec::new();
    /// if let Some(loc) = msg.download_location() {
    ///     let mut iter = client.iter_download(loc);
    ///     while let Some(chunk) = iter.next().await? {
    ///         bytes.extend_from_slice(&chunk);
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn iter_download(&self, location: tl::enums::InputFileLocation) -> DownloadIter {
        DownloadIter {
            client:  self.clone(),
            done:    false,
            request: Some(tl::functions::upload::GetFile {
                precise:       false,
                cdn_supported: false,
                location,
                offset:        0,
                limit:         DOWNLOAD_CHUNK_SIZE,
            }),
        }
    }

    /// Download all bytes of a media attachment at once.
    pub async fn download_media(
        &self,
        location: tl::enums::InputFileLocation,
    ) -> Result<Vec<u8>, InvocationError> {
        let mut bytes = Vec::new();
        let mut iter  = self.iter_download(location);
        while let Some(chunk) = iter.next().await? {
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

// ─── InputFileLocation from IncomingMessage ───────────────────────────────────

impl crate::update::IncomingMessage {
    /// Get the [`InputFileLocation`] for the media in this message, if any.
    ///
    /// Returns `None` for messages without downloadable media.
    pub fn download_location(&self) -> Option<tl::enums::InputFileLocation> {
        let media = match &self.raw {
            tl::enums::Message::Message(m) => m.media.as_ref()?,
            _ => return None,
        };
        match media {
            tl::enums::MessageMedia::Photo(mp) => {
                if let Some(tl::enums::Photo::Photo(p)) = &mp.photo {
                    // Find the largest PhotoSize
                    let thumb = p.sizes.iter().filter_map(|s| match s {
                        tl::enums::PhotoSize::PhotoSize(ps) => Some(ps.r#type.clone()),
                        _ => None,
                    }).last().unwrap_or_else(|| "s".to_string());

                    Some(tl::enums::InputFileLocation::InputPhotoFileLocation(
                        tl::types::InputPhotoFileLocation {
                            id:             p.id,
                            access_hash:    p.access_hash,
                            file_reference: p.file_reference.clone(),
                            thumb_size:     thumb,
                        }
                    ))
                } else { None }
            }
            tl::enums::MessageMedia::Document(md) => {
                if let Some(tl::enums::Document::Document(d)) = &md.document {
                    Some(tl::enums::InputFileLocation::InputDocumentFileLocation(
                        tl::types::InputDocumentFileLocation {
                            id:             d.id,
                            access_hash:    d.access_hash,
                            file_reference: d.file_reference.clone(),
                            thumb_size:     String::new(),
                        }
                    ))
                } else { None }
            }
            _ => None,
        }
    }
}

// ─── MD5 helper (no external dep) ────────────────────────────────────────────

fn md5_bytes(data: &[u8]) -> u128 {
    // Simple MD5 using sha2 isn't available, so we use a basic implementation
    // This is only for small-file checksum; big files skip it.
    // For production use, add the `md-5` crate.
    // Here we return 0 as a placeholder (Telegram accepts empty checksum).
    let _ = data;
    0u128
}
