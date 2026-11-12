//! Points de restauration et fichiers portables de projet (section 13.2).
//!
//! Un instantané couvre toutes les tables d'un projet, trouvées en lisant le schéma : les
//! tables qui ont une colonne `project_id` et leurs tables filles (clé étrangère vers l'une
//! d'elles). Une table ajoutée plus tard est donc prise sans changer ce code. La restauration
//! ne garde que les colonnes connues de la base courante : un instantané d'une autre version
//! reste lisible.

use crate::{Result, Store, StoreError, new_id, now};
use base64::Engine;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const FORMAT: &str = "strata-project";
/// Tables d'un projet jamais copiées : journal et verrous d'édition.
const SKIP: [&str; 3] = ["audit", "locks", "project_points"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Point {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub label: Option<String>,
    pub auto: bool,
    pub created_at: String,
    pub created_by: Option<String>,
    pub size: i64,
}

/// Une table du projet et la condition SQL qui sélectionne ses lignes (paramètre : projet).
#[derive(Debug, Clone)]
struct Scoped {
    table: String,
    cond: String,
    params: usize,
}

fn point(r: &sqlx::sqlite::SqliteRow) -> Point {
    Point {
        id: r.get("id"),
        project_id: r.get("project_id"),
        project_name: r.get("project_name"),
        label: r.get("label"),
        auto: r.get::<i64, _>("auto") != 0,
        created_at: r.get("created_at"),
        created_by: r.get("created_by"),
        size: r.get("size"),
    }
}

pub fn pack(v: &Value) -> Vec<u8> {
    zstd::encode_all(serde_json::to_vec(v).unwrap_or_default().as_slice(), 9).unwrap_or_default()
}

pub fn unpack(b: &[u8]) -> Option<Value> {
    let raw = zstd::decode_all(b).ok()?;
    serde_json::from_slice(&raw).ok()
}

impl Store {
    async fn tables(&self) -> Result<Vec<(String, Vec<String>, Vec<(String, String, String)>)>> {
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx%' ORDER BY name")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for n in names {
            let cols: Vec<String> = sqlx::query(&format!("PRAGMA table_info(\"{n}\")"))
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| r.get::<String, _>("name"))
                .collect();
            // (colonne, table parente, colonne parente)
            let fks: Vec<(String, String, String)> =
                sqlx::query(&format!("PRAGMA foreign_key_list(\"{n}\")"))
                    .fetch_all(&self.pool)
                    .await?
                    .iter()
                    .map(|r| {
                        (
                            r.get::<String, _>("from"),
                            r.get::<String, _>("table"),
                            r.get::<Option<String>, _>("to")
                                .unwrap_or_else(|| "id".into()),
                        )
                    })
                    .collect();
            out.push((n, cols, fks));
        }
        Ok(out)
    }

    async fn scoped(
        &self,
    ) -> Result<(Vec<Scoped>, HashMap<String, Vec<(String, String, String)>>)> {
        let all = self.tables().await?;
        let mut out: Vec<Scoped> = Vec::new();
        let fks: HashMap<String, Vec<(String, String, String)>> =
            all.iter().map(|(n, _, f)| (n.clone(), f.clone())).collect();
        out.push(Scoped {
            table: "projects".into(),
            cond: "id = ?".into(),
            params: 1,
        });
        for (n, cols, _) in &all {
            if SKIP.contains(&n.as_str()) || n == "projects" {
                continue;
            }
            if cols.iter().any(|c| c == "project_id") {
                out.push(Scoped {
                    table: n.clone(),
                    cond: "project_id = ?".into(),
                    params: 1,
                });
            } else if n == "project_links" {
                out.push(Scoped {
                    table: n.clone(),
                    cond: "a_project = ? OR b_project = ?".into(),
                    params: 2,
                });
            }
        }
        // Tables filles sans colonne projet : par leur clé étrangère vers une table du projet.
        let direct: HashMap<String, Scoped> =
            out.iter().map(|s| (s.table.clone(), s.clone())).collect();
        for (n, cols, f) in &all {
            if SKIP.contains(&n.as_str())
                || direct.contains_key(n)
                || cols.iter().any(|c| c == "project_id")
            {
                continue;
            }
            let links: Vec<&(String, String, String)> = f
                .iter()
                .filter(|(_, p, _)| direct.contains_key(p) && p != "projects")
                .collect();
            if !links.is_empty() {
                out.push(Scoped {
                    table: n.clone(),
                    cond: links
                        .iter()
                        .map(|(col, parent, pcol)| {
                            format!(
                                "\"{col}\" IN (SELECT \"{pcol}\" FROM \"{parent}\" WHERE {})",
                                direct[parent].cond
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" OR "),
                    params: links
                        .iter()
                        .map(|(_, parent, _)| direct[parent].params)
                        .sum(),
                });
            }
        }
        Ok((out, fks))
    }

    /// Instantané complet d'un projet.
    pub async fn project_snapshot(&self, project_id: &str) -> Result<Value> {
        let (scoped, _) = self.scoped().await?;
        let mut tables = Map::new();
        for s in &scoped {
            let sql = format!("SELECT * FROM \"{}\" WHERE {}", s.table, s.cond);
            let mut q = sqlx::query(&sql);
            for _ in 0..s.params {
                q = q.bind(project_id);
            }
            let rows = q.fetch_all(&self.pool).await?;
            let mut list = Vec::new();
            for r in &rows {
                let mut o = Map::new();
                for (i, c) in r.columns().iter().enumerate() {
                    let raw = r.try_get_raw(i).map_err(StoreError::Db)?;
                    let v = if raw.is_null() {
                        Value::Null
                    } else {
                        match raw.type_info().name() {
                            "INTEGER" => json!(r.get::<i64, _>(i)),
                            "REAL" => json!(r.get::<f64, _>(i)),
                            "BLOB" => {
                                json!({ "$b": base64::engine::general_purpose::STANDARD.encode(r.get::<Vec<u8>, _>(i)) })
                            }
                            _ => json!(r.get::<String, _>(i)),
                        }
                    };
                    o.insert(c.name().to_string(), v);
                }
                list.push(Value::Object(o));
            }
            tables.insert(s.table.clone(), Value::Array(list));
        }
        let schema: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0);
        Ok(
            json!({ "format": FORMAT, "version": 1, "schema": schema, "project_id": project_id, "taken_at": now(), "tables": tables }),
        )
    }

    /// Remet un projet dans l'état d'un instantané (même s'il a été supprimé), en une seule
    /// transaction. Les colonnes inconnues de la base sont ignorées ; une référence vers un
    /// objet extérieur au projet qui n'existe plus est vidée (ou la ligne écartée).
    pub async fn restore_snapshot(&self, snap: &Value) -> Result<String> {
        let pid = snap["project_id"]
            .as_str()
            .ok_or(StoreError::NotFound)?
            .to_string();
        let tables = snap["tables"].as_object().ok_or(StoreError::NotFound)?;
        let all = self.tables().await?;
        let cols: HashMap<&str, HashSet<&str>> = all
            .iter()
            .map(|(n, c, _)| (n.as_str(), c.iter().map(String::as_str).collect()))
            .collect();
        let fks: HashMap<&str, &Vec<(String, String, String)>> =
            all.iter().map(|(n, _, f)| (n.as_str(), f)).collect();
        let mut tx = self.pool.begin().await?;
        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut *tx)
            .await?;
        // Le projet part avec tout ce qui en dépend (suppressions en cascade).
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(&pid)
            .execute(&mut *tx)
            .await?;
        let snap_tables: HashSet<&str> = tables.keys().map(String::as_str).collect();
        // Lignes de l'instantané, pour vérifier les références internes.
        let mut present: HashMap<(&str, &str), HashSet<String>> = HashMap::new();
        for (t, rows) in tables {
            for r in rows.as_array().into_iter().flatten() {
                for (c, v) in r.as_object().into_iter().flatten() {
                    if let Some(s) = v.as_str() {
                        present
                            .entry((t.as_str(), c.as_str()))
                            .or_default()
                            .insert(s.to_string());
                    }
                }
            }
        }
        // Ordre : parents d'abord (contraintes différées de toute façon).
        let mut order: Vec<&String> = tables.keys().collect();
        order.sort_by_key(|t| if t.as_str() == "projects" { 0 } else { 1 });
        for t in order {
            let Some(known) = cols.get(t.as_str()) else {
                continue;
            };
            if SKIP.contains(&t.as_str()) {
                continue;
            }
            'rows: for r in tables[t].as_array().into_iter().flatten() {
                let Some(o) = r.as_object() else { continue };
                let mut o = o.clone();
                for (col, parent, pcol) in fks.get(t.as_str()).map(|v| v.as_slice()).unwrap_or(&[])
                {
                    let Some(val) = o.get(col).and_then(Value::as_str).map(str::to_string) else {
                        continue;
                    };
                    let inside = snap_tables.contains(parent.as_str())
                        && present
                            .get(&(parent.as_str(), pcol.as_str()))
                            .is_some_and(|s| s.contains(&val));
                    if inside {
                        continue;
                    }
                    let exists: Option<i64> = sqlx::query_scalar(&format!(
                        "SELECT 1 FROM \"{parent}\" WHERE \"{pcol}\" = ?"
                    ))
                    .bind(&val)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if exists.is_none() {
                        // Une référence nullable est vidée ; sinon la ligne est écartée.
                        let nullable = !matches!(
                            col.as_str(),
                            "project_id"
                                | "a_project"
                                | "b_project"
                                | "a_cable"
                                | "b_cable"
                                | "map_id"
                                | "point_id"
                                | "target_id"
                                | "floor_id"
                                | "container_id"
                                | "article_id"
                                | "regie_id"
                                | "cable_id"
                        );
                        if nullable {
                            o.insert(col.clone(), Value::Null);
                        } else {
                            continue 'rows;
                        }
                    }
                }
                let keys: Vec<&String> = o.keys().filter(|k| known.contains(k.as_str())).collect();
                if keys.is_empty() {
                    continue;
                }
                let sql = format!(
                    "INSERT OR REPLACE INTO \"{t}\" ({}) VALUES ({})",
                    keys.iter()
                        .map(|k| format!("\"{k}\""))
                        .collect::<Vec<_>>()
                        .join(", "),
                    vec!["?"; keys.len()].join(", ")
                );
                let mut q = sqlx::query(&sql);
                for k in &keys {
                    q = match &o[k.as_str()] {
                        Value::Null => q.bind(None::<String>),
                        Value::Bool(b) => q.bind(*b as i64),
                        Value::Number(n) if n.is_i64() => q.bind(n.as_i64()),
                        Value::Number(n) => q.bind(n.as_f64()),
                        Value::String(s) => q.bind(s.clone()),
                        Value::Object(m) if m.contains_key("$b") => q.bind(
                            base64::engine::general_purpose::STANDARD
                                .decode(m["$b"].as_str().unwrap_or(""))
                                .unwrap_or_default(),
                        ),
                        other => q.bind(other.to_string()),
                    };
                }
                q.execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(pid)
    }

    pub async fn insert_point(
        &self,
        snap: &Value,
        name: &str,
        label: Option<&str>,
        auto: bool,
        by: Option<&str>,
    ) -> Result<Point> {
        let data = pack(snap);
        let id = new_id();
        let pid = snap["project_id"].as_str().unwrap_or_default().to_string();
        sqlx::query("INSERT INTO project_points (id, project_id, project_name, label, auto, created_at, created_by, size, data) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(&pid).bind(name).bind(label).bind(auto as i64).bind(now()).bind(by).bind(data.len() as i64).bind(&data)
            .execute(&self.pool).await?;
        if auto {
            // On garde les 48 derniers points automatiques du projet.
            sqlx::query("DELETE FROM project_points WHERE project_id = ? AND auto = 1 AND id NOT IN (SELECT id FROM project_points WHERE project_id = ? AND auto = 1 ORDER BY created_at DESC LIMIT 48)")
                .bind(&pid).bind(&pid).execute(&self.pool).await?;
        }
        self.points_of(&pid)
            .await?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or(StoreError::NotFound)
    }

    pub async fn points_of(&self, project_id: &str) -> Result<Vec<Point>> {
        let rows = sqlx::query("SELECT id, project_id, project_name, label, auto, created_at, created_by, size FROM project_points WHERE project_id = ? ORDER BY created_at DESC")
            .bind(project_id).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(point).collect())
    }

    /// Dernier point de chaque projet supprimé.
    pub async fn orphan_points(&self) -> Result<Vec<Point>> {
        let rows = sqlx::query("SELECT id, project_id, project_name, label, auto, created_at, created_by, size FROM project_points p WHERE NOT EXISTS (SELECT 1 FROM projects WHERE id = p.project_id) ORDER BY created_at DESC")
            .fetch_all(&self.pool).await?;
        Ok(rows.iter().map(point).collect())
    }

    pub async fn point(&self, id: &str) -> Result<Option<(Point, Vec<u8>)>> {
        let r = sqlx::query("SELECT * FROM project_points WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(r.map(|r| (point(&r), r.get::<Vec<u8>, _>("data"))))
    }

    pub async fn delete_point(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM project_points WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Projets à doter d'un point automatique : modifiés depuis leur dernier point, et ce
    /// dernier point plus vieux que l'intervalle.
    pub async fn projects_due(&self, interval_h: i64) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "SELECT p.id, p.name, (SELECT MAX(at) FROM audit a WHERE a.project_id = p.id) AS changed, (SELECT MAX(created_at) FROM project_points t WHERE t.project_id = p.id AND t.auto = 1) AS last FROM projects p",
        )
        .fetch_all(&self.pool)
        .await?;
        let now = chrono::Utc::now();
        let parse = |s: Option<String>| {
            s.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&chrono::Utc))
        };
        Ok(rows
            .iter()
            .filter_map(|r| {
                let changed = parse(r.get("changed"));
                let last = parse(r.get("last"));
                let due = match (changed, last) {
                    (None, _) => false,
                    (Some(_), None) => true,
                    (Some(c), Some(l)) => c > l && (now - l).num_minutes() >= interval_h * 60 - 1,
                };
                due.then(|| (r.get("id"), r.get("name")))
            })
            .collect())
    }

    /// Fiches du catalogue citées par un instantané (pour un fichier portable).
    pub async fn catalog_rows_for(&self, snap: &Value) -> Result<Vec<Value>> {
        let text = snap.to_string();
        let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM catalog_items")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for id in ids.into_iter().filter(|i| text.contains(i.as_str())) {
            let r = sqlx::query("SELECT * FROM catalog_items WHERE id = ?")
                .bind(&id)
                .fetch_one(&self.pool)
                .await?;
            let mut o = BTreeMap::new();
            for (i, c) in r.columns().iter().enumerate() {
                let raw = r.try_get_raw(i).map_err(StoreError::Db)?;
                let v = if raw.is_null() {
                    Value::Null
                } else {
                    match raw.type_info().name() {
                        "INTEGER" => json!(r.get::<i64, _>(i)),
                        "REAL" => json!(r.get::<f64, _>(i)),
                        _ => json!(r.get::<String, _>(i)),
                    }
                };
                o.insert(c.name().to_string(), v);
            }
            out.push(json!(o));
        }
        Ok(out)
    }

    /// Ajoute les fiches manquantes ; une fiche déjà présente n'est jamais modifiée.
    pub async fn add_missing_catalog(&self, rows: &[Value]) -> Result<usize> {
        let known: HashSet<String> = self
            .tables()
            .await?
            .into_iter()
            .find(|t| t.0 == "catalog_items")
            .map(|t| t.1.into_iter().collect())
            .unwrap_or_default();
        let mut n = 0;
        for r in rows {
            let Some(o) = r.as_object() else { continue };
            let keys: Vec<&String> = o.keys().filter(|k| known.contains(k.as_str())).collect();
            let sql = format!(
                "INSERT OR IGNORE INTO catalog_items ({}) VALUES ({})",
                keys.iter()
                    .map(|k| format!("\"{k}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
                vec!["?"; keys.len()].join(", ")
            );
            let mut q = sqlx::query(&sql);
            for k in &keys {
                q = match &o[k.as_str()] {
                    Value::Null => q.bind(None::<String>),
                    Value::Number(x) if x.is_i64() => q.bind(x.as_i64()),
                    Value::Number(x) => q.bind(x.as_f64()),
                    Value::String(s) => q.bind(s.clone()),
                    v => q.bind(v.to_string()),
                };
            }
            n += q.execute(&self.pool).await?.rows_affected() as usize;
        }
        Ok(n)
    }
}

/// Copie : tous les identifiants propres au projet sont renouvelés (ils apparaissent aussi
/// dans les adresses de ports et les données) ; le nom prend « (copie) ».
pub fn remap_copy(snap: &Value, name: &str) -> Value {
    let mut ids: HashSet<String> = HashSet::new();
    if let Some(p) = snap["project_id"].as_str() {
        ids.insert(p.to_string());
    }
    for rows in snap["tables"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        for r in rows.as_array().into_iter().flatten() {
            if let Some(id) = r.get("id").and_then(Value::as_str) {
                if id.len() == 36 {
                    ids.insert(id.to_string());
                }
            }
        }
    }
    let mut text = snap.to_string();
    for id in ids {
        text = text.replace(&id, &new_id());
    }
    let mut v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if let Some(p) = v["tables"]["projects"].get_mut(0) {
        p["name"] = json!(name);
        p["name_norm"] = json!(strata_domain::text::normalize(name));
        if let Some(mut d) = p["data"]
            .as_str()
            .and_then(|d| serde_json::from_str::<Value>(d).ok())
        {
            if d.get("name").is_some() {
                d["name"] = json!(name);
                p["data"] = json!(d.to_string());
            }
        }
    }
    // Liens vers d'autres projets : non repris dans une copie.
    if let Some(t) = v["tables"].as_object_mut() {
        t.remove("project_links");
    }
    v
}
