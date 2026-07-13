use crate::credential_store::DashenCredentials;
use anyhow::Result;
use serde_json::json;

const GET_REPORT_TOKEN_URL: &str = "https://inf.ds.163.com/v1/web/game/report/getReportToken";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleTagExtraction {
    Missing,
    Found(String),
    Conflict,
}

pub fn extract_battle_tag(candidates: &[String]) -> BattleTagExtraction {
    let mut found = None;

    for candidate in candidates {
        for line in candidate.lines() {
            let line = line.trim();
            let value = line
                .strip_prefix("战网昵称：")
                .or_else(|| line.strip_prefix("战网昵称:"))
                .map(str::trim);
            let Some(value) = value else {
                continue;
            };
            let Some((name, suffix)) = value.rsplit_once('#') else {
                continue;
            };
            if name.is_empty() || suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit())
            {
                continue;
            }

            match &found {
                Some(existing) if existing != value => return BattleTagExtraction::Conflict,
                Some(_) => {}
                None => found = Some(value.to_string()),
            }
        }
    }

    found.map_or(BattleTagExtraction::Missing, BattleTagExtraction::Found)
}

pub fn extract_role_id(candidates: &[String]) -> Option<String> {
    for candidate in candidates {
        if let Some(role_id) = extract_digits_after_key(candidate, "role_id") {
            return Some(role_id);
        }
        if let Some(role_id) = extract_digits_after_key(candidate, "roleid") {
            return Some(role_id);
        }
    }
    None
}

fn extract_digits_after_key(text: &str, key: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let mut start = 0;
    while let Some(offset) = lower[start..].find(key) {
        let key_end = start + offset + key.len();
        let mut digit_start = None;
        for (index, ch) in text[key_end..].char_indices() {
            if ch.is_ascii_digit() {
                digit_start = Some(key_end + index);
                break;
            }
            if !"=:\"' \t\r\n?&".contains(ch) {
                break;
            }
        }
        if let Some(digit_start) = digit_start {
            let digits = text[digit_start..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if digits.len() >= 5 {
                return Some(digits);
            }
        }
        start = key_end;
    }
    None
}

pub fn build_collect_candidates_script() -> String {
    r#"
(() => {
  const values = [];
  const add = (value) => {
    if (value === undefined || value === null) return;
    const text = String(value);
    if (text) values.push(text);
  };
  add(location.href);
  add(document.documentElement ? document.documentElement.innerText : '');
  for (const entry of performance.getEntriesByType('resource')) add(entry.name);
  for (const storage of [localStorage, sessionStorage]) {
    for (let index = 0; index < storage.length; index += 1) {
      const key = storage.key(index);
      add(key);
      add(storage.getItem(key));
    }
  }
  return values.slice(0, 1000);
})()
"#
    .trim()
    .to_string()
}

pub fn build_get_report_token_script(role_id: &str, server: i64) -> String {
    let payload = json!({
        "appKey": "bn",
        "roleId": role_id,
        "server": server.to_string(),
        "source": 1,
        "type": "yearly",
    });
    format!(
        r#"
(async () => {{
  const url = {url};
  const payload = {payload};
  function getCookie(name) {{
    return (
      document.cookie
        .split('; ')
        .find((row) => row.startsWith(name + '='))
        ?.split('=')
        .slice(1)
        .join('=') || ''
    );
  }}
  if (!window.sig || !window.sig.default) {{
    return {{ ok: false, error: 'window.sig.default is not available on this page yet' }};
  }}
  const body = JSON.stringify(payload);
  const sigMod = await window.sig.default();
  const signRaw = sigMod.gen_sign(body);
  const signObj = JSON.parse(signRaw);
  const xsrf = getCookie('GL-XSRF-TOKEN');
  const uid = getCookie('GOD_UUID');
  const deviceId =
    localStorage.getItem('ns-client-id') ||
    localStorage.getItem('ds-website-uuid') ||
    '';
  const resp = await fetch(url, {{
    method: 'POST',
    credentials: 'include',
    headers: {{
      'Content-Type': 'application/json;charset=UTF-8',
      'GL-ClientType': '61',
      'GL-DeviceId': deviceId,
      'GL-Uid': uid,
      'GL-X-XSRF-TOKEN': xsrf,
      'GL-CheckSum': signObj.sign,
      'GL-Nonce': String(signObj.timestamp),
    }},
    body,
  }});
  const text = await resp.text();
  let json;
  try {{
    json = JSON.parse(text);
  }} catch (error) {{
    return {{ ok: false, error: 'getReportToken returned non-JSON response', status: resp.status, text }};
  }}
  const result = json && json.result ? json.result : {{}};
  return {{
    ok: Boolean(resp.ok && result.token),
    status: resp.status,
    role_id: String(result.roleId || payload.roleId || ''),
    token: String(result.token || ''),
    error: result.token ? '' : String(json.msg || json.message || 'token missing'),
  }};
}})()
"#,
        url = serde_json::to_string(GET_REPORT_TOKEN_URL).unwrap(),
        payload = serde_json::to_string(&payload).unwrap()
    )
    .trim()
    .to_string()
}

pub fn is_signature_helper_missing(result: &serde_json::Value) -> bool {
    result
        .get("error")
        .and_then(|item| item.as_str())
        .is_some_and(|error| error.contains("window.sig.default is not available"))
}

pub fn credentials_from_report_result(
    result: &serde_json::Value,
    fallback_role_id: &str,
    server: i64,
    saved_at: &str,
) -> Result<Option<DashenCredentials>> {
    let ok = result
        .get("ok")
        .and_then(|item| item.as_bool())
        .unwrap_or(false);
    let token = result
        .get("token")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim();
    if !ok || token.is_empty() {
        return Ok(None);
    }
    let role_id = result
        .get("role_id")
        .or_else(|| result.get("roleId"))
        .and_then(|item| item.as_str())
        .unwrap_or(fallback_role_id)
        .parse::<i64>()?;
    Ok(Some(DashenCredentials {
        role_id,
        token: token.to_string(),
        customer_token: None,
        dts: None,
        server,
        saved_at: saved_at.to_string(),
    }))
}
