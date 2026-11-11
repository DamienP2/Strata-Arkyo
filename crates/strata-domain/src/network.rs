//! Réseau (section 11.3) : VLAN, ports réseau, adresses IPv4 et plan multicast SMPTE ST 2110.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::net::Ipv4Addr;

/// Erreur de validation : champ et message en français.
pub type FieldError = (&'static str, String);

// ----- Adresses -----

pub fn parse_ip(s: &str) -> Option<u32> {
    s.trim().parse::<Ipv4Addr>().ok().map(u32::from)
}

pub fn ip_string(v: u32) -> String {
    Ipv4Addr::from(v).to_string()
}

/// Sous-réseau IPv4 (« 10.10.20.0/24 »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    pub net: u32,
    pub prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Cidr> {
        let (a, p) = s.trim().split_once('/')?;
        let prefix: u8 = p.trim().parse().ok()?;
        if !(8..=30).contains(&prefix) {
            return None;
        }
        let ip = parse_ip(a)?;
        let mask = Self::mask_of(prefix);
        Some(Cidr {
            net: ip & mask,
            prefix,
        })
    }

    fn mask_of(prefix: u8) -> u32 {
        u32::MAX << (32 - prefix as u32)
    }

    pub fn mask(&self) -> u32 {
        Self::mask_of(self.prefix)
    }

    pub fn broadcast(&self) -> u32 {
        self.net | !self.mask()
    }

    pub fn contains(&self, ip: u32) -> bool {
        ip & self.mask() == self.net
    }

    /// Adresse utilisable pour un hôte : dans le réseau, ni réseau ni diffusion.
    pub fn host(&self, ip: u32) -> bool {
        self.contains(ip) && ip != self.net && ip != self.broadcast()
    }

    /// Première adresse d'hôte libre (hors passerelle et adresses prises).
    pub fn first_free(&self, used: &HashSet<u32>, gateway: Option<u32>) -> Option<u32> {
        (self.net + 1..self.broadcast()).find(|ip| !used.contains(ip) && Some(*ip) != gateway)
    }

    pub fn size(&self) -> u32 {
        self.broadcast() - self.net - 1
    }
}

impl fmt::Display for Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", ip_string(self.net), self.prefix)
    }
}

// ----- VLAN -----

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VlanData {
    pub vid: u16,
    pub name: String,
    pub color: String,
    pub cidr: Option<String>,
    pub gateway: Option<String>,
    /// Rôle libre : production, contrôle, gestion, Dante…
    pub role: Option<String>,
}

fn blank(o: &Option<String>) -> Option<String> {
    o.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Normalise et vérifie un VLAN (l'unicité du numéro se vérifie dans le projet).
pub fn validate_vlan(v: &mut VlanData) -> Vec<FieldError> {
    let mut e = Vec::new();
    v.name = v.name.trim().to_string();
    v.cidr = blank(&v.cidr);
    v.gateway = blank(&v.gateway);
    v.role = blank(&v.role);
    if !(1..=4094).contains(&v.vid) {
        e.push(("vid", "Numéro de VLAN entre 1 et 4094.".into()));
    }
    if v.name.is_empty() {
        e.push(("name", "Donnez un nom au VLAN.".into()));
    } else if v.name.chars().count() > 60 {
        e.push(("name", "60 caractères au plus.".into()));
    }
    let color = v.color.trim();
    if color.is_empty() {
        v.color = "#3A7BD5".into();
    } else if !(color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit()))
    {
        e.push(("color", "Couleur au format #RRVVBB.".into()));
    }
    let cidr = match &v.cidr {
        Some(c) => match Cidr::parse(c) {
            Some(n) => {
                v.cidr = Some(n.to_string());
                Some(n)
            }
            None => {
                e.push((
                    "cidr",
                    "Sous-réseau au format 10.10.20.0/24 (masque de /8 à /30).".into(),
                ));
                None
            }
        },
        None => None,
    };
    if let Some(g) = &v.gateway {
        match parse_ip(g) {
            None => e.push(("gateway", "Adresse IPv4 invalide.".into())),
            Some(ip) => {
                v.gateway = Some(ip_string(ip));
                match cidr {
                    None => e.push(("gateway", "Indiquez d'abord le sous-réseau.".into())),
                    Some(n) if !n.host(ip) => e.push((
                        "gateway",
                        format!("La passerelle doit être une adresse d'hôte de {n}."),
                    )),
                    _ => {}
                }
            }
        }
    }
    e
}

// ----- Ports réseau -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortMode {
    #[default]
    Access,
    Trunk,
}

