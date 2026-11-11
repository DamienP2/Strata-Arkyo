//! Bibliothèque de panels de brassage (section 6.3).

use crate::{Result, Store, StoreError, map_unique, new_id, now};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use strata_domain::panel::PanelData;
use strata_domain::text::normalize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Panel {
    pub id: String,
    pub panel: PanelData,
    pub rev: i64,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Panel {
    fn from_row(r: &SqliteRow) -> Panel {
        let mut panel: PanelData =
            serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default();
        panel.name = r.get("name");
        Panel {
            id: r.get("id"),
            panel,
            rev: r.get("rev"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }
}

impl Store {
    pub async fn panels(&self) -> Result<Vec<Panel>> {
        let rows = sqlx::query("SELECT * FROM panels ORDER BY name_norm")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(Panel::from_row).collect())
    }

    pub async fn panel(&self, id: &str) -> Result<Option<Panel>> {
        Ok(sqlx::query("SELECT * FROM panels WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Panel::from_row))
    }

    pub async fn insert_panel(&self, p: &PanelData, actor: Option<&str>) -> Result<Panel> {
        let id = new_id();
        let t = now();
        sqlx::query("INSERT INTO panels (id, name, name_norm, data, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&p.name)
            .bind(normalize(&p.name))
            .bind(serde_json::to_string(p).unwrap_or_default())
            .bind(actor)
            .bind(&t)
            .bind(&t)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?;
        self.panel(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_panel(&self, id: &str, rev: i64, p: &PanelData) -> Result<Panel> {
        let done = sqlx::query("UPDATE panels SET name = ?, name_norm = ?, data = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?")
            .bind(&p.name)
            .bind(normalize(&p.name))
            .bind(serde_json::to_string(p).unwrap_or_default())
            .bind(now())
            .bind(id)
            .bind(rev)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?;
        if done.rows_affected() == 0 {
            return Err(if self.panel(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        self.panel(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_panel(&self, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM panels WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn names_are_unique_and_revs_checked() {
        let s = Store::open_in_memory().await.unwrap();
        let p = PanelData {
            name: "Patch vidéo 16 BNC".into(),
            ..Default::default()
        };
        let a = s.insert_panel(&p, None).await.unwrap();
        assert!(matches!(
            s.insert_panel(
                &PanelData {
                    name: "PATCH VIDÉO 16 BNC".into(),
                    ..Default::default()
                },
                None
            )
            .await,
            Err(StoreError::Conflict(_))
        ));
        let mut q = a.panel.clone();
        q.height_u = 2;
        let b = s.update_panel(&a.id, a.rev, &q).await.unwrap();
        assert_eq!(b.panel.height_u, 2);
        assert!(matches!(
            s.update_panel(&a.id, a.rev, &q).await,
            Err(StoreError::Stale)
        ));
        s.delete_panel(&a.id).await.unwrap();
        assert!(s.panels().await.unwrap().is_empty());
    }
}
