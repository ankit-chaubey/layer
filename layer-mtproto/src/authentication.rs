//! Sans-IO MTProto authorization key generation.
//!
//! # Flow
//!
//! ```text
//! let (req, s1) = authentication::step1()?;
//! // send req, receive resp
//! let (req, s2) = authentication::step2(s1, resp)?;
//! // send req, receive resp
//! let (req, s3) = authentication::step3(s2, resp)?;
//! // send req, receive resp
//! let done = authentication::finish(s3, resp)?;
//! // done.auth_key is ready
//! ```

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use layer_crypto::{AuthKey, aes, factorize, generate_key_data_from_nonce, rsa};
use layer_tl_types::{Cursor, Deserializable, Serializable};
use num_bigint::{BigUint, ToBigUint};
use sha1::{Digest, Sha1};

// ─── Error ────────────────────────────────────────────────────────────────────

/// Errors that can occur during auth key generation.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    InvalidNonce         { got: [u8; 16], expected: [u8; 16] },
    InvalidPqSize        { size: usize },
    UnknownFingerprints  { fingerprints: Vec<i64> },
    DhParamsFail,
    InvalidServerNonce   { got: [u8; 16], expected: [u8; 16] },
    EncryptedResponseNotPadded { len: usize },
    InvalidDhInnerData   { error: layer_tl_types::deserialize::Error },
    GParameterOutOfRange { value: BigUint, low: BigUint, high: BigUint },
    DhGenRetry,
    DhGenFail,
    InvalidAnswerHash    { got: [u8; 20], expected: [u8; 20] },
    InvalidNewNonceHash  { got: [u8; 16], expected: [u8; 16] },
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNonce { got, expected }
                => write!(f, "nonce mismatch: got {got:?}, expected {expected:?}"),
            Self::InvalidPqSize { size }
                => write!(f, "pq size {size} invalid (expected 8)"),
            Self::UnknownFingerprints { fingerprints }
                => write!(f, "no known fingerprint in {fingerprints:?}"),
            Self::DhParamsFail
                => write!(f, "server returned DH params failure"),
            Self::InvalidServerNonce { got, expected }
                => write!(f, "server_nonce mismatch: got {got:?}, expected {expected:?}"),
            Self::EncryptedResponseNotPadded { len }
                => write!(f, "encrypted answer len {len} is not 16-byte aligned"),
            Self::InvalidDhInnerData { error }
                => write!(f, "DH inner data deserialization error: {error}"),
            Self::GParameterOutOfRange { value, low, high }
                => write!(f, "g={value} not in range ({low}, {high})"),
            Self::DhGenRetry  => write!(f, "DH gen retry requested"),
            Self::DhGenFail   => write!(f, "DH gen failed"),
            Self::InvalidAnswerHash { got, expected }
                => write!(f, "answer hash mismatch: got {got:?}, expected {expected:?}"),
            Self::InvalidNewNonceHash { got, expected }
                => write!(f, "new nonce hash mismatch: got {got:?}, expected {expected:?}"),
        }
    }
}

// ─── Step state ──────────────────────────────────────────────────────────────

/// State after step 1.
pub struct Step1 { nonce: [u8; 16] }

/// State after step 2.
pub struct Step2 {
    nonce:        [u8; 16],
    server_nonce: [u8; 16],
    new_nonce:    [u8; 32],
}

/// State after step 3.
pub struct Step3 {
    nonce:        [u8; 16],
    server_nonce: [u8; 16],
    new_nonce:    [u8; 32],
    gab:          BigUint,
    time_offset:  i32,
}

/// The final output of a successful auth key handshake.
#[derive(Clone, Debug, PartialEq)]
pub struct Finished {
    /// The 256-byte Telegram authorization key.
    pub auth_key:    [u8; 256],
    /// Clock skew in seconds relative to the server.
    pub time_offset: i32,
    /// Initial server salt.
    pub first_salt:  i64,
}

// ─── Step 1: req_pq_multi ────────────────────────────────────────────────────

/// Generate a `req_pq_multi` request. Returns the request + opaque state.
pub fn step1() -> Result<(layer_tl_types::functions::ReqPqMulti, Step1), Error> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("getrandom");
    do_step1(&buf)
}

fn do_step1(random: &[u8; 16]) -> Result<(layer_tl_types::functions::ReqPqMulti, Step1), Error> {
    let nonce = *random;
    Ok((layer_tl_types::functions::ReqPqMulti { nonce }, Step1 { nonce }))
}