/// Réglages d'un port réseau (une prise RJ45 ou fibre d'un équipement).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetPort {
    pub mode: PortMode,
    /// VLAN du port en accès ; VLAN natif en trunk.
    pub vlan: Option<u16>,
    /// VLAN transportés en trunk.
    pub vlans: Vec<u16>,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    /// Réseau redondant ST 2022-7 : « A » ou « B ».
    pub redundancy: Option<String>,
    pub protocol: Option<String>,
}

impl NetPort {
    pub fn is_empty(&self) -> bool {
        *self == NetPort::default()
    }
}

/// Normalise un port ; `vids` liste les VLAN du projet.
pub fn validate_port(p: &mut NetPort, vids: &HashSet<u16>) -> Vec<FieldError> {
    let mut e = Vec::new();
    p.ip = blank(&p.ip);
    p.hostname = blank(&p.hostname);
    p.protocol = blank(&p.protocol);
    p.redundancy = blank(&p.redundancy).map(|r| r.to_uppercase());
    if let Some(r) = &p.redundancy {
        if r != "A" && r != "B" {
            e.push(("redundancy", "Réseau A ou B.".into()));
        }
    }
    if let Some(v) = p.vlan {
        if !vids.contains(&v) {
            e.push(("vlan", format!("Le VLAN {v} n'existe pas dans le projet.")));
        }
    }
    p.vlans.sort_unstable();
    p.vlans.dedup();
    if p.mode == PortMode::Access {
        p.vlans.clear();
    }
    if let Some(v) = p.vlans.iter().find(|v| !vids.contains(v)) {
        e.push(("vlans", format!("Le VLAN {v} n'existe pas dans le projet.")));
    }
    if let Some(ip) = &p.ip {
        match parse_ip(ip) {
            Some(v) => p.ip = Some(ip_string(v)),
            None => e.push(("ip", "Adresse IPv4 invalide.".into())),
        }
    }
    if let Some(h) = &p.hostname {
        let ok = h.len() <= 63
            && h.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
            && !h.starts_with('-');
        if !ok {
            e.push((
                "hostname",
                "Nom d'hôte : lettres, chiffres, tiret et point (63 au plus).".into(),
            ));
        }
    }
    e
}

// ----- Multicast SMPTE ST 2110 -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Essence {
    Video,
    Audio,
    Data,
}

impl Essence {
    /// Déduite de la famille du signal : V vidéo, A audio, le reste données annexes.
    pub fn of_family(f: Option<&str>) -> Essence {
        match f {
            Some("V") => Essence::Video,
            Some("A") => Essence::Audio,
            _ => Essence::Data,
        }
    }

    /// Deuxième octet du groupe et suffixe du port UDP : 20, 30, 40 (ST 2110-20/-30/-40).
    pub fn code(self) -> u8 {
        match self {
            Essence::Video => 20,
            Essence::Audio => 30,
            Essence::Data => 40,
        }
    }

    pub fn udp(self) -> u16 {
        50000 + self.code() as u16
    }

    pub fn label(self) -> &'static str {
        match self {
            Essence::Video => "Vidéo",
            Essence::Audio => "Audio",
            Essence::Data => "Données",
        }
    }
}

/// Groupe multicast `239.{essence}.{bloc}.{1–255}` à la position `n` (0 = premier).
pub fn group_at(e: Essence, n: u32) -> Option<u32> {
    let bloc = n / 255;
    if bloc > 255 {
        return None;
    }
    let host = n % 255 + 1;
    Some(u32::from(Ipv4Addr::new(
        239,
        e.code(),
        bloc as u8,
        host as u8,
    )))
}

