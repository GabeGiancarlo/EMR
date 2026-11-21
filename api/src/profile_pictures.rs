//! Profile pictures for clients and staff.
//!
//! A person's picture is an uploaded photo, one of the ten Nexus motifs chosen as a
//! default, or nothing stored at all, in which case the app draws the automatic default it
//! derives from the person's id. Pictures live in `profile_pictures`, never on the client
//! or staff row: setting one changes no record version and no clinical index.
//!
//! Who may do what follows the person the picture belongs to. A client's picture is read
//! and changed by whoever may read and change that client, inside the same organization
//! and the same clinician assignment scope. Every member of staff may set their own
//! picture; only an administrator may set someone else's. Every change is audited in the
//! transaction that makes it.
use crate::{
    access::{self, Actor},
    records, ApiError, AppState, Result,
};
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// The largest photo stored. The app sends a 256 pixel square, well under this.
pub const MAX_PHOTO_BYTES: usize = 512 * 1024;
/// The largest side accepted, so a small file cannot unpack into a huge bitmap in a browser.
const MAX_SIDE: u32 = 1024;
/// The motifs that can be chosen as a default. The migration's CHECK lists the same names.
pub const MOTIFS: [&str; 10] = [
    "sprig", "fern", "sprout", "pine", "hill", "river", "cairn", "slab", "sun", "moon",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Subject {
    Clients,
    Staff,
}
impl Subject {
    fn parse(text: &str) -> Result<Self> {
        match text {
            "clients" => Ok(Self::Clients),
            "staff" => Ok(Self::Staff),
            _ => Err(ApiError::missing()),
        }
    }
    fn resource(self) -> &'static str {
        match self {
            Self::Clients => "clients",
            Self::Staff => "staff",
        }
    }
    fn column(self) -> &'static str {
        match self {
            Self::Clients => "client_id",
            Self::Staff => "staff_id",
        }
    }
}

/// What an image file says about itself in its own header.
#[derive(Debug, PartialEq, Eq)]
struct Image {
    content_type: &'static str,
    width: u32,
    height: u32,
}

fn png(bytes: &[u8]) -> Option<Image> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some(Image {
        content_type: "image/png",
        width: u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        height: u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
    })
}

fn jpeg(bytes: &[u8]) -> Option<Image> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut at = 2;
    while at + 3 < bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xFF {
            at += 1; // padding between segments
            continue;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            at += 2; // markers that carry no length
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            return None; // the image data began before any frame header
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        if length < 2 {
            return None;
        }
        // Start-of-frame markers, excluding the three in that range that are tables.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            if at + 8 >= bytes.len() {
                return None;
            }
            return Some(Image {
                content_type: "image/jpeg",
                height: u32::from(u16::from_be_bytes([bytes[at + 5], bytes[at + 6]])),
                width: u32::from(u16::from_be_bytes([bytes[at + 7], bytes[at + 8]])),
            });
        }
        at += 2 + length;
    }
    None
}

