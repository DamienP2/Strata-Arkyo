//! Câbles du synoptique et disposition des nœuds (section 10).

use crate::{Result, Store, StoreError, new_id, now};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use std::collections::HashMap;
use strata_domain::synoptic::CableFields;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cable {
    pub id: String,
    pub project_id: String,
    pub a: String,
    pub b: String,
    #[serde(flatten)]
    pub fields: CableFields,
    pub rev: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Cable {
    fn from_row(r: &SqliteRow) -> Cable {
        Cable {
            id: r.get("id"),
            project_id: r.get("project_id"),
            a: r.get("a"),
            b: r.get("b"),
            fields: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default(),
            rev: r.get("rev"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }
}

/// Câble à poser : deux extrémités et ses champs.
#[derive(Debug, Clone)]
pub struct NewCable {
    pub a: String,
    pub b: String,
    pub fields: CableFields,
}

#[derive(Debug)]
pub enum PlaceCables {
    Done {
        created: Vec<Cable>,
        replaced: Vec<String>,
    },
    /// Ports déjà câblés : rien n'est posé tant que le remplacement n'est pas confirmé.
    Occupied(Vec<String>),
}

impl Store {
    pub async fn cables(&self, project_id: &str) -> Result<Vec<Cable>> {
        let rows = sqlx::query("SELECT * FROM cables WHERE project_id = ? ORDER BY created_at, id")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(Cable::from_row).collect())
    }

    pub async fn cable(&self, id: &str) -> Result<Option<Cable>> {
        Ok(sqlx::query("SELECT * FROM cables WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Cable::from_row))
    }

    /// Pose des câbles d'un coup. Un port occupé fait échouer le lot, sauf `replace` :
    /// le câble qui l'occupait est alors supprimé.
    pub async fn insert_cables(
        &self,
        project_id: &str,
        list: &[NewCable],
        replace: bool,
        actor: Option<&str>,
    ) -> Result<PlaceCables> {
        let mut tx = self.pool.begin().await?;
        let mut busy = Vec::new();
        let mut olds = Vec::new();
        for c in list {
            for p in [&c.a, &c.b] {
                let held: Option<String> = sqlx::query_scalar(
                    "SELECT cable_id FROM cable_ends WHERE project_id = ? AND port = ?",
                )
                .bind(project_id)
                .bind(p)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(h) = held {
                    busy.push(p.clone());
                    if !olds.contains(&h) {
                        olds.push(h);
                    }
                }
            }
        }
        if !busy.is_empty() && !replace {
            return Ok(PlaceCables::Occupied(busy));
        }
        for id in &olds {
            sqlx::query("DELETE FROM cables WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        let t = now();
        let mut ids = Vec::new();
        for c in list {
            let id = new_id();
            sqlx::query("INSERT INTO cables (id, project_id, a, b, data, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(project_id)
                .bind(&c.a)
                .bind(&c.b)
                .bind(serde_json::to_string(&c.fields).unwrap_or_default())
                .bind(actor)
                .bind(&t)
                .bind(&t)
                .execute(&mut *tx)
                .await?;
            for p in [&c.a, &c.b] {
                sqlx::query("INSERT INTO cable_ends (project_id, port, cable_id) VALUES (?, ?, ?)")
                    .bind(project_id)
                    .bind(p)
                    .bind(&id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| StoreError::Conflict("port"))?;
            }
            ids.push(id);
        }
        tx.commit().await?;
        let mut created = Vec::new();
        for id in ids {
            if let Some(c) = self.cable(&id).await? {
                created.push(c);
            }
        }
        Ok(PlaceCables::Done {
            created,
            replaced: olds,
        })
    }

    pub async fn update_cable(&self, id: &str, rev: i64, f: &CableFields) -> Result<Cable> {
        let done = sqlx::query(
            "UPDATE cables SET data = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?",
        )
        .bind(serde_json::to_string(f).unwrap_or_default())
        .bind(now())
        .bind(id)
        .bind(rev)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(if self.cable(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        self.cable(id).await?.ok_or(StoreError::NotFound)
    }

    /// Supprime des câbles d'un projet ; renvoie le nombre supprimé.
    pub async fn delete_cables(&self, project_id: &str, ids: &[String]) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        for id in ids {
            n += sqlx::query("DELETE FROM cables WHERE id = ? AND project_id = ?")
                .bind(id)
                .bind(project_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        tx.commit().await?;
        Ok(n)
    }

    /// Dernier numéro attribué par compteur (élément ou projet, préfixe, sens).
    pub async fn cable_counters(
        &self,
        project_id: &str,
    ) -> Result<HashMap<(String, String, i64), i64>> {
        let rows =
            sqlx::query("SELECT scope, prefix, dir, last FROM cable_counters WHERE project_id = ?")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    (r.get("scope"), r.get("prefix"), r.get("dir")),
                    r.get("last"),
                )
            })
            .collect())
    }

    /// Écrit des numéros (ou les efface) et avance les compteurs, d'un seul coup.
    pub async fn apply_numbers(
        &self,
        project_id: &str,
        numbers: &[(String, Option<String>)],
        counters: &[((String, String, i64), i64)],
    ) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let t = now();
        let mut n = 0;
        for (id, num) in numbers {
            n += sqlx::query("UPDATE cables SET data = json_set(data, '$.number', json(?)), rev = rev + 1, updated_at = ? WHERE id = ? AND project_id = ?")
                .bind(serde_json::to_string(num).unwrap_or_else(|_| "null".into()))
                .bind(&t)
                .bind(id)
                .bind(project_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        for ((scope, prefix, dir), last) in counters {
            sqlx::query("INSERT INTO cable_counters (project_id, scope, prefix, dir, last) VALUES (?, ?, ?, ?, ?) ON CONFLICT (project_id, scope, prefix, dir) DO UPDATE SET last = MAX(last, excluded.last)")
                .bind(project_id)
                .bind(scope)
                .bind(prefix)
                .bind(dir)
                .bind(last)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(n)
    }

    pub async fn layout(
        &self,
        project_id: &str,
        view: &str,
    ) -> Result<HashMap<String, (f64, f64)>> {
        let rows =
            sqlx::query("SELECT node, x, y FROM syn_layout WHERE project_id = ? AND view = ?")
                .bind(project_id)
                .bind(view)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("node"), (r.get("x"), r.get("y"))))
            .collect())
    }

    pub async fn put_layout(
        &self,
        project_id: &str,
        view: &str,
        positions: &[(String, f64, f64)],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (node, x, y) in positions {
            sqlx::query("INSERT INTO syn_layout (project_id, view, node, x, y) VALUES (?, ?, ?, ?, ?) ON CONFLICT (project_id, view, node) DO UPDATE SET x = excluded.x, y = excluded.y")
                .bind(project_id)
                .bind(view)
                .bind(node)
                .bind(x)
                .bind(y)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_domain::project::ProjectData;

    #[tokio::test]
    async fn one_cable_per_port() {
        let s = Store::open_in_memory().await.unwrap();
        let p = s
            .insert_project(
                &ProjectData {
                    name: "P".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let c = |a: &str, b: &str| NewCable {
            a: a.into(),
            b: b.into(),
            fields: Default::default(),
        };
        let PlaceCables::Done { created, .. } = s
            .insert_cables(&p.id, &[c("i:x:0:1", "i:y:0:1")], false, None)
            .await
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(created.len(), 1);
        let r = s
            .insert_cables(&p.id, &[c("i:x:0:1", "i:z:0:1")], false, None)
            .await
            .unwrap();
        assert!(matches!(r, PlaceCables::Occupied(ref v) if v == &vec!["i:x:0:1".to_string()]));
        let PlaceCables::Done { replaced, .. } = s
            .insert_cables(&p.id, &[c("i:x:0:1", "i:z:0:1")], true, None)
            .await
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(replaced, vec![created[0].id.clone()]);
        let all = s.cables(&p.id).await.unwrap();
        assert_eq!(all.len(), 1);
        // Le port libéré par le remplacement est de nouveau libre.
        assert!(matches!(
            s.insert_cables(&p.id, &[c("i:y:0:1", "i:w:0:1")], false, None)
                .await
                .unwrap(),
            PlaceCables::Done { .. }
        ));
        s.put_layout(&p.id, "container:c", &[("i:x".into(), 10.0, 20.0)])
            .await
            .unwrap();
        s.put_layout(&p.id, "container:c", &[("i:x".into(), 30.0, 20.0)])
            .await
            .unwrap();
        assert_eq!(
            s.layout(&p.id, "container:c").await.unwrap()["i:x"],
            (30.0, 20.0)
        );
        assert_eq!(
            s.delete_cables(&p.id, &[all[0].id.clone()]).await.unwrap(),
            1
        );
    }
}
