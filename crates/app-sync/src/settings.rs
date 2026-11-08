//! The `dail/Settings` folder over IMAP (ADR 0015): read the newest copy of the settings
//! document, write a new one and expunge the older ones. Only that folder is touched.

use app_core::{SETTINGS_MAX_BYTES, SettingsCopy, SettingsError};
use app_io::settings_blob;
use async_imap::types::Fetch;
use futures_util::TryStreamExt as _;

use crate::imap::{ImapSession, delete_uids};

/// What a read found: the newest copy and, unless it is `known`, its JSON.
pub type ReadResult = (Option<SettingsCopy>, Option<String>);

/// Whether `path` exists (`LIST "" path`).
async fn exists(session: &mut ImapSession, path: &str) -> Result<bool, String> {
    let names: Vec<_> = session
        .list(Some(""), Some(path))
        .await
        .map_err(|err| format!("IMAP LIST {path} failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP LIST {path} failed: {err}"))?;
    Ok(names.iter().any(|name| name.name() == path))
}

/// The JSON of a raw message from the folder: its `application/json` part, as UTF-8.
pub fn document_of(raw: &[u8]) -> Result<String, String> {
    let malformed = |reason: &str| {
        SettingsError::Malformed {
            reason: reason.to_owned(),
        }
        .to_string()
    };
    let bytes =
        settings_blob::document(raw).ok_or_else(|| malformed("no application/json part"))?;
    String::from_utf8(bytes).map_err(|_| malformed("the JSON part is not UTF-8"))
}

/// The refusal of a message of `bytes` bytes, larger than [`settings_blob::MAX_RAW_BYTES`].
pub fn too_large(bytes: usize) -> String {
    SettingsError::TooLarge {
        bytes,
        max: SETTINGS_MAX_BYTES,
    }
    .to_string()
}

/// Reads the message with the highest UID in `path` (a missing or empty folder holds none).
/// It is not downloaded if it is `known`, or if it is larger than
/// [`settings_blob::MAX_RAW_BYTES`] (an error).
pub async fn read(
    session: &mut ImapSession,
    path: &str,
    known: Option<SettingsCopy>,
) -> Result<ReadResult, String> {
    if !exists(session, path).await? {
        return Ok((None, None));
    }
    let selected = session
        .examine(path)
        .await
        .map_err(|err| format!("IMAP EXAMINE {path} failed: {err}"))?;
    let Some(uid_validity) = selected.uid_validity else {
        return Err(format!("IMAP EXAMINE {path}: no UIDVALIDITY"));
    };
    if selected.exists == 0 {
        return Ok((None, None));
    }
    let newest: Vec<Fetch> = session
        .uid_fetch("*", "(UID RFC822.SIZE)")
        .await
        .map_err(|err| format!("IMAP UID FETCH in {path} failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP UID FETCH in {path} failed: {err}"))?;
    let Some((uid, size)) = newest
        .iter()
        .filter_map(|fetch| Some((fetch.uid?, fetch.size.unwrap_or(0))))
        .max_by_key(|(uid, _)| *uid)
    else {
        return Ok((None, None));
    };
    let copy = SettingsCopy { uid_validity, uid };
    if known == Some(copy) {
        return Ok((Some(copy), None));
    }
    let size = usize::try_from(size).unwrap_or(usize::MAX);
    if size > settings_blob::MAX_RAW_BYTES {
        return Err(too_large(size));
    }
    let bodies: Vec<Fetch> = session
        .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
        .await
        .map_err(|err| format!("IMAP UID FETCH BODY in {path} failed: {err}"))?
        .try_collect()
        .await
        .map_err(|err| format!("IMAP UID FETCH BODY in {path} failed: {err}"))?;
    let raw = bodies
        .iter()
        .find(|fetch| fetch.uid == Some(uid))
        .and_then(Fetch::body)
        .ok_or_else(|| format!("{path}: message UID {uid} is no longer on the server"))?;
    if raw.len() > settings_blob::MAX_RAW_BYTES {
        return Err(too_large(raw.len()));
    }
    Ok((Some(copy), Some(document_of(raw)?)))
}

/// `APPEND`s `raw` to `path` (creating the folder if the first try fails), then marks every
/// older message there `\Deleted` and expunges them. The new copy is appended first, so a
/// failure never loses the settings; at worst an old copy stays.
pub async fn write(session: &mut ImapSession, path: &str, raw: &[u8]) -> Result<(), String> {
    if session
        .append(path, Some("(\\Seen)"), None, raw)
        .await
        .is_err()
    {
        // The folder is missing (first write): create it and try once more.
        if let Err(err) = session.create(path).await {
            tracing::debug!(error = %err, "IMAP CREATE of the settings folder failed");
        }
        session
            .append(path, Some("(\\Seen)"), None, raw)
            .await
            .map_err(|err| format!("IMAP APPEND to {path} failed: {err}"))?;
    }
    session
        .select(path)
        .await
        .map_err(|err| format!("IMAP SELECT {path} failed: {err}"))?;
    let mut uids: Vec<u32> = session
        .uid_search("ALL")
        .await
        .map_err(|err| format!("IMAP UID SEARCH in {path} failed: {err}"))?
        .into_iter()
        .collect();
    uids.sort_unstable();
    uids.pop();
    delete_uids(session, path, &uids).await
}
