//! Dessins et plan de feu (section 11.2).

use crate::{Result, Store, StoreError, new_id, now};
use serde::Serialize;
use sqlx::Row;
use strata_domain::drawing::{Nature, PageData};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Page {
    pub id: String,
    pub nature: Nature,
    pub position: i64,
    pub rev: i64,
    pub updated_at: String,
    #[serde(flatten)]
    pub data: PageData,
}

fn nature_of(s: &str) -> Nature {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or_default()
}

fn nature_str(n: Nature) -> String {
    serde_json::to_value(n)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn page(r: &sqlx::sqlite::SqliteRow) -> Page {
    Page {
        id: r.get("id"),
        nature: nature_of(r.get::<&str, _>("nature")),
        position: r.get("position"),
        rev: r.get("rev"),
        updated_at: r.get("updated_at"),
        data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default(),
    }
}

impl Store {
    pub async fn drawings(&self, project_id: &str) -> Result<Vec<Page>> {
        let rows = sqlx::query(
            "SELECT * FROM drawings WHERE project_id = ? ORDER BY nature, position, updated_at",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(page).collect())
    }

    pub async fn drawing(&self, project_id: &str, id: &str) -> Result<Option<Page>> {
        let r = sqlx::query("SELECT * FROM drawings WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(r.as_ref().map(page))
    }

    pub async fn insert_drawing(
        &self,
        project_id: &str,
        nature: Nature,
        data: &PageData,
    ) -> Result<String> {
        let id = new_id();
        let n = nature_str(nature);
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM drawings WHERE project_id = ? AND nature = ?",
        )
        .bind(project_id)
        .bind(&n)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query("INSERT INTO drawings (id, project_id, nature, position, data, rev, updated_at) VALUES (?, ?, ?, ?, ?, 1, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(n)
            .bind(pos)
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(now())
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    /// Enregistre une page si la révision attendue est toujours la bonne.
    pub async fn save_drawing(
        &self,
        project_id: &str,
        id: &str,
        rev: i64,
        data: &PageData,
    ) -> Result<i64> {
        let r = sqlx::query("UPDATE drawings SET data = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND project_id = ? AND rev = ?")
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(now())
            .bind(id)
            .bind(project_id)
            .bind(rev)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return match self.drawing(project_id, id).await? {
                Some(_) => Err(StoreError::Stale),
                None => Err(StoreError::NotFound),
            };
        }
        Ok(rev + 1)
    }

    /// Enregistre sans contrôle de révision (synchronisation, adressage d'ensemble).
    pub async fn force_drawing(&self, project_id: &str, id: &str, data: &PageData) -> Result<i64> {
        let rev: Option<i64> = sqlx::query_scalar("UPDATE drawings SET data = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND project_id = ? RETURNING rev")
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(now())
            .bind(id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
        rev.ok_or(StoreError::NotFound)
    }

    pub async fn delete_drawing(&self, project_id: &str, id: &str) -> Result<()> {
        let r = sqlx::query("DELETE FROM drawings WHERE id = ? AND project_id = ?")
            .bind(id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Range les pages d'une nature dans l'ordre donné.
    pub async fn order_drawings(&self, project_id: &str, ids: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (i, id) in ids.iter().enumerate() {
            sqlx::query("UPDATE drawings SET position = ? WHERE id = ? AND project_id = ?")
                .bind(i as i64)
                .bind(id)
                .bind(project_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
