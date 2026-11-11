//! Racks : placements, circuits et verrous d'édition.

use crate::{Result, Store, StoreError, new_id, now, timestamp};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use strata_domain::catalog::Width;
use strata_domain::rack::{Phase, RackFace};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Placement {
    pub id: String,
    pub project_id: String,
    pub container_id: String,
    pub article_id: String,
    pub instance_id: Option<String>,
    pub face: RackFace,
    pub u_start: i64,
    pub width: Width,
    pub col: i64,
    pub circuit_id: Option<String>,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Circuit {
    pub id: String,
    pub project_id: String,
    pub container_id: String,
    pub name: String,
    pub phase: Phase,
    pub rating_a: f64,
    pub position: i64,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lock {
    pub view: String,
    pub project_id: String,
    #[serde(skip)]
    pub token_hash: String,
    pub account_id: String,
    pub account_name: String,
    pub taken_at: String,
    pub beat_at: String,
}

fn de<T: serde::de::DeserializeOwned>(s: String) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s)).ok()
}
fn ser<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Placement {
    fn from_row(r: &SqliteRow) -> Placement {
        Placement {
            id: r.get("id"),
            project_id: r.get("project_id"),
            container_id: r.get("container_id"),
            article_id: r.get("article_id"),
            instance_id: r.get("instance_id"),
            face: de(r.get("face")).unwrap_or(RackFace::Front),
            u_start: r.get("u_start"),
            width: de(r.get("width")).unwrap_or(Width::Full),
            col: r.get("col"),
            circuit_id: r.get("circuit_id"),
            rev: r.get("rev"),
        }
    }
}
impl Circuit {
    fn from_row(r: &SqliteRow) -> Circuit {
        Circuit {
            id: r.get("id"),
            project_id: r.get("project_id"),
            container_id: r.get("container_id"),
            name: r.get("name"),
            phase: de(r.get("phase")).unwrap_or(Phase::L1),
            rating_a: r.get("rating_a"),
            position: r.get("position"),
            rev: r.get("rev"),
        }
    }
}
impl Lock {
    fn from_row(r: &SqliteRow) -> Lock {
        Lock {
            view: r.get("view"),
            project_id: r.get("project_id"),
            token_hash: r.get("token_hash"),
            account_id: r.get("account_id"),
            account_name: r.get("account_name"),
            taken_at: r.get("taken_at"),
            beat_at: r.get("beat_at"),
        }
    }
}

/// Position et réglages d'un élément posé.
#[derive(Debug, Clone)]
pub struct PlaceSpec {
    pub face: RackFace,
    pub u_start: i64,
    pub width: Width,
    pub col: i64,
    pub circuit_id: Option<String>,
}

pub enum LockOutcome {
    Taken(Lock),
    Held(Lock),
}

impl Store {
    // ----- Placements -----

    pub async fn placements(&self, container_id: &str) -> Result<Vec<Placement>> {
        let rows =
            sqlx::query("SELECT * FROM placements WHERE container_id = ? ORDER BY u_start, col")
                .bind(container_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.iter().map(Placement::from_row).collect())
    }

