# Media & Files

## Upload and send a photo

```rust
// Upload from disk path
let uploaded = client.upload_file("photo.jpg").await?;

// Send as compressed photo
client.send_file(
    peer,
    uploaded.as_photo_media(),
    Some("My caption here"),
).await?;
```

## Upload and send a document (any file)

```rust
let uploaded = client.upload_file("report.pdf").await?;

// Send as document (preserves original quality/format)
client.send_file(
    peer,
    uploaded.as_document_media(),
    Some("Monthly report"),
).await?;
```

> **TIP:** For photos, `as_photo_media()` lets Telegram compress and display them inline. Use `as_document_media()` to preserve original file quality and format.

## Upload from a stream

```rust
use tokio::fs::File;

let file    = File::open("video.mp4").await?;
let name    = "video.mp4".to_string();
let size    = file.metadata().await?.len() as i32;
let mime    = "video/mp4".to_string();

let uploaded = client.upload_stream(file, size, name, mime).await?;
client.send_file(peer, uploaded.as_document_media(), None).await?;
```

## Send an album (multiple photos/videos)

```rust
let img1 = client.upload_file("photo1.jpg").await?;
let img2 = client.upload_file("photo2.jpg").await?;
let img3 = client.upload_file("photo3.jpg").await?;

client.send_album(
    peer,
    vec![
        img1.as_photo_media(),
        img2.as_photo_media(),
        img3.as_photo_media(),
    ],
    Some("Our trip 📸"),
).await?;
```

Albums are grouped as a single visual unit in the chat.

## UploadedFile — methods

| Method | Returns | Description |
|---|---|---|
| `uploaded.name()` | `&str` | Original filename |
| `uploaded.mime_type()` | `&str` | Detected MIME type |
| `uploaded.as_photo_media()` | `InputMedia` | Send as compressed photo |
| `uploaded.as_document_media()` | `InputMedia` | Send as document |

## Download media from a message

```rust
if let tl::enums::Message::Message(m) = &raw_msg {
    if let Some(media) = &m.media {
        // download_media returns an async iterator of chunks
        let location = client.download_location(media);
        if let Some(loc) = location {
            let mut iter = client.iter_download(loc);
            let mut file = tokio::fs::File::create("download.bin").await?;

            while let Some(chunk) = iter.next().await? {
                tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            }
        }
    }
}
```

## DownloadIter — options

```rust
let mut iter = client.iter_download(location)
    .chunk_size(512 * 1024);  // 512 KB per request (default: 128 KB)
```

## MIME type reference

| File type | MIME | Displays as |
|---|---|---|
| JPEG, PNG, WebP | `image/jpeg`, `image/png` | Photo (compressed) |
| GIF | `image/gif` | Animated image |
| MP4, MOV | `video/mp4` | Video player |
| OGG (Opus codec) | `audio/ogg` | Voice message |
| MP3, FLAC | `audio/mpeg` | Audio player |
| PDF | `application/pdf` | Document with preview |
| ZIP, RAR | `application/zip` | Generic document |
| TGS | `application/x-tgsticker` | Animated sticker |

## Get profile photos

```rust
let photos = client.get_profile_photos(peer, 10).await?;

for photo in &photos {
    if let tl::enums::Photo::Photo(p) = photo {
        println!("Photo ID {} — {} sizes", p.id, p.sizes.len());

        // Find the largest size
        let best = p.sizes.iter()
            .filter_map(|s| match s {
                tl::enums::PhotoSize::PhotoSize(ps) => Some(ps),
                _ => None,
            })
            .max_by_key(|ps| ps.size);

        if let Some(size) = best {
            println!("  Largest: {}x{} ({}B)", size.w, size.h, size.size);
        }
    }
}
```
