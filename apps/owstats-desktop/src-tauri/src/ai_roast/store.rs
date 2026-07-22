use std::collections::BTreeSet;

use crate::ai_roast::report::StoredRoastPayload;
use crate::models::AiMatchRoastView;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRoastRecord {
    pub match_id: String,
    pub award: String,
    pub commentary: String,
    pub report: Option<StoredRoastPayload>,
    pub used_fact_ids: Vec<String>,
    pub model: String,
    pub evaluation_rule_version: i64,
    pub material_version: i64,
    pub prompt_version: i64,
    pub report_schema_version: Option<i64>,
    pub input_schema_version: Option<i64>,
    pub knowledge_version: Option<i64>,
    pub created_at: String,
}

pub fn roast_for_match(conn: &Connection, match_id: &str) -> Result<Option<AiMatchRoastView>> {
    let row = conn
        .query_row(
            "SELECT award,commentary,report_json FROM match_ai_roasts WHERE match_id=?1",
            [match_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((award, commentary, report_json)) = row else {
        return Ok(None);
    };
    let payload = report_json
        .map(|json| {
            serde_json::from_str::<StoredRoastPayload>(&json)
                .map_err(|_| anyhow!("结构化锐评记录无效"))
        })
        .transpose()?;
    let report = match payload {
        Some(StoredRoastPayload::Legacy(report)) => Some(report),
        _ => None,
    };
    Ok(Some(AiMatchRoastView {
        award,
        commentary,
        report,
    }))
}

pub fn save_roast_once(conn: &Connection, roast: &AiRoastRecord) -> Result<AiMatchRoastView> {
    let values = serialized_values(roast)?;
    conn.execute(r#"INSERT INTO match_ai_roasts(match_id,award,commentary,used_fact_ids_json,model,evaluation_rule_version,material_version,prompt_version,created_at,report_json,report_schema_version,input_schema_version,knowledge_version)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(match_id) DO NOTHING"#,params![roast.match_id,roast.award,roast.commentary,values.used_ids,roast.model,roast.evaluation_rule_version,roast.material_version,roast.prompt_version,roast.created_at,values.report_json,roast.report_schema_version,roast.input_schema_version,roast.knowledge_version])?;
    roast_for_match(conn, &roast.match_id)?.ok_or_else(|| anyhow!("锐评保存后未找到永久结果"))
}

pub fn replace_roast(conn: &Connection, roast: &AiRoastRecord) -> Result<AiMatchRoastView> {
    let values = serialized_values(roast)?;
    conn.execute(r#"INSERT INTO match_ai_roasts(match_id,award,commentary,used_fact_ids_json,model,evaluation_rule_version,material_version,prompt_version,created_at,report_json,report_schema_version,input_schema_version,knowledge_version)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
        ON CONFLICT(match_id) DO UPDATE SET award=excluded.award,commentary=excluded.commentary,used_fact_ids_json=excluded.used_fact_ids_json,model=excluded.model,evaluation_rule_version=excluded.evaluation_rule_version,material_version=excluded.material_version,prompt_version=excluded.prompt_version,created_at=excluded.created_at,report_json=excluded.report_json,report_schema_version=excluded.report_schema_version,input_schema_version=excluded.input_schema_version,knowledge_version=excluded.knowledge_version"#,params![roast.match_id,roast.award,roast.commentary,values.used_ids,roast.model,roast.evaluation_rule_version,roast.material_version,roast.prompt_version,roast.created_at,values.report_json,roast.report_schema_version,roast.input_schema_version,roast.knowledge_version])?;
    roast_for_match(conn, &roast.match_id)?.ok_or_else(|| anyhow!("锐评保存后未找到永久结果"))
}

struct SerializedValues {
    used_ids: String,
    report_json: Option<String>,
}
fn serialized_values(roast: &AiRoastRecord) -> Result<SerializedValues> {
    let ids = if let Some(report) = &roast.report {
        report
            .evidence_refs()
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        roast
            .used_fact_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    Ok(SerializedValues {
        used_ids: serde_json::to_string(&ids)?,
        report_json: roast
            .report
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?,
    })
}