/// Premier groupe libre de l'essence.
pub fn next_group(e: Essence, used: &HashSet<u32>) -> Option<u32> {
    (0..256 * 255)
        .map_while(|n| group_at(e, n))
        .find(|g| !used.contains(g))
}

/// Une adresse multicast saisie : 224.0.0.0/4, hors plage locale 224.0.0.x.
pub fn validate_multicast(s: &str) -> Result<u32, String> {
    let ip = parse_ip(s).ok_or_else(|| "Adresse IPv4 invalide.".to_string())?;
    let o = Ipv4Addr::from(ip).octets();
    if !(224..=239).contains(&o[0]) {
        return Err("Une adresse multicast va de 224.0.0.0 à 239.255.255.255.".into());
    }
    if o[0] == 224 && o[1] == 0 && o[2] == 0 {
        return Err("224.0.0.x est réservé au réseau local.".into());
    }
    Ok(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_and_first_free() {
        let n = Cidr::parse("10.10.20.7/24").unwrap();
        assert_eq!(n.to_string(), "10.10.20.0/24");
        assert_eq!(ip_string(n.broadcast()), "10.10.20.255");
        let gw = parse_ip("10.10.20.1");
        let mut used = HashSet::new();
        assert_eq!(ip_string(n.first_free(&used, gw).unwrap()), "10.10.20.2");
        used.insert(parse_ip("10.10.20.2").unwrap());
        assert_eq!(ip_string(n.first_free(&used, gw).unwrap()), "10.10.20.3");
        assert!(!n.host(n.net));
        assert!(Cidr::parse("10.0.0.0/31").is_none());
        let full = Cidr::parse("10.0.0.0/30").unwrap();
        let used: HashSet<u32> = [full.net + 1, full.net + 2].into();
        assert!(full.first_free(&used, None).is_none());
    }

    #[test]
    fn vlan_rules() {
        let mut v = VlanData {
            vid: 0,
            name: " ".into(),
            gateway: Some("10.0.0.1".into()),
            ..Default::default()
        };
        let e = validate_vlan(&mut v);
        let f: Vec<_> = e.iter().map(|e| e.0).collect();
        assert_eq!(f, vec!["vid", "name", "gateway"]);
        let mut v = VlanData {
            vid: 20,
            name: "Prod".into(),
            cidr: Some("10.10.20.9/24".into()),
            gateway: Some("10.10.20.255".into()),
            ..Default::default()
        };
        assert_eq!(validate_vlan(&mut v)[0].0, "gateway");
        assert_eq!(v.cidr.as_deref(), Some("10.10.20.0/24"));
        assert_eq!(v.color, "#3A7BD5");
    }

    #[test]
    fn multicast_plan() {
        assert_eq!(
            ip_string(group_at(Essence::Video, 0).unwrap()),
            "239.20.0.1"
        );
        assert_eq!(
            ip_string(group_at(Essence::Audio, 255).unwrap()),
            "239.30.1.1"
        );
        assert_eq!(Essence::Data.udp(), 50040);
        let used: HashSet<u32> = [group_at(Essence::Video, 0).unwrap()].into();
        assert_eq!(
            ip_string(next_group(Essence::Video, &used).unwrap()),
            "239.20.0.2"
        );
        assert!(validate_multicast("239.1.2.3").is_ok());
        assert!(validate_multicast("224.0.0.5").is_err());
        assert!(validate_multicast("10.0.0.1").is_err());
        assert_eq!(Essence::of_family(Some("A")), Essence::Audio);
    }

    #[test]
    fn port_rules() {
        let vids: HashSet<u16> = [10, 20].into();
        let mut p = NetPort {
            mode: PortMode::Access,
            vlan: Some(30),
            vlans: vec![10],
            ip: Some(" 10.0.0.300 ".into()),
            redundancy: Some("c".into()),
            ..Default::default()
        };
        let f: Vec<_> = validate_port(&mut p, &vids).iter().map(|e| e.0).collect();
        assert_eq!(f, vec!["redundancy", "vlan", "ip"]);
        assert!(p.vlans.is_empty());
    }
}
