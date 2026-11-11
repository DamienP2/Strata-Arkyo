//! Régies (section 6.2) : ensembles ordonnés de contenants, dans un projet ou en bibliothèque.

use crate::{Result, Store, StoreError, new_id, now};
use serde::Serialize;
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use strata_domain::project::{Regie as RegieKind, RegieFields, main_regie_name};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Regie {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub kind: RegieKind,
    pub service_type: Option<String>,
    pub notes: Option<String>,
    pub main: bool,
    pub position: i64,
    pub rev: i64,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Contenants (régie de projet) ou modèles de contenants (bibliothèque), dans l'ordre.
    pub members: Vec<String>,
}

fn kind_str(k: RegieKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Regie {
    fn from_row(r: &SqliteRow) -> Regie {
        Regie {
            id: r.get("id"),
            project_id: r.get("project_id"),
            name: r.get("name"),
            kind: serde_json::from_value(Value::String(r.get("kind"))).unwrap_or_default(),
            service_type: r.get("service_type"),
            notes: r.get("notes"),
            main: r.get::<i64, _>("main") != 0,
            position: r.get("position"),
            rev: r.get("rev"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            members: Vec::new(),
        }
    }
}

async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: Option<&str>,
    f: &RegieFields,
    main: bool,
    actor: Option<&str>,
) -> Result<String> {
    let id = new_id();
    let t = now();
    let pos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM regies WHERE project_id IS ?",
    )
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query("INSERT INTO regies (id, project_id, name, kind, service_type, notes, main, position, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(project_id)
        .bind(&f.name)
        .bind(kind_str(f.kind))
        .bind(&f.service_type)
        .bind(&f.notes)
        .bind(main as i64)
        .bind(pos)
        .bind(actor)
        .bind(&t)
        .bind(&t)
        .execute(&mut **tx)
        .await?;
    Ok(id)
}

async fn put_members(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    library: bool,
    members: &[String],
) -> Result<()> {
    sqlx::query("DELETE FROM regie_members WHERE regie_id = ?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let sql = if library {
        "INSERT INTO regie_members (regie_id, position, template_id) VALUES (?, ?, ?)"
    } else {
        "INSERT INTO regie_members (regie_id, position, container_id) VALUES (?, ?, ?)"
    };
    for (i, m) in members.iter().enumerate() {
        sqlx::query(sql)
            .bind(id)
            .bind(i as i64 + 1)
            .bind(m)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Ajoute un nouveau contenant à la régie principale du projet, créée au besoin.
pub(crate) async fn attach_to_main(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    container_id: &str,
) -> Result<()> {
    let main: Option<String> =
        sqlx::query_scalar("SELECT id FROM regies WHERE project_id = ? AND main = 1")
            .bind(project_id)
            .fetch_optional(&mut **tx)
            .await?;
    let id = match main {
        Some(id) => id,
        None => {
            let (name, data): (String, String) =
                sqlx::query_as("SELECT name, data FROM projects WHERE id = ?")
                    .bind(project_id)
                    .fetch_one(&mut **tx)
                    .await?;
            let d: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
            let f = RegieFields {
                name: main_regie_name(&name),
                kind: d
                    .get("regie")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default(),
                service_type: d
                    .get("service_type")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                notes: None,
            };
            insert_tx(tx, Some(project_id), &f, true, None).await?
        }
    };
    sqlx::query("INSERT INTO regie_members (regie_id, position, container_id) SELECT ?, COALESCE(MAX(position), 0) + 1, ? FROM regie_members WHERE regie_id = ?")
        .bind(&id)
        .bind(container_id)
        .bind(&id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE regies SET rev = rev + 1, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(&id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl Store {
    async fn with_members(&self, mut list: Vec<Regie>) -> Result<Vec<Regie>> {
        for r in &mut list {
            r.members = sqlx::query_scalar(
                "SELECT COALESCE(container_id, template_id) FROM regie_members WHERE regie_id = ? ORDER BY position",
            )
            .bind(&r.id)
            .fetch_all(&self.pool)
            .await?;
        }
        Ok(list)
    }

    /// Régies d'un projet, ou de la bibliothèque si `project_id` est vide.
    pub async fn regies(&self, project_id: Option<&str>) -> Result<Vec<Regie>> {
        let rows = sqlx::query(
            "SELECT * FROM regies WHERE project_id IS ? ORDER BY main DESC, position, name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        self.with_members(rows.iter().map(Regie::from_row).collect())
            .await
    }

    pub async fn regie(&self, id: &str) -> Result<Option<Regie>> {
        let Some(r) = sqlx::query("SELECT * FROM regies WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        Ok(self.with_members(vec![Regie::from_row(&r)]).await?.pop())
    }

    pub async fn insert_regie(
        &self,
        project_id: Option<&str>,
        f: &RegieFields,
        members: &[String],
        actor: Option<&str>,
    ) -> Result<Regie> {
        let mut tx = self.pool.begin().await?;
        let id = insert_tx(&mut tx, project_id, f, false, actor).await?;
        put_members(&mut tx, &id, project_id.is_none(), members).await?;
        tx.commit().await?;
        self.regie(&id).await?.ok_or(StoreError::NotFound)
    }

    /// Modifie les champs et, si `members` est fourni, remplace la liste ordonnée.
    pub async fn update_regie(
        &self,
        id: &str,
        rev: i64,
        f: &RegieFields,
        members: Option<&[String]>,
    ) -> Result<Regie> {
        let mut tx = self.pool.begin().await?;
        let done = sqlx::query("UPDATE regies SET name = ?, kind = ?, service_type = ?, notes = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?")
            .bind(&f.name)
            .bind(kind_str(f.kind))
            .bind(&f.service_type)
            .bind(&f.notes)
            .bind(now())
            .bind(id)
            .bind(rev)
            .execute(&mut *tx)
            .await?;
        if done.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(if self.regie(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        if let Some(m) = members {
            let library: bool =
                sqlx::query_scalar("SELECT project_id IS NULL FROM regies WHERE id = ?")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            put_members(&mut tx, id, library, m).await?;
        }
        tx.commit().await?;
        self.regie(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_regie(&self, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM regies WHERE id = ?")
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
    use strata_domain::project::{ContainerData, ProjectData};

    #[tokio::test]
    async fn first_container_creates_the_main_regie() {
        let s = Store::open_in_memory().await.unwrap();
        let p = s
            .insert_project(
                &ProjectData {
                    name: "Finale".into(),
                    regie: RegieKind::Car,
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let l = s.insert_location(&p.id, "Car", "CAR").await.unwrap();
        assert!(s.regies(Some(&p.id)).await.unwrap().is_empty());
        let a = s
            .insert_container(&p.id, &l.id, "Baie 1", &ContainerData::default())
            .await
            .unwrap();
        let b = s
            .insert_container(&p.id, &l.id, "Baie 2", &ContainerData::default())
            .await
            .unwrap();
        let r = s.regies(Some(&p.id)).await.unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "Finale — Régie principale");
        assert_eq!(r[0].kind, RegieKind::Car);
        assert!(r[0].main);
        assert_eq!(r[0].members, vec![a.id.clone(), b.id.clone()]);

        // Une autre régie, dans l'ordre choisi ; supprimer un contenant le retire partout.
        let f = RegieFields {
            name: "Plateau".into(),
            ..Default::default()
        };
        let x = s
            .insert_regie(Some(&p.id), &f, &[b.id.clone(), a.id.clone()], None)
            .await
            .unwrap();
        assert_eq!(x.members, vec![b.id.clone(), a.id.clone()]);
        assert!(matches!(
            s.update_regie(&x.id, x.rev + 3, &f, None).await,
            Err(StoreError::Stale)
        ));
        s.delete_container(&a.id).await.unwrap();
        assert_eq!(s.regie(&x.id).await.unwrap().unwrap().members, vec![b.id]);
        assert!(s.regies(None).await.unwrap().is_empty());
    }
}
