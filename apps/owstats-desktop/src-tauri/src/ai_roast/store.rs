use crate::models::AiMatchRoastView;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRoastRecord {
    pub match_id: String,
    pub award: String,
    pub commentary: String,
    pub used_fact_ids: Vec<String>,
    pub model: String,
    pub evaluation_rule_version: i64,
    pub material_version: i64,
    pub prompt_version: i64,
    pub created_at: String,
}

pub fn roast_for_match(conn: &Connection, match_id: &str) -> Result<Option<AiMatchRoastView>> {
    Ok(conn
        .query_row(
            "SELECT award, commentary FROM match_ai_roasts WHERE match_id = ?1",
            [match_id],
            |row| {
                Ok(AiMatchRoastView {
                    award: row.get(0)?,
                    commentary: row.get(1)?,
                })
            },
        )
        .optional()?)
}

pub fn save_roast_once(conn: &Connection, roast: &AiRoastRecord) -> Result<AiMatchRoastView> {
    let used_fact_ids_json = serde_json::to_string(&roast.used_fact_ids)?;
    conn.execute(
        r#"
        INSERT INTO match_ai_roasts (
            match_id, award, commentary, used_fact_ids_json, model,
            evaluation_rule_version, material_version, prompt_version, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(match_id) DO NOTHING
        "#,
        params![
            roast.match_id,
            roast.award,
            roast.commentary,
            used_fact_ids_json,
            roast.model,
            roast.evaluation_rule_version,
            roast.material_version,
            roast.prompt_version,
            roast.created_at,
        ],
    )?;
    roast_for_match(conn, &roast.match_id)?
        .ok_or_else(|| anyhow::anyhow!("锐评保存后未找到永久结果"))
}
