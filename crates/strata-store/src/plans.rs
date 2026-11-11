//! Cartographie (section 11.1).

use crate::{Result, Store, StoreError, new_id, now};
use serde::Serialize;
use sqlx::Row;
use strata_domain::plan::{FloorData, MapData, ObjectData, Waypoint};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanMap {
    pub id: String,
    #[serde(flatten)]
    pub data: MapData,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Floor {
    pub id: String,
    pub map_id: String,
    #[serde(flatten)]
    pub data: FloorData,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanObject {
    pub id: String,
    pub map_id: String,
    pub floor_id: String,
    #[serde(flatten)]
    pub data: ObjectData,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanLink {
    pub id: String,
    pub map_id: String,
    pub point_id: String,
    pub target_id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Route {
    pub key: String,
    pub map_id: String,
    pub waypoints: Vec<Waypoint>,
}

fn js<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn object(r: &sqlx::sqlite::SqliteRow) -> Option<PlanObject> {
    Some(PlanObject {
        id: r.get("id"),
        map_id: r.get("map_id"),
        floor_id: r.get("floor_id"),
        data: serde_json::from_str(r.get::<&str, _>("data")).ok()?,
    })
}

impl Store {
    pub async fn plan_maps(&self, project_id: &str) -> Result<Vec<PlanMap>> {
        let rows = sqlx::query(
            "SELECT * FROM plan_maps WHERE project_id = ? ORDER BY position, created_at",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| PlanMap {
                id: r.get("id"),
                data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default(),
                position: r.get("position"),
            })
            .collect())
    }

    /// Crée une carte et ses étages d'un coup (assistant).
    pub async fn insert_plan_map(
        &self,
        project_id: &str,
        map: &MapData,
        floors: &[FloorData],
    ) -> Result<String> {
        let mut tx = self.pool.begin().await?;
        let id = new_id();
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM plan_maps WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO plan_maps (id, project_id, data, position, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(js(map))
        .bind(pos)
        .bind(now())
        .execute(&mut *tx)
        .await?;
        for f in floors {
            sqlx::query("INSERT INTO plan_floors (id, map_id, level, data) VALUES (?, ?, ?, ?)")
                .bind(new_id())
                .bind(&id)
                .bind(f.level)
                .bind(js(f))
                .execute(&mut *tx)
                .await
                .map_err(|_| StoreError::Conflict("level"))?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn update_plan_map(&self, project_id: &str, id: &str, map: &MapData) -> Result<()> {
        let r = sqlx::query("UPDATE plan_maps SET data = ? WHERE id = ? AND project_id = ?")
            .bind(js(map))
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Supprime une carte ; ses fonds de plan partent avec elle.
    pub async fn delete_plan_map(&self, project_id: &str, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let files: Vec<String> = sqlx::query_scalar(
            "SELECT json_extract(data, '$.background') FROM plan_floors WHERE map_id = ? AND json_extract(data, '$.background') IS NOT NULL",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let r = sqlx::query("DELETE FROM plan_maps WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        if r.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        for f in files {
            sqlx::query("DELETE FROM plan_files WHERE id = ?")
                .bind(f)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn plan_floors(&self, map_id: &str) -> Result<Vec<Floor>> {
        let rows = sqlx::query("SELECT * FROM plan_floors WHERE map_id = ? ORDER BY level")
            .bind(map_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| Floor {
                id: r.get("id"),
                map_id: r.get("map_id"),
                data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default(),
            })
            .collect())
    }

    pub async fn insert_floor(&self, map_id: &str, f: &FloorData) -> Result<String> {
        let id = new_id();
        sqlx::query("INSERT INTO plan_floors (id, map_id, level, data) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(map_id)
            .bind(f.level)
            .bind(js(f))
            .execute(&self.pool)
            .await
            .map_err(|_| StoreError::Conflict("level"))?;
        Ok(id)
    }

    /// Le niveau d'un étage est figé : seul le reste change.
    pub async fn update_floor(&self, map_id: &str, id: &str, f: &FloorData) -> Result<()> {
        let r = sqlx::query("UPDATE plan_floors SET data = ? WHERE id = ? AND map_id = ?")
            .bind(js(f))
            .bind(id)
            .bind(map_id)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub async fn delete_floor(&self, map_id: &str, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let bg: Option<Option<String>> = sqlx::query_scalar(
            "SELECT json_extract(data, '$.background') FROM plan_floors WHERE id = ? AND map_id = ?",
        )
        .bind(id)
        .bind(map_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(bg) = bg else {
            return Err(StoreError::NotFound);
        };
        sqlx::query("DELETE FROM plan_floors WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if let Some(f) = bg {
            sqlx::query("DELETE FROM plan_files WHERE id = ?")
                .bind(f)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn insert_plan_file(
        &self,
        project_id: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<String> {
        let id = new_id();
        sqlx::query("INSERT INTO plan_files (id, project_id, content_type, bytes, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(content_type)
            .bind(bytes)
            .bind(now())
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn plan_file(&self, project_id: &str, id: &str) -> Result<Option<(String, Vec<u8>)>> {
        let r = sqlx::query(
            "SELECT content_type, bytes FROM plan_files WHERE id = ? AND project_id = ?",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(r.map(|r| (r.get("content_type"), r.get("bytes"))))
    }

    pub async fn delete_plan_file(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM plan_files WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Objets d'une carte, ou de tout le projet.
    pub async fn plan_objects(
        &self,
        project_id: &str,
        map_id: Option<&str>,
    ) -> Result<Vec<PlanObject>> {
        let rows = match map_id {
            Some(m) => {
                sqlx::query("SELECT * FROM plan_objects WHERE project_id = ? AND map_id = ?")
                    .bind(project_id)
                    .bind(m)
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                sqlx::query("SELECT * FROM plan_objects WHERE project_id = ?")
                    .bind(project_id)
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(rows.iter().filter_map(object).collect())
    }

    pub async fn insert_plan_object(
        &self,
        project_id: &str,
        map_id: &str,
        floor_id: &str,
        o: &ObjectData,
    ) -> Result<String> {
        let id = new_id();
        let cid = match o {
            ObjectData::Container { container_id, .. } => Some(container_id.clone()),
            _ => None,
        };
        sqlx::query("INSERT INTO plan_objects (id, project_id, map_id, floor_id, container_id, data) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(map_id)
            .bind(floor_id)
            .bind(cid)
            .bind(js(o))
            .execute(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(d) if d.message().contains("UNIQUE") => StoreError::Conflict("container"),
                _ => e.into(),
            })?;
        Ok(id)
    }

    /// Met à jour un objet (le contenant d'un placement ne change pas ; l'étage peut changer).
    pub async fn update_plan_object(
        &self,
        map_id: &str,
        id: &str,
        floor_id: &str,
        o: &ObjectData,
    ) -> Result<()> {
        let r = sqlx::query(
            "UPDATE plan_objects SET data = ?, floor_id = ? WHERE id = ? AND map_id = ?",
        )
        .bind(js(o))
        .bind(floor_id)
        .bind(id)
        .bind(map_id)
        .execute(&self.pool)
        .await?;
        if r.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub async fn delete_plan_objects(&self, map_id: &str, ids: &[String]) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        for id in ids {
            n += sqlx::query("DELETE FROM plan_objects WHERE id = ? AND map_id = ?")
                .bind(id)
                .bind(map_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        tx.commit().await?;
        Ok(n)
    }

    pub async fn plan_links(&self, map_id: &str) -> Result<Vec<PlanLink>> {
        let rows = sqlx::query("SELECT * FROM plan_links WHERE map_id = ?")
            .bind(map_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let d: serde_json::Value =
                    serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default();
                PlanLink {
                    id: r.get("id"),
                    map_id: r.get("map_id"),
                    point_id: r.get("point_id"),
                    target_id: r.get("target_id"),
                    label: d.get("label").and_then(|v| v.as_str()).map(str::to_string),
                }
            })
            .collect())
    }

    pub async fn insert_plan_link(
        &self,
        map_id: &str,
        point_id: &str,
        target_id: &str,
        label: Option<&str>,
    ) -> Result<String> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO plan_links (id, map_id, point_id, target_id, data) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(map_id)
        .bind(point_id)
        .bind(target_id)
        .bind(serde_json::json!({ "label": label }).to_string())
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn delete_plan_link(&self, map_id: &str, id: &str) -> Result<()> {
        let r = sqlx::query("DELETE FROM plan_links WHERE id = ? AND map_id = ?")
            .bind(id)
            .bind(map_id)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub async fn plan_routes(&self, project_id: &str) -> Result<Vec<Route>> {
        let rows = sqlx::query("SELECT * FROM plan_routes WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| Route {
                key: r.get("key"),
                map_id: r.get("map_id"),
                waypoints: serde_json::from_str(r.get::<&str, _>("waypoints")).unwrap_or_default(),
            })
            .collect())
    }

    pub async fn set_plan_route(
        &self,
        project_id: &str,
        key: &str,
        map_id: &str,
        w: &[Waypoint],
    ) -> Result<()> {
        sqlx::query("INSERT INTO plan_routes (project_id, key, map_id, waypoints) VALUES (?, ?, ?, ?) \
                     ON CONFLICT (project_id, key) DO UPDATE SET map_id = excluded.map_id, waypoints = excluded.waypoints")
            .bind(project_id)
            .bind(key)
            .bind(map_id)
            .bind(js(&w))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_plan_route(&self, project_id: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM plan_routes WHERE project_id = ? AND key = ?")
            .bind(project_id)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
