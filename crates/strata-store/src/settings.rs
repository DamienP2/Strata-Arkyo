//! Réglages mémorisés par projet.

use crate::{Result, Store, StoreError, now};
use serde_json::Value;

impl Store {
    /// Réglage d'un projet et sa révision (0 s'il n'a jamais été enregistré).
    pub async fn project_setting(&self, project_id: &str, key: &str) -> Result<(Value, i64)> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT data, rev FROM project_settings WHERE project_id = ? AND key = ?",
        )
        .bind(project_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row
            .map(|(d, r)| (serde_json::from_str(&d).unwrap_or(Value::Null), r))
            .unwrap_or((Value::Null, 0)))
    }

    /// Enregistre un réglage si `rev` est la révision lue ; renvoie la nouvelle révision.
    pub async fn put_project_setting(
        &self,
        project_id: &str,
        key: &str,
        rev: i64,
        data: &Value,
    ) -> Result<i64> {
        let done = if rev == 0 {
            sqlx::query("INSERT OR IGNORE INTO project_settings (project_id, key, data, rev, updated_at) VALUES (?, ?, ?, 1, ?)")
                .bind(project_id)
                .bind(key)
                .bind(data.to_string())
                .bind(now())
                .execute(&self.pool)
                .await?
        } else {
            sqlx::query("UPDATE project_settings SET data = ?, rev = rev + 1, updated_at = ? WHERE project_id = ? AND key = ? AND rev = ?")
                .bind(data.to_string())
                .bind(now())
                .bind(project_id)
                .bind(key)
                .bind(rev)
                .execute(&self.pool)
                .await?
        };
        if done.rows_affected() == 0 {
            return Err(StoreError::Stale);
        }
        Ok(rev + 1)
    }
}