// ─── Step 2: req_DH_params ───────────────────────────────────────────────────

/// Process `ResPQ` and generate `req_DH_params`.
pub fn step2(
    data:     Step1,
    response: layer_tl_types::enums::ResPq,
) -> Result<(layer_tl_types::functions::ReqDhParams, Step2), Error> {
    let mut rnd = [0u8; 256];
    getrandom::getrandom(&mut rnd).expect("getrandom");
    do_step2(data, response, &rnd)
}

fn do_step2(
    data:     Step1,
    response: layer_tl_types::enums::ResPq,
    random:   &[u8; 256],
) -> Result<(layer_tl_types::functions::ReqDhParams, Step2), Error> {
    let Step1 { nonce } = data;

    // ResPq has a single constructor: resPQ → variant ResPq
    let res_pq = match response {
        layer_tl_types::enums::ResPq::ResPq(x) => x,
    };

    check_nonce(&res_pq.nonce, &nonce)?;

    if res_pq.pq.len() != 8 {
        return Err(Error::InvalidPqSize { size: res_pq.pq.len() });
    }

    let pq = u64::from_be_bytes(res_pq.pq.as_slice().try_into().unwrap());
    let (p, q) = factorize(pq);

    let mut new_nonce = [0u8; 32];
    new_nonce.copy_from_slice(&random[..32]);

    // random[32..256] is 224 bytes for RSA padding
    let rnd224: &[u8; 224] = random[32..].try_into().unwrap();

    fn trim_be(v: u64) -> Vec<u8> {
        let b = v.to_be_bytes();
        let skip = b.iter().position(|&x| x != 0).unwrap_or(7);
        b[skip..].to_vec()
    }

    let p_bytes = trim_be(p);
    let q_bytes = trim_be(q);

    // Build PQInnerData using the first (non-DC, non-temp) constructor
    // variant name: PQInnerData (same as type name since constructor name == type name)
    let pq_inner = layer_tl_types::enums::PQInnerData::PQInnerData(
        layer_tl_types::types::PQInnerData {
            pq: pq.to_be_bytes().to_vec(),
            p: p_bytes.clone(),
            q: q_bytes.clone(),
            nonce,
            server_nonce: res_pq.server_nonce,
            new_nonce,
        }
    ).to_bytes();

    let fingerprint = res_pq.server_public_key_fingerprints
        .iter()
        .copied()
        .find(|&fp| key_for_fingerprint(fp).is_some())
        .ok_or_else(|| Error::UnknownFingerprints {
            fingerprints: res_pq.server_public_key_fingerprints.clone()
        })?;

    let key = key_for_fingerprint(fingerprint).unwrap();
    let ciphertext = rsa::encrypt_hashed(&pq_inner, &key, rnd224);

    Ok((
        layer_tl_types::functions::ReqDhParams {
            nonce,
            server_nonce: res_pq.server_nonce,
            p: p_bytes,
            q: q_bytes,
            public_key_fingerprint: fingerprint,
            encrypted_data: ciphertext,
        },
        Step2 { nonce, server_nonce: res_pq.server_nonce, new_nonce },
    ))
}

// ─── Step 3: set_client_DH_params ────────────────────────────────────────────

/// Process `ServerDhParams` and generate `set_client_DH_params`.
pub fn step3(
    data:     Step2,
    response: layer_tl_types::enums::ServerDhParams,
) -> Result<(layer_tl_types::functions::SetClientDhParams, Step3), Error> {
    let mut rnd = [0u8; 272]; // 256 for DH b, 16 for padding
    getrandom::getrandom(&mut rnd).expect("getrandom");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap().as_secs() as i32;
    do_step3(data, response, &rnd, now)
}

