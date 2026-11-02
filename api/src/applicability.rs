//! Organization-authored form applicability. Rules come only from active templates.
//! Client stage is the client's status. The relevant episode is the current active
//! episode, otherwise the latest discharged episode. Nothing here infers a legal
//! or clinical requirement, and template edits do not rewrite response snapshots.

use crate::{access, records, AppState, Result};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

pub const DISCLAIMER: &str = "These items reflect active organization-authored template rules only. Nexus does not infer legal or clinical requirements.";

#[derive(Clone, Debug, PartialEq, Eq)]
struct EpisodeCandidate {
    id: Uuid,
    status: String,
    level_of_care: String,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TemplateRule {
    id: Uuid,
    name: String,
    applies_to_stage: String,
    applies_to_care_level: String,
    required_for_stage: bool,
    active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResponseFact {
    template_id: Uuid,
    episode_id: Option<Uuid>,
    status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct FormRequirementItem {
    template_id: Uuid,
    template_name: String,
    disposition: String,
    completion: String,
    response_status: Option<String>,
    reason: String,
    episode_id: Option<Uuid>,
    applies_to_stage: String,
    applies_to_care_level: String,
    required_for_stage: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Evaluation {
    relevant_episode_id: Option<Uuid>,
    relevant_stage: String,
    relevant_care_level: Option<String>,
    requirements: Vec<FormRequirementItem>,
}

#[derive(Serialize)]
struct FormRequirementsBody {
    client_id: Uuid,
    client_status: String,
    relevant_episode_id: Option<Uuid>,
    relevant_stage: String,
    relevant_care_level: Option<String>,
    disclaimer: &'static str,
    requirements: Vec<FormRequirementItem>,
}

fn pick(mut items: Vec<&EpisodeCandidate>, discharged: bool) -> Option<&EpisodeCandidate> {
    items.sort_by(|left, right| {
        let primary = if discharged {
            left.end_date.cmp(&right.end_date)
        } else {
            left.start_date.cmp(&right.start_date)
        };
        primary
            .then(left.start_date.cmp(&right.start_date))
            .then(left.created_at.cmp(&right.created_at))
            .then(left.id.cmp(&right.id))
    });
    items.pop()
}

fn select_relevant_episode(episodes: &[EpisodeCandidate]) -> Option<&EpisodeCandidate> {
    let active: Vec<_> = episodes
        .iter()
        .filter(|episode| episode.status == "active")
        .collect();
    if !active.is_empty() {
        return pick(active, false);
    }
    let discharged: Vec<_> = episodes
        .iter()
        .filter(|episode| episode.status == "discharged")
        .collect();
    if discharged.is_empty() {
        None
    } else {
        pick(discharged, true)
    }
}

fn rule_matches(rule: &TemplateRule, stage: &str, care_level: Option<&str>) -> bool {
    if !rule.active {
        return false;
    }
    let stage_ok = rule.applies_to_stage == "any" || rule.applies_to_stage == stage;
    let care_ok = rule.applies_to_care_level == "any"
        || care_level.is_some_and(|level| level == rule.applies_to_care_level);
    stage_ok && care_ok
}

fn rule_reason(rule: &TemplateRule) -> String {
    let expectation = if rule.required_for_stage {
        "Required by this organization"
    } else {
        "Suggested by this organization"
    };
    let stage = if rule.applies_to_stage == "any" {
        "any stage"
    } else {
        rule.applies_to_stage.as_str()
    };
    let care = match rule.applies_to_care_level.as_str() {
        "any" => "any care level",
        "iop" => "IOP",
        "php" => "PHP",
        other => other,
    };
    format!("{expectation} for {stage}, {care}. Based on your organization's configuration.")
}

fn completion_for(
    rule: &TemplateRule,
    relevant_episode: Option<Uuid>,
    responses: &[ResponseFact],
) -> (String, Option<String>) {
    let matching: Vec<&ResponseFact> = responses
        .iter()
        .filter(|response| {
            response.template_id == rule.id && response.episode_id == relevant_episode
        })
        .collect();
    if matching.iter().any(|response| response.status == "signed") {
        return ("signed".into(), Some("signed".into()));
    }
    if matching
        .iter()
        .any(|response| response.status == "reviewed")
    {
        return ("draft".into(), Some("reviewed".into()));
    }
    if matching.iter().any(|response| response.status == "draft") {
        return ("draft".into(), Some("draft".into()));
    }
    ("needed".into(), None)
}

fn evaluate(
    client_status: &str,
    episodes: &[EpisodeCandidate],
    templates: &[TemplateRule],
    responses: &[ResponseFact],
) -> Evaluation {
    let episode = select_relevant_episode(episodes);
    let relevant_episode_id = episode.map(|item| item.id);
    let relevant_care_level = episode.map(|item| item.level_of_care.clone());
    let mut requirements = Vec::new();
    for rule in templates {
        if !rule_matches(rule, client_status, relevant_care_level.as_deref()) {
            continue;
        }
        let (completion, response_status) = completion_for(rule, relevant_episode_id, responses);
        requirements.push(FormRequirementItem {
            template_id: rule.id,
            template_name: rule.name.clone(),
            disposition: if rule.required_for_stage {
                "required"
            } else {
                "suggested"
            }
            .into(),
            completion,
            response_status,
            reason: rule_reason(rule),
            episode_id: relevant_episode_id,
            applies_to_stage: rule.applies_to_stage.clone(),
            applies_to_care_level: rule.applies_to_care_level.clone(),
            required_for_stage: rule.required_for_stage,
        });
    }
    requirements.sort_by(|left, right| {
        let rank = |disposition: &str| if disposition == "required" { 0 } else { 1 };
        rank(&left.disposition)
            .cmp(&rank(&right.disposition))
            .then_with(|| {
                left.template_name
                    .to_lowercase()
                    .cmp(&right.template_name.to_lowercase())
            })
            .then(left.template_id.cmp(&right.template_id))
    });
    Evaluation {
        relevant_episode_id,
        relevant_stage: client_status.to_string(),
        relevant_care_level,
        requirements,
    }
}

fn map_episode(row: &sqlx::postgres::PgRow) -> EpisodeCandidate {
    let created: NaiveDateTime = row.get("created_at");
    EpisodeCandidate {
        id: row.get("id"),
        status: row.get("status"),
        level_of_care: row.get("level_of_care"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created, Utc),
    }
}

pub async fn client_requirements(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let client_id = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, "form_responses", false).await?;
    let mut tx = state.pool.begin().await?;
    let client = records::get(&mut tx, "clients", client_id, actor.org, false).await?;
    records::scope(&mut tx, &actor, "clients", &client).await?;
    let client_status = client
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let episodes = sqlx::query(
        "SELECT id, status, level_of_care, start_date, end_date, (created_at AT TIME ZONE 'UTC') AS created_at FROM episodes WHERE organization_id=$1 AND client_id=$2",
    )
    .bind(actor.org)
    .bind(client_id)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(map_episode)
    .collect::<Vec<_>>();
    let templates = sqlx::query(
        "SELECT id, name, applies_to_stage, applies_to_care_level, required_for_stage, active FROM templates WHERE organization_id=$1 AND active",
    )
    .bind(actor.org)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|row| TemplateRule {
        id: row.get("id"),
        name: row.get("name"),
        applies_to_stage: row.get("applies_to_stage"),
        applies_to_care_level: row.get("applies_to_care_level"),
        required_for_stage: row.get("required_for_stage"),
        active: row.get("active"),
    })
    .collect::<Vec<_>>();
    let responses = sqlx::query(
        "SELECT template_id, episode_id, status FROM form_responses WHERE organization_id=$1 AND client_id=$2",
    )
    .bind(actor.org)
    .bind(client_id)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|row| ResponseFact {
        template_id: row.get("template_id"),
        episode_id: row.get("episode_id"),
        status: row.get("status"),
    })
    .collect::<Vec<_>>();
    let evaluation = evaluate(&client_status, &episodes, &templates, &responses);
    access::audit(
        &mut tx,
        &actor,
        "read",
        "form_requirements",
        Some(client_id),
    )
    .await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok()
        .insert_header(("cache-control", "no-store, private"))
        .json(FormRequirementsBody {
            client_id,
            client_status,
            relevant_episode_id: evaluation.relevant_episode_id,
            relevant_stage: evaluation.relevant_stage,
            relevant_care_level: evaluation.relevant_care_level,
            disclaimer: DISCLAIMER,
            requirements: evaluation.requirements,
        }))
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate, rule_reason, select_relevant_episode, EpisodeCandidate, ResponseFact,
        TemplateRule, DISCLAIMER,
    };
    use chrono::{DateTime, NaiveDate, Utc};
    use uuid::Uuid;

    fn uid(kind: u8, n: u8) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes[0] = kind;
        bytes[15] = n;
        Uuid::from_bytes(bytes)
    }

    fn at(hour: u32) -> DateTime<Utc> {
        DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(hour, 0, 0)
                .unwrap(),
            Utc,
        )
    }

    fn day(year: i32, month: u32, dom: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, dom).unwrap()
    }

    fn episode(
        n: u8,
        status: &str,
        level: &str,
        start: (i32, u32, u32),
        end: Option<(i32, u32, u32)>,
        hour: u32,
    ) -> EpisodeCandidate {
        EpisodeCandidate {
            id: uid(2, n),
            status: status.into(),
            level_of_care: level.into(),
            start_date: day(start.0, start.1, start.2),
            end_date: end.map(|value| day(value.0, value.1, value.2)),
            created_at: at(hour),
        }
    }

    fn rule(
        n: u8,
        name: &str,
        stage: &str,
        care: &str,
        required: bool,
        active: bool,
    ) -> TemplateRule {
        TemplateRule {
            id: uid(1, n),
            name: name.into(),
            applies_to_stage: stage.into(),
            applies_to_care_level: care.into(),
            required_for_stage: required,
            active,
        }
    }

    fn response(template: u8, episode_id: Option<Uuid>, status: &str) -> ResponseFact {
        ResponseFact {
            template_id: uid(1, template),
            episode_id,
            status: status.into(),
        }
    }

    fn assert_non_inferring(reason: &str) {
        let lower = reason.to_lowercase();
        assert!(lower.ends_with("based on your organization's configuration."));
        for banned in [
            "legally required",
            "clinically necessary",
            "clinically required",
            "must comply",
            "compliance",
        ] {
            assert!(!lower.contains(banned), "{reason}");
        }
    }

    #[test]
    fn disclaimer_does_not_infer_requirements() {
        assert!(DISCLAIMER.contains("does not infer legal or clinical requirements"));
        assert_non_inferring(&format!("Noted. {DISCLAIMER}").replace(
            "These items reflect active organization-authored template rules only. Nexus does not infer legal or clinical requirements.",
            "Based on your organization's configuration.",
        ));
        assert_eq!(
            rule_reason(&rule(1, "Any packet", "any", "iop", false, true)),
            "Suggested by this organization for any stage, IOP. Based on your organization's configuration."
        );
        assert_eq!(
            rule_reason(&rule(2, "Zebra intake", "intake", "any", true, true)),
            "Required by this organization for intake, any care level. Based on your organization's configuration."
        );
    }

    #[test]
    fn intake_without_episode_counts_only_null_episode_signatures() {
        let planned = episode(9, "planned", "iop", (2020, 1, 1), None, 1);
        let templates = vec![
            rule(1, "Zebra intake", "intake", "any", true, true),
            rule(2, "Alpha suggested", "any", "any", false, true),
            rule(3, "IOP rule", "intake", "iop", true, true),
            rule(4, "Inactive rule", "intake", "any", true, false),
            rule(5, "Active only", "active", "any", true, true),
        ];
        let responses = vec![
            response(1, Some(planned.id), "signed"),
            response(2, None, "draft"),
        ];
        let eval = evaluate("intake", &[planned], &templates, &responses);
        assert!(eval.relevant_episode_id.is_none());
        assert_eq!(eval.relevant_stage, "intake");
        assert_eq!(eval.relevant_care_level, None);
        assert_eq!(
            eval.requirements
                .iter()
                .map(|item| item.template_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Zebra intake", "Alpha suggested"]
        );
        assert_eq!(eval.requirements[0].disposition, "required");
        assert_eq!(eval.requirements[0].completion, "needed");
        assert_eq!(eval.requirements[0].episode_id, None);
        assert_eq!(eval.requirements[0].reason, rule_reason(&templates[0]));
        assert_eq!(eval.requirements[1].disposition, "suggested");
        assert_eq!(eval.requirements[1].completion, "draft");
        assert_eq!(
            eval.requirements[1].response_status.as_deref(),
            Some("draft")
        );
        eval.requirements
            .iter()
            .for_each(|item| assert_non_inferring(&item.reason));

        let signed = evaluate(
            "intake",
            &[],
            &templates,
            &[response(1, None, "signed"), response(1, None, "draft")],
        );
        assert_eq!(signed.requirements[0].completion, "signed");
        assert!(evaluate("intake", &[], &[], &[response(1, None, "signed")])
            .requirements
            .is_empty());
    }

    #[test]
    fn relevant_episode_links_signatures_without_changing_client_stage() {
        let older = episode(1, "discharged", "iop", (2020, 1, 1), Some((2020, 2, 1)), 1);
        let newer = episode(2, "discharged", "php", (2020, 3, 1), Some((2020, 4, 1)), 1);
        let active = episode(3, "active", "outpatient", (2020, 5, 1), None, 2);
        assert_eq!(
            select_relevant_episode(&[older.clone(), newer.clone(), active.clone()])
                .unwrap()
                .id,
            active.id
        );
        assert_eq!(
            select_relevant_episode(&[older.clone(), newer.clone()])
                .unwrap()
                .id,
            newer.id
        );
        let mut later = newer.clone();
        later.id = uid(2, 4);
        later.created_at = at(8);
        let mut earlier = newer.clone();
        earlier.id = uid(2, 5);
        earlier.created_at = at(3);
        assert_eq!(
            select_relevant_episode(&[earlier, later.clone()])
                .unwrap()
                .id,
            later.id
        );

        let templates = vec![
            rule(1, "Any packet", "intake", "any", true, true),
            rule(2, "PHP rule", "intake", "php", true, true),
        ];
        let draft = evaluate(
            "intake",
            &[older.clone(), active.clone()],
            &templates,
            &[
                response(1, None, "signed"),
                response(1, Some(active.id), "reviewed"),
            ],
        );
        assert_eq!(draft.relevant_stage, "intake");
        assert_eq!(draft.relevant_episode_id, Some(active.id));
        assert_eq!(draft.relevant_care_level.as_deref(), Some("outpatient"));
        assert_eq!(draft.requirements.len(), 1);
        assert_eq!(draft.requirements[0].completion, "draft");
        assert_eq!(
            draft.requirements[0].response_status.as_deref(),
            Some("reviewed")
        );
        assert_eq!(draft.requirements[0].episode_id, Some(active.id));

        let signed = evaluate(
            "intake",
            std::slice::from_ref(&active),
            &templates,
            &[response(1, Some(active.id), "signed")],
        );
        assert_eq!(signed.requirements[0].completion, "signed");
    }

    #[test]
    fn unlisted_care_levels_and_inquiry_match_only_authored_any_rules() {
        let residential = episode(6, "active", "residential", (2021, 1, 1), None, 1);
        let templates = vec![
            rule(1, "beta form", "any", "any", false, true),
            rule(2, "Alpha form", "active", "any", false, true),
            rule(3, "Zebra required", "active", "php", true, true),
            rule(4, "Inactive required", "active", "any", true, false),
        ];
        let eval = evaluate("active", &[residential], &templates, &[]);
        assert_eq!(
            eval.requirements
                .iter()
                .map(|item| item.template_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha form", "beta form"]
        );
        assert!(eval
            .requirements
            .iter()
            .all(|item| item.disposition == "suggested"));
        assert!(evaluate("inquiry", &[], &templates, &[])
            .requirements
            .iter()
            .all(|item| item.template_name == "beta form"));
        assert!(evaluate(
            "discharged",
            &[],
            &[rule(9, "PHP only", "discharged", "php", true, true)],
            &[]
        )
        .requirements
        .is_empty());
    }
}
