//! Contrôles de construction en atelier (section 9.7).

use crate::{Result, Store, now};
use serde::Serialize;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Check {
    pub key: String,
    pub checked_by: Option<String>,
    pub checked_at: String,
}

impl Store {
    pub async fn checks(&self, container_id: &str) -> Result<Vec<Check>> {
        let rows = sqlx::query(
            "SELECT key, checked_by, checked_at FROM build_checks WHERE container_id = ?",
        )
        .bind(container_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| Check {
                key: r.get("key"),
                checked_by: r.get("checked_by"),
                checked_at: r.get("checked_at"),
            })
            .collect())
    }

    /// Coche ou décoche plusieurs éléments d'un coup (glisser sur plusieurs lignes).
    /// Une coche existante garde son auteur et sa date.
    pub async fn set_checks(
        &self,
        container_id: &str,
        keys: &[String],
        on: bool,
        actor: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let t = now();
        for k in keys {
            if on {
                sqlx::query("INSERT OR IGNORE INTO build_checks (container_id, key, checked_by, checked_at) VALUES (?, ?, ?, ?)")
                    .bind(container_id)
                    .bind(k)
                    .bind(actor)
                    .bind(&t)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("DELETE FROM build_checks WHERE container_id = ? AND key = ?")
                    .bind(container_id)
                    .bind(k)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn clear_checks(&self, container_id: &str) -> Result<u64> {
        Ok(
            sqlx::query("DELETE FROM build_checks WHERE container_id = ?")
                .bind(container_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
}