fn do_step3(
    data:     Step2,
    response: layer_tl_types::enums::ServerDhParams,
    random:   &[u8; 272],
    now:      i32,
) -> Result<(layer_tl_types::functions::SetClientDhParams, Step3), Error> {
    let Step2 { nonce, server_nonce, new_nonce } = data;

    let mut server_dh_ok = match response {
        layer_tl_types::enums::ServerDhParams::Fail(f) => {
            check_nonce(&f.nonce, &nonce)?;
            check_server_nonce(&f.server_nonce, &server_nonce)?;
            // Verify new_nonce_hash
            let digest: [u8; 20] = {
                let mut sha = Sha1::new();
                sha.update(new_nonce);
                sha.finalize().into()
            };
            let mut expected_hash = [0u8; 16];
            expected_hash.copy_from_slice(&digest[4..]);
            check_new_nonce_hash(&f.new_nonce_hash, &expected_hash)?;
            return Err(Error::DhParamsFail);
        }
        layer_tl_types::enums::ServerDhParams::Ok(x) => x,
    };

    check_nonce(&server_dh_ok.nonce, &nonce)?;
    check_server_nonce(&server_dh_ok.server_nonce, &server_nonce)?;

    if server_dh_ok.encrypted_answer.len() % 16 != 0 {
        return Err(Error::EncryptedResponseNotPadded { len: server_dh_ok.encrypted_answer.len() });
    }

    let (key, iv) = generate_key_data_from_nonce(&server_nonce, &new_nonce);
    aes::ige_decrypt(&mut server_dh_ok.encrypted_answer, &key, &iv);
    let plain = server_dh_ok.encrypted_answer;

    let got_hash: [u8; 20] = plain[..20].try_into().unwrap();
    let mut cursor = Cursor::from_slice(&plain[20..]);

    // ServerDhInnerData has single constructor server_DH_inner_data
    // variant name = ServerDhInnerData (full name, since it equals type name)
    let inner = match layer_tl_types::enums::ServerDhInnerData::deserialize(&mut cursor) {
        Ok(layer_tl_types::enums::ServerDhInnerData::ServerDhInnerData(x)) => x,
        Err(e) => return Err(Error::InvalidDhInnerData { error: e }),
    };

    let expected_hash: [u8; 20] = {
        let mut sha = Sha1::new();
        sha.update(&plain[20..20 + cursor.pos()]);
        sha.finalize().into()
    };
    if got_hash != expected_hash {
        return Err(Error::InvalidAnswerHash { got: got_hash, expected: expected_hash });
    }

    check_nonce(&inner.nonce, &nonce)?;
    check_server_nonce(&inner.server_nonce, &server_nonce)?;

    let dh_prime = BigUint::from_bytes_be(&inner.dh_prime);
    let g = inner.g.to_biguint().unwrap();
    let g_a = BigUint::from_bytes_be(&inner.g_a);
    let time_offset = inner.server_time - now;

    let b = BigUint::from_bytes_be(&random[..256]);
    let g_b = g.modpow(&b, &dh_prime);
    let gab = g_a.modpow(&b, &dh_prime);

    // Validate DH parameters
    let one = BigUint::from(1u32);
    check_g_in_range(&g,   &one, &(&dh_prime - &one))?;
    check_g_in_range(&g_a, &one, &(&dh_prime - &one))?;
    check_g_in_range(&g_b, &one, &(&dh_prime - &one))?;
    let safety = one.clone() << (2048 - 64);
    check_g_in_range(&g_a, &safety, &(&dh_prime - &safety))?;
    check_g_in_range(&g_b, &safety, &(&dh_prime - &safety))?;

    // ClientDhInnerData has single constructor client_DH_inner_data
    // variant name = ClientDhInnerData
    let client_dh_inner = layer_tl_types::enums::ClientDhInnerData::ClientDhInnerData(
        layer_tl_types::types::ClientDhInnerData {
            nonce,
            server_nonce,
            retry_id: 0,
            g_b: g_b.to_bytes_be(),
        }
    ).to_bytes();

    let digest: [u8; 20] = {
        let mut sha = Sha1::new();
        sha.update(&client_dh_inner);
        sha.finalize().into()
    };

    let pad_len = (16 - ((20 + client_dh_inner.len()) % 16)) % 16;
    let rnd16 = &random[256..256 + pad_len.min(16)];

    let mut hashed = Vec::with_capacity(20 + client_dh_inner.len() + pad_len);
    hashed.extend_from_slice(&digest);
    hashed.extend_from_slice(&client_dh_inner);
    hashed.extend_from_slice(&rnd16[..pad_len]);

    aes::ige_encrypt(&mut hashed, &key, &iv);

    Ok((
        layer_tl_types::functions::SetClientDhParams {
            nonce,
            server_nonce,
            encrypted_data: hashed,
        },
        Step3 { nonce, server_nonce, new_nonce, gab, time_offset },
    ))
}

// ─── finish: create_key ──────────────────────────────────────────────────────

