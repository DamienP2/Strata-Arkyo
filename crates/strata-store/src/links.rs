//! Liens entre projets.

use crate::{Result, Store, StoreError, new_id, now};
use serde::Serialize;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProjectLink {
    pub id: String,
    pub a_project: String,
    pub a_cable: String,
    pub b_project: String,
    pub b_cable: String,
    pub note: Option<String>,
    pub created_at: String,
}

fn link(r: &sqlx::sqlite::SqliteRow) -> ProjectLink {
    ProjectLink {
        id: r.get("id"),
        a_project: r.get("a_project"),
        a_cable: r.get("a_cable"),
        b_project: r.get("b_project"),
        b_cable: r.get("b_cable"),
        note: r.get("note"),
        created_at: r.get("created_at"),
    }
}

impl Store {
    /// Liens où le projet figure, d'un côté ou de l'autre.
    pub async fn project_links(&self, project_id: &str) -> Result<Vec<ProjectLink>> {
        let rows = sqlx::query(
            "SELECT * FROM project_links WHERE a_project = ? OR b_project = ? ORDER BY created_at",
        )
        .bind(project_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(link).collect())
    }

    /// Câbles déjà liés, toutes parties confondues.
    pub async fn linked_cables(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT a_cable AS c FROM project_links UNION SELECT b_cable FROM project_links",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get("c")).collect())
    }

    pub async fn insert_project_link(
        &self,
        a: (&str, &str),
        b: (&str, &str),
        note: Option<&str>,
        by: Option<&str>,
    ) -> Result<String> {
        let id = new_id();
        sqlx::query("INSERT INTO project_links (id, a_project, a_cable, b_project, b_cable, note, created_at, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(a.0)
            .bind(a.1)
            .bind(b.0)
            .bind(b.1)
            .bind(note)
            .bind(now())
            .bind(by)
            .execute(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(d) if d.message().contains("UNIQUE") => StoreError::Conflict("cable"),
                _ => e.into(),
            })?;
        Ok(id)
    }

    pub async fn delete_project_link(&self, id: &str) -> Result<Option<ProjectLink>> {
        let r = sqlx::query("DELETE FROM project_links WHERE id = ? RETURNING *")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(r.as_ref().map(link))
    }
}
