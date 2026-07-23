use super::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) fn assign_party_groups(source: &[Value], rows: Vec<PlayerRow>) -> Vec<PlayerRow> {
    let mut first_index = HashMap::new();
    for (index, item) in source.iter().enumerate() {
        if let Some(id) = string_field(item, "bnetId").filter(|id| !id.is_empty()) {
            first_index.entry(id).or_insert(index);
        }
    }

    let mut edges = vec![HashSet::<usize>::new(); source.len()];
    for (index, item) in source.iter().enumerate() {
        let Some(friends) = item.get("friendBnetIds").and_then(Value::as_array) else {
            continue;
        };
        for friend in friends.iter().filter_map(party_id) {
            let Some(&other) = first_index.get(&friend) else {
                continue;
            };
            if other == index {
                continue;
            }
            edges[index].insert(other);
            edges[other].insert(index);
        }
    }

    let mut component_for = vec![None; source.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for start in 0..source.len() {
        if component_for[start].is_some() || edges[start].is_empty() {
            continue;
        }
        let component_id = components.len();
        let mut queue = VecDeque::from([start]);
        let mut members = Vec::new();
        component_for[start] = Some(component_id);
        while let Some(index) = queue.pop_front() {
            members.push(index);
            for &next in &edges[index] {
                if component_for[next].is_none() {
                    component_for[next] = Some(component_id);
                    queue.push_back(next);
                }
            }
        }
        members.sort_unstable();
        components.push(members);
    }

    let mut rows = rows.into_iter().map(Some).collect::<Vec<_>>();
    let mut emitted = HashSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for index in 0..rows.len() {
        if rows[index].is_none() {
            continue;
        }
        if let Some(component_id) = component_for.get(index).copied().flatten() {
            if emitted.insert(component_id) {
                for &member in &components[component_id] {
                    if let Some(mut row) = rows.get_mut(member).and_then(Option::take) {
                        row.party_group = Some(component_id);
                        result.push(row);
                    }
                }
            }
        } else if let Some(row) = rows[index].take() {
            result.push(row);
        }
    }
    result.extend(rows.into_iter().flatten());
    result
}

fn party_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(id: &str) -> PlayerRow {
        PlayerRow {
            side: "teammate".into(),
            name: id.into(),
            bnet_id: id.into(),
            rank_label: None,
            rank_key: None,
            hero_guid: String::new(),
            hero_name: "测试英雄".into(),
            kills: 0,
            assists: 0,
            deaths: 0,
            damage: 0,
            healing: 0,
            mitigation: 0,
            final_hits: 0,
            target_time_sec: 0,
            healing_taken: 0,
            damage_taken: 0,
            worth: None,
            end_worth: None,
            worth_max: false,
            mod_guids: Vec::new(),
            trait_guids: Vec::new(),
            has_source_max_flags: false,
            kill_max: false,
            hero_damage_max: false,
            cure_max: false,
            resist_damage_max: false,
            perks: Vec::new(),
            hero_stats: Vec::new(),
            party_group: None,
        }
    }

    #[test]
    fn groups_unilateral_transitive_links_and_preserves_first_seen_order() {
        let source = vec![
            json!({"bnetId":"solo-a","friendBnetIds":[]}),
            json!({"bnetId":"party-a","friendBnetIds":["party-b"]}),
            json!({"bnetId":"solo-b","friendBnetIds":["enemy-x", "missing-x"]}),
            json!({"bnetId":"party-c","friendBnetIds":[]}),
            json!({"bnetId":"party-b","friendBnetIds":["party-c", "party-c"]}),
        ];
        let rows = source
            .iter()
            .map(|item| row(item["bnetId"].as_str().unwrap()))
            .collect();

        let grouped = assign_party_groups(&source, rows);

        assert_eq!(
            grouped
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec!["solo-a", "party-a", "party-c", "party-b", "solo-b"]
        );
        assert_eq!(
            grouped
                .iter()
                .map(|row| row.party_group)
                .collect::<Vec<_>>(),
            vec![None, Some(0), Some(0), Some(0), None]
        );
    }

    #[test]
    fn keeps_multiple_parties_separate_and_solos_unmarked() {
        let source = vec![
            json!({"bnetId":"a","friendBnetIds":["b"]}),
            json!({"bnetId":"c","friendBnetIds":["d"]}),
            json!({"bnetId":"solo"}),
            json!({"bnetId":"b","friendBnetIds":[]}),
            json!({"bnetId":"d","friendBnetIds":[]}),
        ];
        let rows = source
            .iter()
            .map(|item| row(item["bnetId"].as_str().unwrap()))
            .collect();

        let grouped = assign_party_groups(&source, rows);

        assert_eq!(
            grouped
                .iter()
                .map(|row| (row.name.as_str(), row.party_group))
                .collect::<Vec<_>>(),
            vec![
                ("a", Some(0)),
                ("b", Some(0)),
                ("c", Some(1)),
                ("d", Some(1)),
                ("solo", None),
            ]
        );
    }
}
