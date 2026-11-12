//! Temps réel : diffusion des modifications et présence, par WebSocket (`/api/live`).
//!
//! Les écritures passent toujours par l'API REST ; le WebSocket ne fait que diffuser.
//! Un client rejoint des salles (« project:<id> », « catalog ») ; il reçoit les
//! événements de ces salles et la liste des personnes présentes. Chaque requête
//! d'écriture peut porter `X-Strata-Client` : l'événement le reprend, et l'auteur
//! sait ainsi ignorer ses propres modifications.

use crate::AppState;
use crate::auth::Member;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::request::Parts;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use strata_domain::{Domain, Level};
use tokio::sync::broadcast;

/// Une présence expire sans battement pendant ce délai.
pub const PRESENCE_TTL: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Un objet a changé : `kind` (project, location, container, article, instance, task,
    /// catalog_item, ref), `action` (create, update, delete, move), version après écriture.
    Change {
        room: String,
        kind: String,
        id: String,
        action: String,
        rev: Option<i64>,
        actor: String,
        client: Option<String>,
    },
    /// Personnes présentes dans une salle.
    Presence {
        room: String,
        members: Vec<PresenceMember>,
    },
    /// Curseur d'une personne sur une vue graphique (coordonnées de la vue).
    Cursor {
        room: String,
        #[serde(skip)]
        conn: u64,
        member: PresenceMember,
        x: f64,
        y: f64,
    },
    /// Sélection d'une personne sur une vue graphique.
    Select {
        room: String,
        #[serde(skip)]
        conn: u64,
        member: PresenceMember,
        ids: Vec<String>,
    },
}

/// Couleurs d'utilisateur : stables, dérivées de l'identifiant.
pub const USER_COLORS: [&str; 10] = [
    "#2459E0", "#0E8550", "#B78400", "#7A44C9", "#CF3A2F", "#0A8797", "#BD3489", "#D06006",
    "#4F6BFF", "#5B8C2A",
];

pub fn user_color(id: &str) -> &'static str {
    // FNV-1a : même identifiant, même couleur, sur tous les postes.
    let mut h: u32 = 0x811c9dc5;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    USER_COLORS[(h % 10) as usize]
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PresenceMember {
    pub id: String,
    pub name: String,
    pub color: String,
}

struct Seen {
    member: PresenceMember,
    at: Instant,
}

pub struct Hub {
    tx: broadcast::Sender<Event>,
    /// Salle → connexion → présence.
    rooms: Mutex<HashMap<String, HashMap<u64, Seen>>>,
    next: std::sync::atomic::AtomicU64,
}

impl Default for Hub {
    fn default() -> Self {
        Hub {
            tx: broadcast::channel(1024).0,
            rooms: Mutex::new(HashMap::new()),
            next: Default::default(),
        }
    }
}

impl Hub {
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Publie un changement. Personne n'écoute ? Rien de grave.
    #[allow(clippy::too_many_arguments)]
    pub fn change(
        &self,
        room: impl Into<String>,
        kind: &str,
        id: &str,
        action: &str,
        rev: Option<i64>,
        actor: &str,
        client: Option<&str>,
    ) {
        let _ = self.tx.send(Event::Change {
            room: room.into(),
            kind: kind.into(),
            id: id.into(),
            action: action.into(),
            rev,
            actor: actor.into(),
            client: client.map(str::to_string),
        });
    }

