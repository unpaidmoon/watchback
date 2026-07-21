use crate::credential_store::DashenCredentials;
use anyhow::Result;
use serde_json::json;
use std::collections::BTreeSet;

const GET_REPORT_TOKEN_URL: &str = "https://inf.ds.163.com/v1/web/game/report/getReportToken";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleTagExtraction {
    Missing,
    Found(String),
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleIdExtraction {
    Missing,
    Found(String),
    Conflict,
}

pub fn extract_battle_tag(page_text: &str) -> BattleTagExtraction {
    let mut candidates = Vec::new();

    for line in page_text.lines() {
        let line = line.trim();
        let Some(after_label) = line.strip_prefix("战网昵称") else {
            continue;
        };
        let after_label = after_label.trim_start();
        let after_colon = after_label
            .strip_prefix('：')
            .or_else(|| after_label.strip_prefix(':'));
        let Some(value) = after_colon.and_then(|text| text.split_whitespace().next()) else {
            continue;
        };
        candidates.push(value.to_string());
    }

    extract_battle_tag_candidates(&candidates, false)
}

pub fn extract_battle_tag_candidates(
    candidates: &[String],
    truncated: bool,
) -> BattleTagExtraction {
    if truncated {
        return BattleTagExtraction::Conflict;
    }

    let mut found = BTreeSet::new();
    for candidate in candidates {
        let value = candidate.trim();
        let Some((name, suffix)) = value.split_once('#') else {
            continue;
        };
        if name.is_empty()
            || suffix.is_empty()
            || suffix.contains('#')
            || !suffix.chars().all(|ch| ch.is_ascii_digit())
        {
            continue;
        }
        found.insert(value.to_string());
    }

    match found.len() {
        0 => BattleTagExtraction::Missing,
        1 => BattleTagExtraction::Found(found.into_iter().next().expect("one BattleTag")),
        _ => BattleTagExtraction::Conflict,
    }
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

pub fn extract_role_id_for_battle_tag(candidates: &[String], battle_tag: &str) -> RoleIdExtraction {
    let battle_tag = battle_tag.trim();
    if battle_tag.is_empty() {
        return RoleIdExtraction::Missing;
    }

    let role_ids = candidates
        .iter()
        .filter(|candidate| contains_exact_battle_tag(candidate, battle_tag))
        .flat_map(|candidate| {
            extract_all_digits_after_key(candidate, "role_id")
                .into_iter()
                .chain(extract_all_digits_after_key(candidate, "roleid"))
        })
        .collect::<BTreeSet<_>>();

    match role_ids.len() {
        0 => RoleIdExtraction::Missing,
        1 => RoleIdExtraction::Found(role_ids.into_iter().next().expect("one role id")),
        _ => RoleIdExtraction::Conflict,
    }
}

fn contains_exact_battle_tag(candidate: &str, battle_tag: &str) -> bool {
    candidate.match_indices(battle_tag).any(|(start, _)| {
        let before = candidate[..start].chars().next_back();
        let end = start + battle_tag.len();
        let after = candidate[end..].chars().next();
        let is_boundary = |character: Option<char>| {
            character.is_none_or(|character| !character.is_alphanumeric() && character != '#')
        };
        is_boundary(before) && is_boundary(after)
    })
}

pub fn extract_login_role_id(
    current_candidates: &[String],
    storage_candidates: &[String],
    resource_candidates: &[String],
    evidence_truncated: bool,
) -> RoleIdExtraction {
    if evidence_truncated {
        return RoleIdExtraction::Conflict;
    }

    let current_role_ids = extract_role_ids(current_candidates);
    let auxiliary_role_ids = extract_role_ids(
        &storage_candidates
            .iter()
            .chain(resource_candidates)
            .cloned()
            .collect::<Vec<_>>(),
    );

    if current_role_ids.len() > 1 || (current_role_ids.is_empty() && auxiliary_role_ids.len() > 1) {
        return RoleIdExtraction::Conflict;
    }

    let Some(current_role_id) = current_role_ids.into_iter().next() else {
        return RoleIdExtraction::Missing;
    };

    RoleIdExtraction::Found(current_role_id)
}

fn extract_role_ids(candidates: &[String]) -> BTreeSet<String> {
    let mut role_ids = BTreeSet::new();
    for candidate in candidates {
        role_ids.extend(extract_all_digits_after_key(candidate, "role_id"));
        role_ids.extend(extract_all_digits_after_key(candidate, "roleid"));
    }
    role_ids
}

fn extract_digits_after_key(text: &str, key: &str) -> Option<String> {
    extract_all_digits_after_key(text, key).into_iter().next()
}

fn extract_all_digits_after_key(text: &str, key: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut start = 0;
    let mut found = Vec::new();
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
                start = digit_start + digits.len();
                found.push(digits);
                continue;
            }
        }
        start = key_end;
    }
    found
}

