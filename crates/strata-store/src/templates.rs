//! Bibliothèque de modèles de contenants (section 6.1).

use crate::{Result, Store, StoreError, map_unique, new_id, now};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use strata_domain::text::normalize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateStatus {
    #[default]
    Draft,
    Inactive,
    Active,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ContainerTemplate {
    pub id: String,
    pub name: String,
    pub status: TemplateStatus,
    pub cable_digit: Option<i64>,
    /// Contenant et contenu : voir `strata_api::templates`.
    pub data: Value,
    pub rev: i64,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ContainerTemplate {
    fn from_row(r: &SqliteRow) -> ContainerTemplate {
        ContainerTemplate {
            id: r.get("id"),
            name: r.get("name"),
            status: serde_json::from_value(Value::String(r.get("status"))).unwrap_or_default(),
            cable_digit: r.get("cable_digit"),
            data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or(Value::Null),
            rev: r.get("rev"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }
}

fn status_str(s: TemplateStatus) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Store {
    pub async fn templates(&self) -> Result<Vec<ContainerTemplate>> {
        let rows = sqlx::query("SELECT * FROM container_templates ORDER BY name_norm")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(ContainerTemplate::from_row).collect())
    }

    pub async fn template(&self, id: &str) -> Result<Option<ContainerTemplate>> {
        Ok(
            sqlx::query("SELECT * FROM container_templates WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .as_ref()
                .map(ContainerTemplate::from_row),
        )
    }

    /// Crée un modèle. Son identifiant câble est le premier chiffre libre de 1 à 9, puis 0.
    pub async fn insert_template(
        &self,
        name: &str,
        data: &Value,
        actor: Option<&str>,
    ) -> Result<ContainerTemplate> {
        let mut tx = self.pool.begin().await?;
        let used: Vec<i64> = sqlx::query_scalar(
            "SELECT cable_digit FROM container_templates WHERE cable_digit IS NOT NULL",
        )
        .fetch_all(&mut *tx)
        .await?;
        let digit = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0]
            .into_iter()
            .find(|d| !used.contains(d));
        let id = new_id();
        let t = now();
        sqlx::query("INSERT INTO container_templates (id, name, name_norm, cable_digit, data, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(normalize(name))
            .bind(digit)
            .bind(data.to_string())
            .bind(actor)
            .bind(&t)
            .bind(&t)
            .execute(&mut *tx)
            .await
            .map_err(map_unique)?;
        tx.commit().await?;
        self.template(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_template(
        &self,
        id: &str,
        rev: i64,
        name: &str,
        status: TemplateStatus,
        data: &Value,
    ) -> Result<ContainerTemplate> {
        let done = sqlx::query("UPDATE container_templates SET name = ?, name_norm = ?, status = ?, data = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?")
            .bind(name)
            .bind(normalize(name))
            .bind(status_str(status))
            .bind(data.to_string())
            .bind(now())
            .bind(id)
            .bind(rev)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?;
        if done.rows_affected() == 0 {
            return Err(if self.template(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        self.template(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_template(&self, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM container_templates WHERE id = ?")
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
    async fn digits_are_unique_and_names_too() {
        let s = Store::open_in_memory().await.unwrap();
        let a = s
            .insert_template("Baie vidéo", &serde_json::json!({}), None)
            .await
            .unwrap();
        let b = s
            .insert_template("Flight HF", &serde_json::json!({}), None)
            .await
            .unwrap();
        assert_eq!((a.cable_digit, b.cable_digit), (Some(1), Some(2)));
        assert!(matches!(
            s.insert_template("BAIE VIDÉO", &serde_json::json!({}), None)
                .await,
            Err(StoreError::Conflict(_))
        ));
        s.delete_template(&a.id).await.unwrap();
        assert_eq!(
            s.insert_template("Autre", &serde_json::json!({}), None)
                .await
                .unwrap()
                .cable_digit,
            Some(1),
            "premier chiffre libre"
        );
        let u = s
            .update_template(
                &b.id,
                b.rev,
                "Flight HF",
                TemplateStatus::Active,
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(u.status, TemplateStatus::Active);
    }
}
