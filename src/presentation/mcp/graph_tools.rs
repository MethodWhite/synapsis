use serde_json::{Value, json};
use crate::infrastructure::database::Database;

fn search_entities(db: &Database, query: &str, limit: i32) -> Vec<(i64, String, String, i64)> {
    let conn = db.get_conn();
    let sql = "SELECT id, name, entity_type, mention_count FROM entities WHERE name LIKE ?1 LIMIT ?2";
    if let Ok(mut stmt) = conn.prepare(sql) {
        let search = format!("%{}%", query);
        if let Ok(rows) = stmt.query_map(rusqlite::params![search, limit], |row| {
            Ok((row.get::<_, i64>(0).unwrap_or(0),
                row.get::<_, String>(1).unwrap_or_default(),
                row.get::<_, String>(2).unwrap_or_default(),
                row.get::<_, i64>(3).unwrap_or(0)))
        }) {
            return rows.filter_map(|r| r.ok()).collect();
        }
    }
    vec![]
}

fn get_relations(db: &Database, entity_id: i64, limit: i32) -> Vec<(String, f64, i64, String, String)> {
    let conn = db.get_conn();
    let sql = "SELECT r.relation_type, r.weight, e2.id, e2.name, e2.entity_type
               FROM relations r
               JOIN entities e2 ON (CASE WHEN r.source_id = ?1 THEN r.target_id ELSE r.source_id END) = e2.id
               WHERE r.source_id = ?1 OR r.target_id = ?1 LIMIT ?2";
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![entity_id, entity_id, limit], |row| {
            Ok((row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, i64>(2).unwrap_or(0),
                row.get::<_, String>(3).unwrap_or_default(),
                row.get::<_, String>(4).unwrap_or_default()))
        }) {
            return rows.filter_map(|r| r.ok()).collect();
        }
    }
    vec![]
}

pub fn handle_graph_search(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let query = args["query"].as_str().unwrap_or("");
    if query.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing 'query'" }
        }));
    }
    let limit = args["limit"].as_u64().unwrap_or(10) as i32;

    let entities = search_entities(db, query, limit);
    let results: Vec<Value> = entities.into_iter().map(|(id, name, etype, count)| json!({
        "id": id, "name": name, "type": etype, "mention_count": count
    })).collect();

    Ok(json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).unwrap_or_default() }] }
    }))
}

pub fn handle_entity_expand(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let entity_id = args["entity_id"].as_i64().unwrap_or(0);
    if entity_id == 0 {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing or invalid 'entity_id'" }
        }));
    }

    let entities = search_entities(db, &entity_id.to_string(), 1);
    let entity_name = entities.first().map(|e| e.1.clone()).unwrap_or_default();
    if entity_name.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": format!("Entity {} not found", entity_id) }
        }));
    }

    let related = get_relations(db, entity_id, 50);
    let rel_json: Vec<Value> = related.into_iter().map(|(rtype, weight, nid, nname, ntype)| json!({
        "relation_type": rtype, "weight": weight,
        "entity_id": nid, "entity_name": nname, "entity_type": ntype
    })).collect();

    Ok(json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": format!(
            "Entity: {} (id={})\nRelated: {}\n{}",
            entity_name, entity_id, rel_json.len(),
            serde_json::to_string_pretty(&rel_json).unwrap_or_default()
        )}] }
    }))
}

pub fn handle_graph_context(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let query = args["query"].as_str().unwrap_or("");
    let depth = args["depth"].as_u64().unwrap_or(2) as usize;
    if query.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing 'query'" }
        }));
    }

    let entities = search_entities(db, query, 10);
    if entities.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": "<graph>\n0 entities\n</graph>" }] }
        }));
    }

    let mut parts = vec![format!("<graph>\n{} entities", entities.len())];
    for (eid, ename, etype, _) in &entities {
        parts.push(format!("[{}] {} (id={})", etype, ename, eid));
        let rels = get_relations(db, *eid, 20);
        for (rtype, _weight, _nid, nname, _ntype) in &rels {
            parts.push(format!("  - {} {}", rtype, nname));
        }
        if depth > 1 && !rels.is_empty() {
            for (_rtype, _weight, nid, nname, ntype) in &rels {
                parts.push(format!("    [{}] {}", ntype, nname));
                let rels2 = get_relations(db, *nid, 5);
                for (r2, _, _, n2, _) in &rels2 {
                    if n2 != ename {
                        parts.push(format!("      - {} {}", r2, n2));
                    }
                }
            }
        }
    }
    parts.push("</graph>".to_string());

    Ok(json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": parts.join("\n") }] }
    }))
}