fn webp(bytes: &[u8]) -> Option<Image> {
    if bytes.len() < 30 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let (width, height) = match &bytes[12..16] {
        b"VP8 " => {
            if bytes[23..26] != [0x9D, 0x01, 0x2A] {
                return None;
            }
            (
                u32::from(u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF),
                u32::from(u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF),
            )
        }
        b"VP8L" => {
            if bytes[20] != 0x2F {
                return None;
            }
            let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
            ((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1)
        }
        b"VP8X" => {
            // A profile picture holds still: the animation flag is refused.
            if bytes[20] & 0x02 != 0 {
                return None;
            }
            let three = |at: usize| {
                u32::from(bytes[at])
                    | u32::from(bytes[at + 1]) << 8
                    | u32::from(bytes[at + 2]) << 16
            };
            (three(24) + 1, three(27) + 1)
        }
        _ => return None,
    };
    Some(Image {
        content_type: "image/webp",
        width,
        height,
    })
}

fn inspect(bytes: &[u8]) -> Option<Image> {
    png(bytes).or_else(|| jpeg(bytes)).or_else(|| webp(bytes))
}

/// The declared media type without parameters, lower case.
fn declared_type(req: &HttpRequest) -> String {
    req.headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn too_large() -> ApiError {
    ApiError {
        status: StatusCode::PAYLOAD_TOO_LARGE,
        code: "payload_too_large",
        message: "This photo is larger than 512 KB. Choose a smaller one.".into(),
    }
}

/// A photo is accepted only when its own header agrees with what the request says it is.
fn accept_photo(declared: &str, bytes: &[u8]) -> Result<Image> {
    if bytes.len() > MAX_PHOTO_BYTES {
        return Err(too_large());
    }
    let image = inspect(bytes)
        .ok_or_else(|| ApiError::invalid("Use a JPEG, PNG or WebP photo that is not animated."))?;
    if declared != image.content_type {
        return Err(ApiError::invalid(
            "The file is not the kind of image the request says it is. Use a JPEG, PNG or WebP photo.",
        ));
    }
    if image.width == 0 || image.height == 0 || image.width > MAX_SIDE || image.height > MAX_SIDE {
        return Err(ApiError::invalid(format!(
            "This photo is {} by {} pixels. The largest accepted is {MAX_SIDE} on a side.",
            image.width, image.height
        )));
    }
    Ok(image)
}

/// What a request asks the picture to become.
enum Change {
    Photo(Image),
    Motif(&'static str),
    Automatic,
}

fn read_change(req: &HttpRequest, body: &[u8]) -> Result<Change> {
    let declared = declared_type(req);
    if declared != "application/json" {
        return accept_photo(&declared, body).map(Change::Photo);
    }
    let choice: Value = serde_json::from_slice(body)
        .map_err(|_| ApiError::invalid("The request body is not valid JSON for this action."))?;
    match choice.get("motif") {
        Some(Value::Null) => Ok(Change::Automatic),
        Some(Value::String(name)) => MOTIFS
            .iter()
            .copied()
            .find(|motif| *motif == name.as_str())
            .map(Change::Motif)
            .ok_or_else(|| ApiError::invalid("Choose one of the Nexus default pictures.")),
        _ => Err(ApiError::invalid(
            "Send a photo, or {\"motif\": name} to choose a default, or {\"motif\": null} for the automatic one.",
        )),
    }
}

/// The acting staff member, re-read inside the transaction and allowed to change this picture.
async fn writer(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    subject: Subject,
    id: Uuid,
) -> Result<Actor> {
    match subject {
        Subject::Clients => {
            let actor = access::lock_mutation(req, tx, org, "clients").await?;
            let client = records::get(tx, "clients", id, org, false).await?;
            records::scope(tx, &actor, "clients", &client).await?;
            Ok(actor)
        }
        Subject::Staff => {
            // Your own picture is not a change to the staff record, so it needs no
            // administrator. Someone else's does. Serialize with offboarding first, then
            // read the caller again: a deactivated account cannot act on a stale role.
            crate::scheduling::lock_org(tx, org).await?;
            let me = access::staff_id(req)?;
            let value: Value = sqlx::query_scalar(
                "SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND organization_id=$2 AND active",
            )
            .bind(me)
            .bind(org)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(ApiError::denied)?;
            let actor = Actor {
                id: me,
                org,
                role: value["role"].as_str().unwrap_or_default().into(),
                value,
            };
            if actor.id != id && actor.role != "admin" {
                return Err(ApiError::denied());
            }
            records::get(tx, "staff", id, org, false).await?;
            Ok(actor)
        }
    }
}

/// `PUT /api/profile-pictures/{clients|staff}/{id}`: an image body stores a photo; a JSON
/// body chooses a default motif, or with `null` returns to the automatic one.
pub async fn replace(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, Uuid)>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (subject, id) = path.into_inner();
    let subject = Subject::parse(&subject)?;
    let org = access::actor(&req, &state.pool).await?.org;
    // Refuse a bad body before taking the organization's lock.
    let change = read_change(&req, &body)?;
    let mut tx = state.pool.begin().await?;
    let actor = writer(&req, &mut tx, org, subject, id).await?;
    let column = subject.column();
    sqlx::query(&format!(
        "DELETE FROM profile_pictures WHERE organization_id=$1 AND {column}=$2"
    ))
    .bind(org)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let insert = format!(
        "INSERT INTO profile_pictures(id,organization_id,{column},motif,content_type,bytes,sha256,updated_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8)"
    );
    let (avatar, action) = match change {
        Change::Automatic => (String::new(), "clear_profile_picture"),
        Change::Motif(motif) => {
            sqlx::query(&insert)
                .bind(Uuid::new_v4())
                .bind(org)
                .bind(id)
                .bind(motif)
                .bind(None::<&str>)
                .bind(None::<&[u8]>)
                .bind(None::<&str>)
                .bind(actor.id)
                .execute(&mut *tx)
                .await?;
            (motif.to_owned(), "choose_profile_default")
        }
        Change::Photo(image) => {
            let digest = crate::sha256_hex(&body);
            sqlx::query(&insert)
                .bind(Uuid::new_v4())
                .bind(org)
                .bind(id)
                .bind(None::<&str>)
                .bind(image.content_type)
                .bind(&body[..])
                .bind(&digest)
                .bind(actor.id)
                .execute(&mut *tx)
                .await?;
            (format!("photo:{}", &digest[..16]), "set_profile_photo")
        }
    };
    access::audit(&mut tx, &actor, action, subject.resource(), Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(json!({ "avatar": avatar })))
}

/// `GET /api/profile-pictures/{clients|staff}/{id}`: the stored photo, for anyone who may
/// read that person. A chosen motif or the automatic default has no file and answers 404.
pub async fn show(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, Uuid)>,
) -> Result<HttpResponse> {
    let (subject, id) = path.into_inner();
    let subject = Subject::parse(&subject)?;
    let actor = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    access::require(&mut *tx, &actor, subject.resource(), false).await?;
    let person = records::get(&mut tx, subject.resource(), id, actor.org, false).await?;
    if subject == Subject::Clients {
        records::scope(&mut tx, &actor, "clients", &person).await?;
    }
    let photo: Option<(String, Vec<u8>)> = sqlx::query_as(&format!(
        "SELECT content_type, bytes FROM profile_pictures WHERE organization_id=$1 AND {}=$2 AND bytes IS NOT NULL",
        subject.column()
    ))
    .bind(actor.org)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let (content_type, bytes) = photo.ok_or_else(ApiError::missing)?;
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", content_type))
        // A face is identifying. Nothing outside this signed-in session keeps a copy.
        .insert_header(("Cache-Control", "private, no-store"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Content-Disposition", "inline"))
        .insert_header(("Referrer-Policy", "no-referrer"))
        .body(bytes))
}

/// For the workspace: each visible person's chosen picture as `"fern"` or
/// `"photo:<16 hex>"`. The tag changes when the photo does, so the app knows to fetch again.
/// People with no row are absent and get the automatic default.
pub async fn for_workspace(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    workspace: &Value,
) -> Result<Value> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT COALESCE(client_id, staff_id), COALESCE(motif, 'photo:' || left(sha256, 16)) FROM profile_pictures WHERE organization_id=$1",
    )
    .bind(actor.org)
    .fetch_all(&mut **tx)
    .await?;
    // Only for people already in this response: the rows above them carry the role and
    // clinician-assignment scope, and a picture must never reveal a person they do not.
    let visible = |resource: &str, id: &str| {
        workspace[resource]
            .as_array()
            .is_some_and(|people| people.iter().any(|person| person["id"] == id))
    };
    let mut chosen = Map::new();
    for (id, avatar) in rows {
        let id = id.to_string();
        if visible("clients", &id) || visible("staff", &id) {
            chosen.insert(id, json!(avatar));
        }
    }
    Ok(Value::Object(chosen))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/profile-pictures/{subject}/{id}")
            .app_data(web::PayloadConfig::new(MAX_PHOTO_BYTES))
            .route(web::get().to(show))
            .route(web::put().to(replace))
            .default_service(web::to(crate::method_not_allowed)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    fn tiny_jpeg(width: u16, height: u16) -> Vec<u8> {
        // SOI, a JFIF segment to skip, then a baseline frame header.
        let mut bytes = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46, 0xFF, 0xC0, 0x00, 0x0B, 0x08,
        ];
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        bytes
    }

    fn tiny_webp(chunk: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
        bytes.extend_from_slice(chunk);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.extend_from_slice(payload);
        bytes.resize(bytes.len().max(30), 0);
        bytes
    }

    #[test]
    fn each_accepted_format_reports_its_own_size() {
        assert_eq!(
            inspect(&tiny_png(256, 200)),
            Some(Image {
                content_type: "image/png",
                width: 256,
                height: 200
            })
        );
        assert_eq!(
            inspect(&tiny_jpeg(300, 180)),
            Some(Image {
                content_type: "image/jpeg",
                width: 300,
                height: 180
            })
        );
        // Lossy: three frame bytes, the start code, then 14-bit width and height.
        let lossy = tiny_webp(
            b"VP8 ",
            &[0, 0, 0, 0x9D, 0x01, 0x2A, 0x00, 0x01, 0xC8, 0x00],
        );
        assert_eq!(
            inspect(&lossy),
            Some(Image {
                content_type: "image/webp",
                width: 256,
                height: 200
            })
        );
        // Lossless: signature byte, then (width - 1) and (height - 1) packed in 14 bits each.
        let packed: u32 = 255 | (199 << 14);
        let mut lossless = vec![0x2F];
        lossless.extend_from_slice(&packed.to_le_bytes());
        assert_eq!(
            inspect(&tiny_webp(b"VP8L", &lossless)),
            Some(Image {
                content_type: "image/webp",
                width: 256,
                height: 200
            })
        );
        // Extended: flags, three reserved bytes, then 24-bit (width - 1) and (height - 1).
        let extended = tiny_webp(b"VP8X", &[0, 0, 0, 0, 255, 0, 0, 199, 0, 0]);
        assert_eq!(
            inspect(&extended),
            Some(Image {
                content_type: "image/webp",
                width: 256,
                height: 200
            })
        );
    }

    #[test]
    fn files_that_are_not_still_photos_are_refused() {
        for bytes in [
            &b""[..],
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>",
            b"GIF89a\x01\0\x01\0\0\0\0",
            b"%PDF-1.7 not an image at all, only long enough",
            b"\x89PNG\r\n\x1a\n\0\0\0\rIDAT but no header chunk",
            b"\xFF\xD8\xFF\xD9",
        ] {
            assert_eq!(inspect(bytes), None);
        }
        let animated = tiny_webp(b"VP8X", &[0x02, 0, 0, 0, 255, 0, 0, 199, 0, 0]);
        assert_eq!(inspect(&animated), None);
        // A length that runs past the end must not read out of bounds.
        assert_eq!(inspect(&[0xFF, 0xD8, 0xFF, 0xE0, 0xFF, 0xFF, 0, 0]), None);
        assert_eq!(
            inspect(&[0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00]),
            None
        );
    }

    #[test]
    fn a_photo_must_be_what_it_claims_within_the_size_limits() {
        assert!(accept_photo("image/png", &tiny_png(256, 256)).is_ok());
        assert!(accept_photo("image/jpeg", &tiny_jpeg(1024, 1024)).is_ok());
        // Declared as one format, actually another.
        let mismatch = accept_photo("image/jpeg", &tiny_png(256, 256)).unwrap_err();
        assert_eq!(mismatch.status, StatusCode::BAD_REQUEST);
        assert!(accept_photo("image/svg+xml", b"<svg/>").is_err());
        assert!(accept_photo("", &tiny_png(256, 256)).is_err());
        // Small file, enormous bitmap.
        let huge = accept_photo("image/png", &tiny_png(20_000, 20_000)).unwrap_err();
        assert!(huge.message.contains("20000 by 20000"), "{}", huge.message);
        assert!(accept_photo("image/png", &tiny_png(0, 10)).is_err());
        assert!(accept_photo("image/jpeg", &tiny_jpeg(1025, 10)).is_err());
        let mut heavy = tiny_png(256, 256);
        heavy.resize(MAX_PHOTO_BYTES + 1, 0);
        assert_eq!(
            accept_photo("image/png", &heavy).unwrap_err().status,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[test]
    fn the_migration_allows_exactly_the_motifs_the_api_offers() {
        let migration = include_str!("../../migrations/202611210024_profile_pictures.sql");
        let listed = MOTIFS
            .iter()
            .map(|motif| format!("'{motif}'"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(
            migration.contains(&format!("motif IN ({listed})")),
            "{listed}"
        );
    }

    #[test]
    fn only_clients_and_staff_have_pictures() {
        assert_eq!(Subject::parse("clients").unwrap().column(), "client_id");
        assert_eq!(Subject::parse("staff").unwrap().column(), "staff_id");
        for other in ["facilities", "notes", "organizations", "client_id", ""] {
            assert_eq!(
                Subject::parse(other).unwrap_err().status,
                StatusCode::NOT_FOUND
            );
        }
    }
}
