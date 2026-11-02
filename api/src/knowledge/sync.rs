use super::*;
use sha2::{Digest, Sha256};
pub(super) fn stable_id(value: &str) -> Uuid {
    let hash = Sha256::digest(value.as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
pub(super) fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub(super) fn split(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0;
    let mut chunks = Vec::new();
    while start < chars.len() {
        let end = (start + 1200).min(chars.len());
        let part: String = chars[start..end].iter().collect();
        if !part.trim().is_empty() {
            chunks.push(part);
        }
        if end == chars.len() {
            break;
        }
        start = end - 150;
    }
    chunks
}
pub(crate) async fn refresh(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
) -> Result<()> {
    crate::scheduling::lock_org(tx, actor.org).await?;
    check_patient(tx, actor, client).await?;
    let documents = sources::collect(tx, actor, client).await?;
    if documents.len() > 1000 {
        return Err(ApiError::conflict("This patient exceeds the current 1000-source indexing limit. No partial index was published."));
    }
    let prepared: Vec<(&Document, Vec<String>)> =
        documents.iter().map(|d| (d, split(&d.body))).collect();
    if prepared
        .iter()
        .map(|(_, chunks)| chunks.len())
        .sum::<usize>()
        > 5000
    {
        return Err(ApiError::conflict("This patient exceeds the current 5000-excerpt indexing limit. No partial index was published."));
    }
    let ids: Vec<Uuid> = documents.iter().map(|d| d.id).collect();
    sqlx::query("DELETE FROM knowledge_documents WHERE organization_id=$1 AND client_id=$2 AND NOT(id=ANY($3))").bind(actor.org).bind(client).bind(&ids).execute(&mut **tx).await?;
    for (document, chunks) in prepared {
        sqlx::query("INSERT INTO knowledge_documents(id,organization_id,client_id,source_kind,source_id,source_version,source_hash,title,body,source_date,episode_id,session_id,classification) SELECT id,organization_id,client_id,source_kind,source_id,source_version,source_hash,title,body,source_date,episode_id,session_id,classification FROM jsonb_populate_record(NULL::knowledge_documents,$1) ON CONFLICT(id) DO NOTHING").bind(json!(document)).execute(&mut **tx).await?;
        for (ordinal, text) in chunks.iter().enumerate() {
            let id = stable_id(&format!("{}:{ordinal}:{}", document.id, hash(text)));
            sqlx::query("INSERT INTO knowledge_chunks(id,organization_id,client_id,document_id,ordinal,text) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(id) DO NOTHING").bind(id).bind(actor.org).bind(client).bind(document.id).bind(ordinal as i32).bind(text).execute(&mut **tx).await?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_chunks_preserve_overlap_without_breaking_characters() {
        let value = "é療🪷".repeat(1000);
        let chunks = split(&value);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.chars().count() <= 1200));
        assert_eq!(
            chunks[0].chars().skip(1050).collect::<String>(),
            chunks[1].chars().take(150).collect::<String>()
        );
        let rebuilt = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                c.chars()
                    .skip(if i == 0 { 0 } else { 150 })
                    .collect::<String>()
            })
            .collect::<String>();
        assert_eq!(rebuilt, value);
    }
    #[test]
    fn stable_ids_distinguish_source_versions_and_index_rebuilds() {
        assert_eq!(stable_id("source:v1:0"), stable_id("source:v1:0"));
        assert_ne!(stable_id("source:v1:0"), stable_id("source:v2:0"));
        assert_ne!(stable_id("patient-a:note"), stable_id("patient-b:note"));
        assert!(split(" \n\t").is_empty());
    }
}