/// Finalise the handshake. Returns the ready [`Finished`] on success.
pub fn finish(
    data:     Step3,
    response: layer_tl_types::enums::SetClientDhParamsAnswer,
) -> Result<Finished, Error> {
    let Step3 { nonce, server_nonce, new_nonce, gab, time_offset } = data;

    struct DhData { nonce: [u8; 16], server_nonce: [u8; 16], hash: [u8; 16], num: u8 }

    let dh = match response {
        // Variant names come from the constructor names: dh_gen_ok → DhGenOk, etc.
        layer_tl_types::enums::SetClientDhParamsAnswer::DhGenOk(x)    =>
            DhData { nonce: x.nonce, server_nonce: x.server_nonce, hash: x.new_nonce_hash1, num: 1 },
        layer_tl_types::enums::SetClientDhParamsAnswer::DhGenRetry(x) =>
            DhData { nonce: x.nonce, server_nonce: x.server_nonce, hash: x.new_nonce_hash2, num: 2 },
        layer_tl_types::enums::SetClientDhParamsAnswer::DhGenFail(x)  =>
            DhData { nonce: x.nonce, server_nonce: x.server_nonce, hash: x.new_nonce_hash3, num: 3 },
    };

    check_nonce(&dh.nonce, &nonce)?;
    check_server_nonce(&dh.server_nonce, &server_nonce)?;

    let mut key_bytes = [0u8; 256];
    let gab_bytes = gab.to_bytes_be();
    let skip = 256 - gab_bytes.len();
    key_bytes[skip..].copy_from_slice(&gab_bytes);

    let auth_key = AuthKey::from_bytes(key_bytes);
    let expected_hash = auth_key.calc_new_nonce_hash(&new_nonce, dh.num);
    check_new_nonce_hash(&dh.hash, &expected_hash)?;

    let first_salt = {
        let mut buf = [0u8; 8];
        for ((dst, a), b) in buf.iter_mut().zip(&new_nonce[..8]).zip(&server_nonce[..8]) {
            *dst = a ^ b;
        }
        i64::from_le_bytes(buf)
    };

    match dh.num {
        1 => Ok(Finished { auth_key: auth_key.to_bytes(), time_offset, first_salt }),
        2 => Err(Error::DhGenRetry),
        _ => Err(Error::DhGenFail),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn check_nonce(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected { Ok(()) } else {
        Err(Error::InvalidNonce { got: *got, expected: *expected })
    }
}
fn check_server_nonce(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected { Ok(()) } else {
        Err(Error::InvalidServerNonce { got: *got, expected: *expected })
    }
}
fn check_new_nonce_hash(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected { Ok(()) } else {
        Err(Error::InvalidNewNonceHash { got: *got, expected: *expected })
    }
}
fn check_g_in_range(val: &BigUint, lo: &BigUint, hi: &BigUint) -> Result<(), Error> {
    if lo < val && val < hi { Ok(()) } else {
        Err(Error::GParameterOutOfRange { value: val.clone(), low: lo.clone(), high: hi.clone() })
    }
}

/// RSA key by server fingerprint. Includes both production and test DC keys.
#[allow(clippy::unreadable_literal)]
pub fn key_for_fingerprint(fp: i64) -> Option<rsa::Key> {
    Some(match fp {
        // Production DC key (fingerprint -3414540481677951611)
        -3414540481677951611 => rsa::Key::new(
            "29379598170669337022986177149456128565388431120058863768162556424047512191330847455146576344487764408661701890505066208632169112269581063774293102577308490531282748465986139880977280302242772832972539403531316010870401287642763009136156734339538042419388722777357134487746169093539093850251243897188928735903389451772730245253062963384108812842079887538976360465290946139638691491496062099570836476454855996319192747663615955633778034897140982517446405334423701359108810182097749467210509584293428076654573384828809574217079944388301239431309115013843331317877374435868468779972014486325557807783825502498215169806323",
            "65537"
        )?,
        // Test DC key (fingerprint -5595554452916591101)
        -5595554452916591101 => rsa::Key::new(
            "25342889448840415564971689590713473206898847759084779052582026594546022463853940585885215951168491965708222649399180603818074200620463776135424884632162512403163793083921641631564740959529419359595852941166848940585952337613333022396096584117954892216031229237302943701877588456738335398602461675225081791820393153757504952636234951323237820036543581047826906120927972487366805292115792231423684261262330394324750785450942589751755390156647751460719351439969059949569615302809050721500330239005077889855323917509948255722081644689442127297605422579707142646660768825302832201908302295573257427896031830742328565032949",
            "65537"
        )?,
        _ => return None,
    })
}
