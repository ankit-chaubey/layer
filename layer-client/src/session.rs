//! Session persistence — saves auth key, salt, time offset, DC table.

use std::collections::HashMap;
use std::io::{self};
use std::path::Path;

#[derive(Clone)]
pub struct DcEntry {
    pub dc_id:      i32,
    pub addr:       String,
    pub auth_key:   Option<[u8; 256]>,
    pub first_salt: i64,
    pub time_offset: i32,
}

pub struct PersistedSession {
    pub home_dc_id: i32,
    pub dcs:        Vec<DcEntry>,
}

impl PersistedSession {
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.home_dc_id.to_le_bytes());
        b.push(self.dcs.len() as u8);
        for d in &self.dcs {
            b.extend_from_slice(&d.dc_id.to_le_bytes());
            match &d.auth_key {
                Some(k) => { b.push(1); b.extend_from_slice(k); }
                None    => { b.push(0); }
            }
            b.extend_from_slice(&d.first_salt.to_le_bytes());
            b.extend_from_slice(&d.time_offset.to_le_bytes());
            let ab = d.addr.as_bytes();
            b.push(ab.len() as u8);
            b.extend_from_slice(ab);
        }
        std::fs::write(path, b)
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let buf = std::fs::read(path)?;
        let mut p = 0usize;
        macro_rules! r {
            ($n:expr) => {{
                if p + $n > buf.len() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated session"));
                }
                let s = &buf[p..p + $n];
                p += $n;
                s
            }};
        }
        let home_dc_id = i32::from_le_bytes(r!(4).try_into().unwrap());
        let dc_count   = r!(1)[0] as usize;
        let mut dcs    = Vec::with_capacity(dc_count);
        for _ in 0..dc_count {
            let dc_id       = i32::from_le_bytes(r!(4).try_into().unwrap());
            let has_key     = r!(1)[0];
            let auth_key    = if has_key == 1 {
                let mut k = [0u8; 256];
                k.copy_from_slice(r!(256));
                Some(k)
            } else {
                None
            };
            let first_salt   = i64::from_le_bytes(r!(8).try_into().unwrap());
            let time_offset  = i32::from_le_bytes(r!(4).try_into().unwrap());
            let al           = r!(1)[0] as usize;
            let addr         = String::from_utf8_lossy(r!(al)).into_owned();
            dcs.push(DcEntry { dc_id, addr, auth_key, first_salt, time_offset });
        }
        Ok(Self { home_dc_id, dcs })
    }
}

/// Bootstrap DC address table (fallback if GetConfig fails).
pub fn default_dc_addresses() -> HashMap<i32, String> {
    [
        (1, "149.154.175.53:443"),
        (2, "149.154.167.51:443"),
        (3, "149.154.175.100:443"),
        (4, "149.154.167.91:443"),
        (5, "91.108.56.130:443"),
    ]
    .into_iter()
    .map(|(id, addr)| (id, addr.to_string()))
    .collect()
}