pub fn build_collect_candidates_script() -> String {
    r#"
(() => {
  const MAX_ROLE_EVIDENCE_VALUES = 500;
  const MAX_BATTLETAG_CANDIDATES = 20;
  const pageText = document.documentElement?.innerText ?? '';
  const createGroup = () => ({ values: [], truncated: false });
  const currentGroup = createGroup();
  const storageGroup = createGroup();
  const resourceGroup = createGroup();
  const add = (group, value, limit = MAX_ROLE_EVIDENCE_VALUES) => {
    if (value === undefined || value === null) return;
    const text = String(value);
    if (!text) return;
    if (group.values.length >= limit) {
      group.truncated = true;
      return;
    }
    group.values.push(text);
  };
  const collectBattleTags = (doc, group) => {
    if (!doc?.body || group.truncated) return;
    const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT);
    const textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);
    for (let index = 0; index < textNodes.length; index += 1) {
      const text = String(textNodes[index].nodeValue ?? '');
      const label = ['战网昵称', '战网昵称']
        .find((candidate) => text.includes(candidate));
      if (!label) continue;
      const labelIndex = text.indexOf(label);
      let evidence = text.slice(labelIndex + label.length);
      for (let lookahead = 1; lookahead <= 4; lookahead += 1) {
        const match = evidence.match(/^\s*[:：]\s*([^\s#]+#[0-9]+)(?![0-9A-Za-z#])/);
        if (match) {
          add(group, match[1], MAX_BATTLETAG_CANDIDATES);
          break;
        }
        const nextNode = textNodes[index + lookahead];
        if (!nextNode) break;
        evidence += ` ${String(nextNode.nodeValue ?? '')}`;
      }
    }
    for (const iframe of doc.querySelectorAll('iframe')) {
      try {
        collectBattleTags(iframe.contentDocument, group);
      } catch (_) {
        // Cross-origin frames are intentionally inaccessible.
      }
    }
  };
  const scanBattleTags = () => {
    const next = createGroup();
    collectBattleTags(document, next);
    return next;
  };
  const captureKey = '__owstatsBattleTagCapture';
  let capture = window[captureKey];
  if (!capture || !Array.isArray(capture.values)) {
    capture = { values: [], truncated: false, observer: null };
    Object.defineProperty(window, captureKey, {
      value: capture,
      configurable: true,
    });
  }
  const updateCapture = () => {
    const next = scanBattleTags();
    if (next.values.length > 0 || next.truncated) {
      capture.values = next.values;
      capture.truncated = next.truncated;
    }
  };
  updateCapture();
  if (!capture.observer) {
    capture.observer = new MutationObserver(updateCapture);
    capture.observer.observe(document.documentElement ?? document, {
      childList: true,
      subtree: true,
      characterData: true,
    });
  }
  const battleTagGroup = {
    values: [...capture.values],
    truncated: capture.truncated,
  };
  add(currentGroup, location.href);
  add(currentGroup, pageText);
  for (const storage of [localStorage, sessionStorage]) {
    for (let index = 0; index < storage.length; index += 1) {
      const key = storage.key(index);
      add(storageGroup, key);
      add(storageGroup, storage.getItem(key));
    }
  }
  for (const entry of performance.getEntriesByType('resource')) {
    add(resourceGroup, entry.name);
  }
  return {
    pageText: pageText,
    battleTagCandidates: battleTagGroup,
    roleCandidates: {
      current: currentGroup,
      storage: storageGroup,
      resources: resourceGroup,
    },
  };
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
