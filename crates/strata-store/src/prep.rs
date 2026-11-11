//! Préparation et livraison (section 11.4).

use crate::{Result, Store, now};
use serde::Serialize;
use sqlx::Row;
use strata_domain::prep::{LineState, Supply};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PrepLine {
    pub key: String,
    pub status: Option<String>,
    pub supply: Option<Supply>,
    pub reserve: i64,
    pub note: Option<String>,
    pub updated_by: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CableDone {
    pub cable_id: String,
    pub done_at: String,
    pub done_by: Option<String>,
    pub note: Option<String>,
}

fn supply_str(s: Supply) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Store {
    pub async fn prep_lines(&self, project_id: &str) -> Result<Vec<PrepLine>> {
        let rows = sqlx::query("SELECT * FROM prep_lines WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| PrepLine {
                key: r.get("key"),
                status: r.get("status"),
                supply: r
                    .get::<Option<String>, _>("supply")
                    .and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok()),
                reserve: r.get("reserve"),
                note: r.get("note"),
                updated_by: r.get("updated_by"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    /// Enregistre la saisie d'une ligne ; ce qui n'est pas fourni est gardé.
    pub async fn put_prep_line(
        &self,
        project_id: &str,
        key: &str,
        s: &LineState,
        actor: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO prep_lines (project_id, key, status, supply, reserve, note, updated_by, updated_at) VALUES (?, ?, ?, ?, COALESCE(?, 0), ?, ?, ?) \
             ON CONFLICT (project_id, key) DO UPDATE SET status = COALESCE(excluded.status, status), supply = COALESCE(excluded.supply, supply), \
             reserve = COALESCE(?, reserve), note = COALESCE(excluded.note, note), updated_by = excluded.updated_by, updated_at = excluded.updated_at",
        )
        .bind(project_id)
        .bind(key)
        .bind(&s.status)
        .bind(s.supply.map(supply_str))
        .bind(s.reserve)
        .bind(&s.note)
        .bind(actor)
        .bind(now())
        .bind(s.reserve)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn cables_done(&self, project_id: &str) -> Result<Vec<CableDone>> {
        let rows = sqlx::query("SELECT * FROM cable_done WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| CableDone {
                cable_id: r.get("cable_id"),
                done_at: r.get("done_at"),
                done_by: r.get("done_by"),
                note: r.get("note"),
            })
            .collect())
    }

    /// Marque une liaison câblée (datée automatiquement) ou la remet à faire.
    pub async fn set_cable_done(
        &self,
        project_id: &str,
        cable_id: &str,
        done: bool,
        note: Option<&str>,
        actor: Option<&str>,
    ) -> Result<()> {
        if done {
            sqlx::query("INSERT INTO cable_done (cable_id, project_id, done_at, done_by, note) VALUES (?, ?, ?, ?, ?) ON CONFLICT (cable_id) DO UPDATE SET note = excluded.note")
                .bind(cable_id)
                .bind(project_id)
                .bind(now())
                .bind(actor)
                .bind(note)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query("DELETE FROM cable_done WHERE cable_id = ? AND project_id = ?")
                .bind(cable_id)
                .bind(project_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Affecte un modèle de câble à plusieurs câbles (une ligne de préparation entière).
    pub async fn set_cable_model(
        &self,
        project_id: &str,
        ids: &[String],
        model: Option<&str>,
    ) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        for id in ids {
            n += sqlx::query("UPDATE cables SET data = json_set(data, '$.cable_model_id', json(?)), rev = rev + 1, updated_at = ? WHERE id = ? AND project_id = ?")
                .bind(serde_json::to_string(&model).unwrap_or_else(|_| "null".into()))
                .bind(now())
                .bind(id)
                .bind(project_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        tx.commit().await?;
        Ok(n)
    }
}
