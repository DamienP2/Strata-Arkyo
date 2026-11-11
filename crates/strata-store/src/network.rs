//! Réseau (section 11.3).

use crate::{Result, Store, StoreError, new_id};
use serde::Serialize;
use sqlx::Row;
use strata_domain::network::{NetPort, VlanData};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Vlan {
    pub id: String,
    #[serde(flatten)]
    pub data: VlanData,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Flow {
    pub key: String,
    pub address: String,
    pub udp: i64,
    pub manual: bool,
}

fn taken(e: sqlx::Error) -> StoreError {
    match &e {
        sqlx::Error::Database(d) if d.message().contains("UNIQUE") => StoreError::Conflict("vid"),
        _ => e.into(),
    }
}

impl Store {
    pub async fn vlans(&self, project_id: &str) -> Result<Vec<Vlan>> {
        let rows = sqlx::query("SELECT id, data FROM net_vlans WHERE project_id = ? ORDER BY vid")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| Vlan {
                id: r.get("id"),
                data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default(),
            })
            .collect())
    }

    pub async fn insert_vlan(&self, project_id: &str, v: &VlanData) -> Result<String> {
        let id = new_id();
        sqlx::query("INSERT INTO net_vlans (id, project_id, vid, data) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(v.vid as i64)
            .bind(serde_json::to_string(v).unwrap_or_default())
            .execute(&self.pool)
            .await
            .map_err(taken)?;
        Ok(id)
    }

    /// Met à jour un VLAN ; si son numéro change, les ports qui l'utilisent suivent.
    pub async fn update_vlan(&self, project_id: &str, id: &str, v: &VlanData) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let old: Option<i64> =
            sqlx::query_scalar("SELECT vid FROM net_vlans WHERE id = ? AND project_id = ?")
                .bind(id)
                .bind(project_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(old) = old else {
            return Err(StoreError::NotFound);
        };
        sqlx::query("UPDATE net_vlans SET vid = ?, data = ? WHERE id = ?")
            .bind(v.vid as i64)
            .bind(serde_json::to_string(v).unwrap_or_default())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(taken)?;
        if old != v.vid as i64 {
            let rows = sqlx::query("SELECT port_ref, data FROM net_ports WHERE project_id = ?")
                .bind(project_id)
                .fetch_all(&mut *tx)
                .await?;
            for r in rows {
                let mut p: NetPort =
                    serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default();
                let mut changed = false;
                if p.vlan == Some(old as u16) {
                    p.vlan = Some(v.vid);
                    changed = true;
                }
                for x in p.vlans.iter_mut().filter(|x| **x == old as u16) {
                    *x = v.vid;
                    changed = true;
                }
                if changed {
                    p.vlans.sort_unstable();
                    sqlx::query(
                        "UPDATE net_ports SET data = ? WHERE project_id = ? AND port_ref = ?",
                    )
                    .bind(serde_json::to_string(&p).unwrap_or_default())
                    .bind(project_id)
                    .bind(r.get::<&str, _>("port_ref"))
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Supprime un VLAN et le retire des ports.
    pub async fn delete_vlan(&self, project_id: &str, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let vid: Option<i64> = sqlx::query_scalar(
            "DELETE FROM net_vlans WHERE id = ? AND project_id = ? RETURNING vid",
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(vid) = vid else {
            return Err(StoreError::NotFound);
        };
        let rows = sqlx::query("SELECT port_ref, data FROM net_ports WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(&mut *tx)
            .await?;
        for r in rows {
            let mut p: NetPort = serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default();
            let before = p.clone();
            if p.vlan == Some(vid as u16) {
                p.vlan = None;
            }
            p.vlans.retain(|x| *x as i64 != vid);
            if p != before {
                sqlx::query("UPDATE net_ports SET data = ? WHERE project_id = ? AND port_ref = ?")
                    .bind(serde_json::to_string(&p).unwrap_or_default())
                    .bind(project_id)
                    .bind(r.get::<&str, _>("port_ref"))
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn net_ports(&self, project_id: &str) -> Result<Vec<(String, NetPort)>> {
        let rows = sqlx::query("SELECT port_ref, data FROM net_ports WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get("port_ref"),
                    serde_json::from_str(r.get::<&str, _>("data")).unwrap_or_default(),
                )
            })
            .collect())
    }

    /// Enregistre des réglages de ports en une fois (un réglage vide efface la ligne).
    pub async fn set_net_ports(&self, project_id: &str, ports: &[(String, NetPort)]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (r, p) in ports {
            if p.is_empty() {
                sqlx::query("DELETE FROM net_ports WHERE project_id = ? AND port_ref = ?")
                    .bind(project_id)
                    .bind(r)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query(
                    "INSERT INTO net_ports (project_id, port_ref, data) VALUES (?, ?, ?) \
                             ON CONFLICT (project_id, port_ref) DO UPDATE SET data = excluded.data",
                )
                .bind(project_id)
                .bind(r)
                .bind(serde_json::to_string(p).unwrap_or_default())
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn net_flows(&self, project_id: &str) -> Result<Vec<Flow>> {
        let rows = sqlx::query("SELECT * FROM net_flows WHERE project_id = ? ORDER BY key")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| Flow {
                key: r.get("key"),
                address: r.get("address"),
                udp: r.get("udp"),
                manual: r.get::<i64, _>("manual") != 0,
            })
            .collect())
    }

    pub async fn set_net_flows(&self, project_id: &str, flows: &[Flow]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for f in flows {
            sqlx::query("INSERT INTO net_flows (project_id, key, address, udp, manual) VALUES (?, ?, ?, ?, ?) \
                         ON CONFLICT (project_id, key) DO UPDATE SET address = excluded.address, udp = excluded.udp, manual = excluded.manual")
                .bind(project_id)
                .bind(&f.key)
                .bind(&f.address)
                .bind(f.udp)
                .bind(f.manual as i64)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn clear_net_flows(&self, project_id: &str, keys: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for k in keys {
            sqlx::query("DELETE FROM net_flows WHERE project_id = ? AND key = ?")
                .bind(project_id)
                .bind(k)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
