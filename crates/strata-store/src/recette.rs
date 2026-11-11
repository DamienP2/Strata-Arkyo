//! Recette (section 11.5).

use crate::{Result, Store, StoreError, new_id, now};
use serde::Serialize;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Point {
    pub key: String,
    pub kind: String,
    pub label: String,
    pub position: i64,
    pub status: String,
    pub comment: Option<String>,
    pub by_account: Option<String>,
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LogEntry {
    pub at: String,
    pub by_account: Option<String>,
    pub status: Option<String>,
    pub comment: Option<String>,
}

impl Store {
    pub async fn recette_points(&self, project_id: &str) -> Result<Vec<Point>> {
        let rows =
            sqlx::query("SELECT * FROM recette_points WHERE project_id = ? ORDER BY position, at")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .iter()
            .map(|r| Point {
                key: r.get("key"),
                kind: r.get("kind"),
                label: r.get("label"),
                position: r.get("position"),
                status: r.get("status"),
                comment: r.get("comment"),
                by_account: r.get("by_account"),
                at: r.get("at"),
            })
            .collect())
    }

    /// Pose un verdict (avec l'étiquette du moment) et l'inscrit au journal.
    pub async fn set_verdict(
        &self,
        project_id: &str,
        key: &str,
        kind: &str,
        label: &str,
        status: &str,
        comment: Option<&str>,
        actor: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let t = now();
        sqlx::query("INSERT INTO recette_points (project_id, key, kind, label, status, comment, by_account, at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
                     ON CONFLICT (project_id, key) DO UPDATE SET label = excluded.label, status = excluded.status, comment = excluded.comment, by_account = excluded.by_account, at = excluded.at")
            .bind(project_id).bind(key).bind(kind).bind(label).bind(status).bind(comment).bind(actor).bind(&t)
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO recette_log (id, project_id, key, at, by_account, status, comment) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(new_id()).bind(project_id).bind(key).bind(&t).bind(actor).bind(status).bind(comment)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn add_recette_comment(
        &self,
        project_id: &str,
        key: &str,
        comment: &str,
        actor: Option<&str>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO recette_log (id, project_id, key, at, by_account, comment) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(new_id()).bind(project_id).bind(key).bind(now()).bind(actor).bind(comment)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn recette_log(&self, project_id: &str, key: &str) -> Result<Vec<LogEntry>> {
        let rows = sqlx::query("SELECT at, by_account, status, comment FROM recette_log WHERE project_id = ? AND key = ? ORDER BY at DESC").bind(project_id).bind(key).fetch_all(&self.pool).await?;
        Ok(rows
            .iter()
            .map(|r| LogEntry {
                at: r.get("at"),
                by_account: r.get("by_account"),
                status: r.get("status"),
                comment: r.get("comment"),
            })
            .collect())
    }

    /// Ajoute un point libre, en fin de liste ; il naît « À tester ».
    pub async fn add_free_point(&self, project_id: &str, label: &str) -> Result<String> {
        let key = format!("free:{}", new_id());
        let pos: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM recette_points WHERE project_id = ? AND kind = 'free'").bind(project_id).fetch_one(&self.pool).await?;
        sqlx::query("INSERT INTO recette_points (project_id, key, kind, label, position) VALUES (?, ?, 'free', ?, ?)").bind(project_id).bind(&key).bind(label).bind(pos).execute(&self.pool).await?;
        Ok(key)
    }

    pub async fn rename_free_point(&self, project_id: &str, key: &str, label: &str) -> Result<()> {
        let n = sqlx::query("UPDATE recette_points SET label = ? WHERE project_id = ? AND key = ? AND kind = 'free'").bind(label).bind(project_id).bind(key).execute(&self.pool).await?.rows_affected();
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub async fn delete_free_point(&self, project_id: &str, key: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM recette_points WHERE project_id = ? AND key = ? AND kind = 'free'",
        )
        .bind(project_id)
        .bind(key)
        .execute(&self.pool)
        .await?;
        sqlx::query("DELETE FROM recette_log WHERE project_id = ? AND key = ?")
            .bind(project_id)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn order_free_points(&self, project_id: &str, keys: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (i, k) in keys.iter().enumerate() {
            sqlx::query("UPDATE recette_points SET position = ? WHERE project_id = ? AND key = ? AND kind = 'free'").bind(i as i64 + 1).bind(project_id).bind(k).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