    pub async fn project_placements(&self, project_id: &str) -> Result<Vec<Placement>> {
        let rows = sqlx::query("SELECT * FROM placements WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(Placement::from_row).collect())
    }

    pub async fn placement(&self, id: &str) -> Result<Option<Placement>> {
        Ok(sqlx::query("SELECT * FROM placements WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Placement::from_row))
    }

    pub async fn insert_placement(
        &self,
        project_id: &str,
        container_id: &str,
        article_id: &str,
        instance_id: Option<&str>,
        p: &PlaceSpec,
    ) -> Result<Placement> {
        let id = new_id();
        let t = now();
        sqlx::query("INSERT INTO placements (id, project_id, container_id, article_id, instance_id, face, u_start, width, col, circuit_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(container_id)
            .bind(article_id)
            .bind(instance_id)
            .bind(ser(&p.face))
            .bind(p.u_start)
            .bind(ser(&p.width))
            .bind(p.col)
            .bind(&p.circuit_id)
            .bind(&t)
            .bind(&t)
            .execute(&self.pool)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db) if db.is_unique_violation() => StoreError::Conflict("placed"),
                _ => StoreError::Db(e),
            })?;
        self.placement(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_placement(&self, id: &str, rev: i64, p: &PlaceSpec) -> Result<Placement> {
        let done = sqlx::query("UPDATE placements SET face = ?, u_start = ?, width = ?, col = ?, circuit_id = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?")
            .bind(ser(&p.face))
            .bind(p.u_start)
            .bind(ser(&p.width))
            .bind(p.col)
            .bind(&p.circuit_id)
            .bind(now())
            .bind(id)
            .bind(rev)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(if self.placement(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        self.placement(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_placements(&self, ids: &[String]) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        for id in ids {
            n += sqlx::query("DELETE FROM placements WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        tx.commit().await?;
        Ok(n)
    }

    pub async fn clear_placements(&self, container_id: &str) -> Result<u64> {
        Ok(sqlx::query("DELETE FROM placements WHERE container_id = ?")
            .bind(container_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    // ----- Circuits -----

    pub async fn circuits(&self, container_id: &str) -> Result<Vec<Circuit>> {
        let rows = sqlx::query(
            "SELECT * FROM circuits WHERE container_id = ? ORDER BY position, created_at",
        )
        .bind(container_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(Circuit::from_row).collect())
    }

    pub async fn circuit(&self, id: &str) -> Result<Option<Circuit>> {
        Ok(sqlx::query("SELECT * FROM circuits WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Circuit::from_row))
    }

    pub async fn insert_circuit(
        &self,
        project_id: &str,
        container_id: &str,
        name: &str,
        phase: Phase,
        rating_a: f64,
    ) -> Result<Circuit> {
        let id = new_id();
        let t = now();
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM circuits WHERE container_id = ?",
        )
        .bind(container_id)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query("INSERT INTO circuits (id, project_id, container_id, name, phase, rating_a, position, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(container_id)
            .bind(name)
            .bind(ser(&phase))
            .bind(rating_a)
            .bind(pos)
            .bind(&t)
            .bind(&t)
            .execute(&self.pool)
            .await?;
        self.circuit(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_circuit(
        &self,
        id: &str,
        rev: i64,
        name: &str,
        phase: Phase,
        rating_a: f64,
    ) -> Result<Circuit> {
        let done = sqlx::query("UPDATE circuits SET name = ?, phase = ?, rating_a = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?")
            .bind(name)
            .bind(ser(&phase))
            .bind(rating_a)
            .bind(now())
            .bind(id)
            .bind(rev)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(if self.circuit(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        self.circuit(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_circuit(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM circuits WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ----- Verrous d'édition -----

    pub async fn lock(&self, view: &str) -> Result<Option<Lock>> {
        Ok(sqlx::query("SELECT * FROM locks WHERE view = ?")
            .bind(view)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Lock::from_row))
    }

    /// Prend la main si elle est libre, expirée (sans battement depuis `ttl_secs`) ou déjà à soi.
    pub async fn take_lock(
        &self,
        view: &str,
        project_id: &str,
        token_hash: &str,
        account_id: &str,
        account_name: &str,
        ttl_secs: i64,
    ) -> Result<LockOutcome> {
        let mut tx = self.pool.begin().await?;
        let cur = sqlx::query("SELECT * FROM locks WHERE view = ?")
            .bind(view)
            .fetch_optional(&mut *tx)
            .await?
            .as_ref()
            .map(Lock::from_row);
        let limit = timestamp(chrono::Utc::now() - chrono::Duration::seconds(ttl_secs));
        if let Some(l) = &cur {
            if l.token_hash != token_hash && l.beat_at >= limit {
                return Ok(LockOutcome::Held(l.clone()));
            }
        }
        let t = now();
        let taken_at = cur
            .as_ref()
            .filter(|l| l.token_hash == token_hash)
            .map(|l| l.taken_at.clone())
            .unwrap_or_else(|| t.clone());
        sqlx::query("INSERT OR REPLACE INTO locks (view, project_id, token_hash, account_id, account_name, taken_at, beat_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(view)
            .bind(project_id)
            .bind(token_hash)
            .bind(account_id)
            .bind(account_name)
            .bind(&taken_at)
            .bind(&t)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(LockOutcome::Taken(
            self.lock(view).await?.ok_or(StoreError::NotFound)?,
        ))
    }

    /// Libère la main si elle appartient à cette session. Renvoie vrai si elle a été libérée.
    pub async fn release_lock(&self, view: &str, token_hash: &str) -> Result<bool> {
        Ok(
            sqlx::query("DELETE FROM locks WHERE view = ? AND token_hash = ?")
                .bind(view)
                .bind(token_hash)
                .execute(&self.pool)
                .await?
                .rows_affected()
                > 0,
        )
    }

    /// La session tient-elle la main, avec un battement récent ?
    pub async fn holds_lock(&self, view: &str, token_hash: &str, ttl_secs: i64) -> Result<bool> {
        let limit = timestamp(chrono::Utc::now() - chrono::Duration::seconds(ttl_secs));
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM locks WHERE view = ? AND token_hash = ? AND beat_at >= ?",
        )
        .bind(view)
        .bind(token_hash)
        .bind(limit)
        .fetch_one(&self.pool)
        .await?;
        Ok(n > 0)
    }

    /// Libère les verrous d'une session (déconnexion).
    pub async fn release_session_locks(&self, token_hash: &str) -> Result<Vec<Lock>> {
        let rows = sqlx::query("DELETE FROM locks WHERE token_hash = ? RETURNING *")
            .bind(token_hash)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(Lock::from_row).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_domain::project::{ArticleNature, ContainerData, ProjectData};

    #[tokio::test]
    async fn locks_are_exclusive_until_they_expire() {
        let s = Store::open_in_memory().await.unwrap();
        let p = s
            .insert_project(
                &ProjectData {
                    name: "P".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        assert!(matches!(
            s.take_lock("rack:1", &p.id, "t1", "a1", "Léa", 45)
                .await
                .unwrap(),
            LockOutcome::Taken(_)
        ));
        assert!(
            matches!(s.take_lock("rack:1", &p.id, "t2", "a2", "Hugo", 45).await.unwrap(), LockOutcome::Held(l) if l.account_name == "Léa")
        );
        assert!(s.holds_lock("rack:1", "t1", 45).await.unwrap());
        // Sans battement : la main expire.
        sqlx::query("UPDATE locks SET beat_at = '2000-01-01T00:00:00Z'")
            .execute(&s.pool)
            .await
            .unwrap();
        assert!(!s.holds_lock("rack:1", "t1", 45).await.unwrap());
        assert!(matches!(
            s.take_lock("rack:1", &p.id, "t2", "a2", "Hugo", 45)
                .await
                .unwrap(),
            LockOutcome::Taken(_)
        ));
        assert!(!s.release_lock("rack:1", "t1").await.unwrap());
        assert!(s.release_lock("rack:1", "t2").await.unwrap());
    }

    #[tokio::test]
    async fn an_instance_is_placed_once_and_unplaced_ones_go_first() {
        let s = Store::open_in_memory().await.unwrap();
        let p = s
            .insert_project(
                &ProjectData {
                    name: "P".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let l = s.insert_location(&p.id, "Régie", "REG").await.unwrap();
        let c = s
            .insert_container(&p.id, &l.id, "Baie", &ContainerData::default())
            .await
            .unwrap();
        let (a, _) = s
            .add_article(
                &p.id,
                &l.id,
                Some(&c.id),
                ArticleNature::Equipment,
                Some("eq"),
                3,
                None,
                true,
            )
            .await
            .unwrap();
        let inst = s.article_instances(&a.id).await.unwrap();
        let spec = PlaceSpec {
            face: RackFace::Front,
            u_start: 1,
            width: Width::Full,
            col: 0,
            circuit_id: None,
        };
        // Le dernier exemplaire créé est posé : une baisse de quantité retire d'abord les autres.
        s.insert_placement(&p.id, &c.id, &a.id, Some(&inst[2].id), &spec)
            .await
            .unwrap();
        assert!(matches!(
            s.insert_placement(&p.id, &c.id, &a.id, Some(&inst[2].id), &spec)
                .await,
            Err(StoreError::Conflict("placed"))
        ));
        let a = s.set_article_quantity(&a.id, a.rev, 1).await.unwrap();
        let left = s.article_instances(&a.id).await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, inst[2].id, "l'exemplaire posé reste");
        assert_eq!(s.placements(&c.id).await.unwrap().len(), 1);
    }
}