    fn members(&self, room: &str) -> Vec<PresenceMember> {
        let rooms = self.rooms.lock().unwrap();
        let mut seen = HashSet::new();
        let mut out: Vec<PresenceMember> = rooms
            .get(room)
            .map(|m| {
                m.values()
                    .filter(|s| seen.insert(s.member.id.clone()))
                    .map(|s| s.member.clone())
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn announce(&self, room: &str) {
        let _ = self.tx.send(Event::Presence {
            room: room.into(),
            members: self.members(room),
        });
    }

    fn join(&self, room: &str, conn: u64, member: PresenceMember) {
        self.rooms
            .lock()
            .unwrap()
            .entry(room.into())
            .or_default()
            .insert(
                conn,
                Seen {
                    member,
                    at: Instant::now(),
                },
            );
        self.announce(room);
    }

    fn leave(&self, room: &str, conn: u64) {
        let changed = {
            let mut rooms = self.rooms.lock().unwrap();
            let changed = rooms
                .get_mut(room)
                .is_some_and(|m| m.remove(&conn).is_some());
            rooms.retain(|_, m| !m.is_empty());
            changed
        };
        if changed {
            self.announce(room);
        }
    }

    fn beat(&self, conn: u64) {
        for m in self.rooms.lock().unwrap().values_mut() {
            if let Some(s) = m.get_mut(&conn) {
                s.at = Instant::now();
            }
        }
    }

    /// Retire les présences sans battement récent ; à appeler périodiquement.
    pub fn sweep(&self) {
        let stale: Vec<(String, u64)> = {
            let rooms = self.rooms.lock().unwrap();
            rooms
                .iter()
                .flat_map(|(r, m)| {
                    m.iter()
                        .filter(|(_, s)| s.at.elapsed() > PRESENCE_TTL)
                        .map(move |(c, _)| (r.clone(), *c))
                })
                .collect()
        };
        for (room, conn) in stale {
            self.leave(&room, conn);
        }
    }

    /// Personnes présentes dans une salle (pour l'API REST).
    pub fn present(&self, room: &str) -> Vec<PresenceMember> {
        self.members(room)
    }
}

/// Identifiant de l'onglet qui écrit (en-tête `X-Strata-Client`), repris dans les événements.
pub struct Origin(pub Option<String>);

impl axum::extract::FromRequestParts<AppState> for Origin {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        Ok(Origin(
            parts
                .headers
                .get("x-strata-client")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.chars().take(64).collect()),
        ))
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    Join { room: String },
    Leave { room: String },
    Ping,
    Cursor { room: String, x: f64, y: f64 },
    Select { room: String, ids: Vec<String> },
}

/// Taille maximale d'un message reçu.
const MAX_MSG: usize = 4096;

pub async fn socket(
    State(st): State<AppState>,
    Member(s): Member,
    ws: WebSocketUpgrade,
) -> Response {
    let member = PresenceMember {
        id: s.account.id.clone(),
        name: s.account.name.clone(),
        color: user_color(&s.account.id).to_string(),
    };
    let can_projects = s.actor.allows(Domain::Projets, Level::Lecture);
    let can_catalog = s.actor.allows(Domain::Catalogue, Level::Lecture);
    ws.on_upgrade(move |socket| run(st, socket, member, can_projects, can_catalog))
}

async fn run(
    st: AppState,
    mut socket: WebSocket,
    member: PresenceMember,
    can_projects: bool,
    can_catalog: bool,
) {
    let hub = st.hub.clone();
    let conn = hub.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut rx = hub.subscribe();
    let mut joined: HashSet<String> = HashSet::new();
    let mut last_cursor = Instant::now() - Duration::from_secs(1);
    loop {
        tokio::select! {
            msg = socket.recv() => {
                let Some(Ok(msg)) = msg else { break };
                let Message::Text(text) = msg else { continue };
                if text.len() > MAX_MSG { continue; }
                match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(ClientMsg::Join { room }) => {
                        let allowed = (room.starts_with("project:") && can_projects) || (room == "catalog" && can_catalog) || room == "site";
                        if allowed && joined.len() < 32 && joined.insert(room.clone()) {
                            hub.join(&room, conn, member.clone());
                        }
                    }
                    Ok(ClientMsg::Leave { room }) => {
                        if joined.remove(&room) {
                            hub.leave(&room, conn);
                        }
                    }
                    Ok(ClientMsg::Cursor { room, x, y }) => {
                        // Environ 30 par seconde au plus ; au-delà, on laisse tomber.
                        if joined.contains(&room) && x.is_finite() && y.is_finite() && last_cursor.elapsed() >= Duration::from_millis(25) {
                            last_cursor = Instant::now();
                            let _ = hub.tx.send(Event::Cursor { room, conn, member: member.clone(), x, y });
                        }
                    }
                    Ok(ClientMsg::Select { room, ids }) => {
                        if joined.contains(&room) && ids.len() <= 200 {
                            let _ = hub.tx.send(Event::Select { room, conn, member: member.clone(), ids });
                        }
                    }
                    Ok(ClientMsg::Ping) => {
                        hub.beat(conn);
                        if socket.send(Message::Text(r#"{"type":"pong"}"#.into())).await.is_err() { break; }
                    }
                    Err(_) => {}
                }
            }
            ev = rx.recv() => {
                let ev = match ev {
                    Ok(ev) => ev,
                    // Trop d'événements manqués : le client recharge tout.
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                        continue;
                    }
                    Err(_) => break,
                };
                let room = match &ev {
                    Event::Change { room, .. } | Event::Presence { room, .. } => room,
                    // Ses propres curseur et sélection ne reviennent pas à l'onglet qui les envoie.
                    Event::Cursor { room, conn: c, .. } | Event::Select { room, conn: c, .. } => { if *c == conn { continue; } room }
                };
                if !joined.contains(room) { continue; }
                let Ok(text) = serde_json::to_string(&ev) else { continue };
                if socket.send(Message::Text(text.into())).await.is_err() { break; }
            }
        }
    }
    for room in joined {
        hub.leave(&room, conn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_is_per_person_and_expires() {
        let hub = Hub::default();
        let mut rx = hub.subscribe();
        let lea = PresenceMember {
            id: "lea".into(),
            name: "Léa".into(),
            color: "#000".into(),
        };
        hub.join("project:1", 1, lea.clone());
        hub.join("project:1", 2, lea.clone()); // deux onglets
        assert_eq!(hub.present("project:1").len(), 1);
        hub.leave("project:1", 1);
        assert_eq!(hub.present("project:1").len(), 1);
        // Vieillir la présence restante.
        hub.rooms
            .lock()
            .unwrap()
            .get_mut("project:1")
            .unwrap()
            .get_mut(&2)
            .unwrap()
            .at = Instant::now() - PRESENCE_TTL - Duration::from_secs(1);
        hub.sweep();
        assert!(hub.present("project:1").is_empty());
        let mut last = None;
        while let Ok(e) = rx.try_recv() {
            last = Some(e);
        }
        assert!(matches!(last, Some(Event::Presence { members, .. }) if members.is_empty()));
    }

    #[test]
    fn user_color_is_stable_and_from_palette() {
        let a = user_color("01a1cbc2-93ac-761a-bdc2-4d4b69ab4cd2");
        assert_eq!(a, user_color("01a1cbc2-93ac-761a-bdc2-4d4b69ab4cd2"));
        assert!(USER_COLORS.contains(&a));
        let n: std::collections::HashSet<_> = (0..200).map(|i| user_color(&format!("compte-{i}"))).collect();
        assert_eq!(n.len(), 10, "les dix couleurs servent");
    }
}
