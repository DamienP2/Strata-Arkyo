//! Dessins et plan de feu (section 11.2) : pages par nature, éléments posés en mm de page,
//! blocs équipement tirés de la nomenclature, liens, adressage DMX.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub type FieldError = (&'static str, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nature {
    #[default]
    Schematic,
    Video,
    Audio,
    Lighting,
}

impl Nature {
    /// Fonctions (familles) des équipements proposés ; un schéma de principe prend tout.
    pub fn functions(self) -> &'static [&'static str] {
        match self {
            Nature::Schematic => &[],
            Nature::Video => &["V"],
            Nature::Audio => &["A"],
            Nature::Lighting => &["L"],
        }
    }

    pub fn accepts(self, functions: &[String]) -> bool {
        let f = self.functions();
        f.is_empty() || functions.iter().any(|x| f.contains(&x.as_str()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Nature::Schematic => "Schéma de principe",
            Nature::Video => "Implantation vidéo",
            Nature::Audio => "Implantation audio",
            Nature::Lighting => "Plan de feu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmxDisplay {
    #[default]
    Address,
    Channel,
    Number,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageData {
    pub name: String,
    /// A4 à A0.
    pub paper: String,
    pub landscape: bool,
    /// Pas de la grille en mm de page ; 0 : pas de grille.
    pub grid_mm: f64,
    pub snap: bool,
    pub background: Option<String>,
    /// Échelle réelle facultative (1:N).
    pub scale: Option<u32>,
    pub legend: bool,
    pub dmx_display: DmxDisplay,
    pub elements: Vec<Element>,
}

impl Default for PageData {
    fn default() -> Self {
        PageData {
            name: String::new(),
            paper: "A3".into(),
            landscape: true,
            grid_mm: 5.0,
            snap: true,
            background: None,
            scale: None,
            legend: true,
            dmx_display: DmxDisplay::Address,
            elements: Vec::new(),
        }
    }
}

/// Taille d'une feuille en mm (portrait).
pub fn paper_mm(p: &str) -> Option<(f64, f64)> {
    Some(match p {
        "A4" => (210.0, 297.0),
        "A3" => (297.0, 420.0),
        "A2" => (420.0, 594.0),
        "A1" => (594.0, 841.0),
        "A0" => (841.0, 1189.0),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStyle {
    #[default]
    Straight,
    Orthogonal,
    Curve,
}

/// Bout d'un lien : accroché à un élément (son centre), sinon un point libre.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Anchor {
    pub element: Option<String>,
    pub x: f64,
    pub y: f64,
}

/// Ce que l'équipement garde de la nomenclature (mis à jour par la synchronisation).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub item_id: Option<String>,
    pub name: String,
    pub reference: Option<String>,
    pub category: Option<String>,
    pub tag: String,
    pub function: Option<String>,
    pub images: Vec<String>,
    pub dmx_channels: Option<u32>,
    pub dmx_modes: Vec<(String, u32)>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DmxPatch {
    pub console: u32,
    pub universe: u32,
    pub address: u32,
    /// Mode : fixe le nombre de canaux ; à défaut, les canaux par défaut.
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum Element {
    Text {
        id: String,
        x: f64,
        y: f64,
        #[serde(default)]
        w: f64,
        #[serde(default)]
        rot: f64,
        text: String,
        #[serde(default = "size4")]
        size: f64,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        bold: bool,
    },
    Rect {
        id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default)]
        rot: f64,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default = "one")]
        width: f64,
        #[serde(default)]
        radius: f64,
    },
    Ellipse {
        id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default)]
        rot: f64,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default = "one")]
        width: f64,
    },
    Line {
        id: String,
        points: Vec<[f64; 2]>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default = "one")]
        width: f64,
        #[serde(default)]
        dash: bool,
        #[serde(default)]
        arrow_start: bool,
        #[serde(default)]
        arrow_end: bool,
    },
    Image {
        id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default)]
        rot: f64,
        file: String,
    },
    Zone {
        id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        label: String,
        #[serde(default)]
        color: Option<String>,
    },
    Link {
        id: String,
        from: Anchor,
        to: Anchor,
        #[serde(default)]
        style: LinkStyle,
        #[serde(default)]
        arrow_start: bool,
        #[serde(default = "yes")]
        arrow_end: bool,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default = "one")]
        width: f64,
        #[serde(default)]
        signal_id: Option<String>,
    },
    Equipment {
        id: String,
        instance_id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default)]
        rot: f64,
        /// Libellé propre (sinon le nom du catalogue).
        #[serde(default)]
        label: Option<String>,
        /// Source d'image : pictogramme, image du catalogue (indice) ou aucune.
        #[serde(default)]
        image: Option<String>,
        /// Éléments affichés : au moins un parmi label, name, reference, tag, category, dmx.
        show: Vec<String>,
        #[serde(default)]
        frame: Option<String>,
        #[serde(default)]
        snapshot: Snapshot,
        #[serde(default)]
        dmx: Option<DmxPatch>,
    },
}

fn size4() -> f64 {
    4.0
}
fn one() -> f64 {
    0.5
}
fn yes() -> bool {
    true
}

impl Element {
    pub fn id(&self) -> &str {
        match self {
            Element::Text { id, .. }
            | Element::Rect { id, .. }
            | Element::Ellipse { id, .. }
            | Element::Line { id, .. }
            | Element::Image { id, .. }
            | Element::Zone { id, .. }
            | Element::Link { id, .. }
            | Element::Equipment { id, .. } => id,
        }
    }
}

const SHOW: [&str; 6] = ["label", "name", "reference", "tag", "category", "dmx"];
const MAX_ELEMENTS: usize = 5000;

fn color_ok(c: &Option<String>) -> bool {
    c.as_deref().is_none_or(|c| {
        c == "none"
            || (c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()))
    })
}

fn pos(v: f64) -> bool {
    v > 0.0 && num_ok(v)
}

fn num_ok(v: f64) -> bool {
    v.is_finite() && v.abs() <= 10_000.0
}

/// Canaux occupés par un projecteur : ceux du mode choisi, sinon ceux par défaut.
pub fn channels(s: &Snapshot, mode: Option<&str>) -> Option<u32> {
    mode.and_then(|m| s.dmx_modes.iter().find(|(n, _)| n == m).map(|(_, c)| *c))
        .or(s.dmx_channels)
        .filter(|c| (1..=512).contains(c))
}

pub fn validate_page(p: &mut PageData) -> Vec<FieldError> {
    let mut e = Vec::new();
    p.name = p.name.trim().to_string();
    if p.name.is_empty() || p.name.chars().count() > 80 {
        e.push(("name", "Nom de 1 à 80 caractères.".into()));
    }
    if paper_mm(&p.paper).is_none() {
        e.push(("paper", "Format de A4 à A0.".into()));
    }
    if !(0.0..=50.0).contains(&p.grid_mm) {
        e.push(("grid_mm", "Grille de 0 à 50 mm.".into()));
    }
    if p.scale.is_some_and(|s| !(1..=100_000).contains(&s)) {
        e.push(("scale", "Échelle de 1:1 à 1:100 000.".into()));
    }
    if !color_ok(&p.background) {
        e.push(("background", "Couleur au format #RRVVBB.".into()));
    }
    if p.elements.len() > MAX_ELEMENTS {
        e.push((
            "elements",
            format!("{MAX_ELEMENTS} éléments au plus par page."),
        ));
    }
    let mut ids = HashSet::new();
    let mut instances = HashSet::new();
    let all_ids: HashSet<String> = p.elements.iter().map(|x| x.id().to_string()).collect();
    for el in p.elements.iter_mut() {
        if el.id().is_empty() || el.id().len() > 64 || !ids.insert(el.id().to_string()) {
            e.push(("elements", "Chaque élément a un identifiant unique.".into()));
            break;
        }
        let bad = match el {
            Element::Text {
                x,
                y,
                text,
                size,
                color,
                ..
            } => {
                *text = text.trim_end().to_string();
                !num_ok(*x)
                    || !num_ok(*y)
                    || text.is_empty()
                    || text.chars().count() > 2000
                    || !(1.0..=200.0).contains(size)
                    || !color_ok(color)
            }
            Element::Rect {
                x,
                y,
                w,
                h,
                stroke,
                fill,
                width,
                ..
            }
            | Element::Ellipse {
                x,
                y,
                w,
                h,
                stroke,
                fill,
                width,
                ..
            } => {
                !num_ok(*x)
                    || !num_ok(*y)
                    || !pos(*w)
                    || !pos(*h)
                    || !color_ok(stroke)
                    || !color_ok(fill)
                    || !(0.0..=20.0).contains(width)
            }
            Element::Line {
                points,
                stroke,
                width,
                ..
            } => {
                points.len() < 2
                    || points.len() > 500
                    || points.iter().any(|p| !num_ok(p[0]) || !num_ok(p[1]))
                    || !color_ok(stroke)
                    || !(0.0..=20.0).contains(width)
            }
            Element::Image {
                x, y, w, h, file, ..
            } => !num_ok(*x) || !num_ok(*y) || !pos(*w) || !pos(*h) || file.is_empty(),
            Element::Zone {
                x,
                y,
                w,
                h,
                label,
                color,
                ..
            } => {
                *label = label.trim().to_string();
                !num_ok(*x)
                    || !num_ok(*y)
                    || !pos(*w)
                    || !pos(*h)
                    || label.chars().count() > 80
                    || !color_ok(color)
            }
            Element::Link {
                from,
                to,
                label,
                color,
                width,
                ..
            } => {
                let anchor_bad = |a: &Anchor| {
                    !num_ok(a.x)
                        || !num_ok(a.y)
                        || a.element.as_ref().is_some_and(|x| !all_ids.contains(x))
                };
                if label.as_deref().is_some_and(|l| l.trim().is_empty()) {
                    *label = None;
                }
                anchor_bad(from)
                    || anchor_bad(to)
                    || label.as_deref().is_some_and(|l| l.chars().count() > 80)
                    || !color_ok(color)
                    || !(0.1..=20.0).contains(width)
            }
            Element::Equipment {
                instance_id,
                x,
                y,
                w,
                h,
                show,
                frame,
                label,
                dmx,
                snapshot,
                ..
            } => {
                if label.as_deref().is_some_and(|l| l.trim().is_empty()) {
                    *label = None;
                }
                show.retain(|s| SHOW.contains(&s.as_str()));
                show.dedup();
                if show.is_empty() {
                    e.push((
                        "elements",
                        "Un bloc équipement affiche au moins une information.".into(),
                    ));
                }
                if !instances.insert(instance_id.clone()) {
                    e.push(("elements", "Un exemplaire ne se place qu'une fois.".into()));
                }
                if let Some(d) = dmx {
                    if let Some(m) = e_dmx(d, snapshot) {
                        e.push(("dmx", m));
                    }
                }
                !num_ok(*x)
                    || !num_ok(*y)
                    || !pos(*w)
                    || !pos(*h)
                    || !color_ok(frame)
                    || label.as_deref().is_some_and(|l| l.chars().count() > 80)
            }
        };
        if bad {
            e.push(("elements", format!("Élément {} invalide.", el.id())));
        }
    }
    e
}

fn e_dmx(d: &DmxPatch, s: &Snapshot) -> Option<String> {
    if d.console < 1 {
        return Some("Numéro console à partir de 1.".into());
    }
    if !(1..=65_535).contains(&d.universe) {
        return Some("Univers de 1 à 65 535.".into());
    }
    if !(1..=512).contains(&d.address) {
        return Some("Adresse de 1 à 512.".into());
    }
    if d.mode
        .as_deref()
        .is_some_and(|m| !s.dmx_modes.iter().any(|(n, _)| n == m))
    {
        return Some("Mode inconnu pour cet appareil.".into());
    }
    let n = channels(s, d.mode.as_deref()).unwrap_or(1);
    if d.address + n - 1 > 512 {
        return Some(format!(
            "{n} canaux à partir de {} dépassent 512.",
            d.address
        ));
    }
    None
}

/// Un projecteur adressé, vu sur l'ensemble des plans de feu du projet.
#[derive(Debug, Clone, PartialEq)]
pub struct Fixture {
    pub key: String,
    pub item_id: Option<String>,
    pub patch: Option<DmxPatch>,
    pub channels: u32,
}

/// Proposition d'adresse : même univers qu'un appareil identique déjà posé, première plage
/// libre de la taille du mode, univers suivant au-delà de 512 ; numéro console = plus grand + 1.
pub fn suggest(existing: &[Fixture], item_id: Option<&str>, channels: u32) -> DmxPatch {
    let n = channels.clamp(1, 512);
    let mut used: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    let mut console = 0;
    let mut start_universe = None;
    for f in existing {
        if let Some(p) = &f.patch {
            used.entry(p.universe)
                .or_default()
                .push((p.address, p.address + f.channels.max(1) - 1));
            console = console.max(p.console);
            if start_universe.is_none() && item_id.is_some() && f.item_id.as_deref() == item_id {
                start_universe = Some(p.universe);
            }
        }
    }
    let mut u = start_universe.unwrap_or_else(|| used.keys().next().copied().unwrap_or(1));
    loop {
        let ranges = used.get(&u).cloned().unwrap_or_default();
        let mut a = 1;
        let mut sorted = ranges.clone();
        sorted.sort_unstable();
        for (s, e) in sorted {
            if a + n - 1 < s {
                break;
            }
            if e >= a {
                a = e + 1;
            }
        }
        if a + n - 1 <= 512 || u >= 65_535 {
            return DmxPatch {
                console: console + 1,
                universe: u,
                address: a.min(512),
                mode: None,
            };
        }
        u += 1;
    }
}

/// Paires de projecteurs dont les plages se chevauchent dans un même univers.
pub fn conflicts(fx: &[Fixture]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let patched: Vec<(&Fixture, &DmxPatch)> = fx
        .iter()
        .filter_map(|f| f.patch.as_ref().map(|p| (f, p)))
        .collect();
    for (i, (a, pa)) in patched.iter().enumerate() {
        for (b, pb) in patched.iter().skip(i + 1) {
            if pa.universe != pb.universe {
                continue;
            }
            let (a0, a1) = (pa.address, pa.address + a.channels.max(1) - 1);
            let (b0, b1) = (pb.address, pb.address + b.channels.max(1) - 1);
            if a0 <= b1 && b0 <= a1 {
                out.push((a.key.clone(), b.key.clone()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(key: &str, item: &str, u: u32, a: u32, n: u32, console: u32) -> Fixture {
        Fixture {
            key: key.into(),
            item_id: Some(item.into()),
            patch: Some(DmxPatch {
                console,
                universe: u,
                address: a,
                mode: None,
            }),
            channels: n,
        }
    }

    #[test]
    fn suggestion_rules() {
        // Rien de posé : univers 1, adresse 1, console 1.
        let p = suggest(&[], Some("spot"), 16);
        assert_eq!((p.universe, p.address, p.console), (1, 1, 1));
        // Même univers qu'un appareil identique, première plage libre.
        let ex = [
            fx("a", "wash", 1, 1, 20, 1),
            fx("b", "spot", 2, 1, 16, 2),
            fx("c", "spot", 2, 33, 16, 5),
        ];
        let p = suggest(&ex, Some("spot"), 16);
        assert_eq!((p.universe, p.address, p.console), (2, 17, 6));
        // Trou trop petit : on passe derrière.
        let p = suggest(&ex, Some("spot"), 17);
        assert_eq!((p.universe, p.address), (2, 49));
        // Univers plein : univers suivant.
        let full = [fx("a", "spot", 3, 1, 500, 1)];
        let p = suggest(&full, Some("spot"), 16);
        assert_eq!((p.universe, p.address), (4, 1));
    }

    #[test]
    fn overlap_detection() {
        let f = [
            fx("a", "x", 1, 1, 10, 1),
            fx("b", "x", 1, 10, 4, 2),
            fx("c", "x", 2, 1, 10, 3),
            fx("d", "x", 1, 11, 2, 4),
        ];
        let c = conflicts(&f);
        assert_eq!(
            c,
            vec![
                ("a".to_string(), "b".to_string()),
                ("b".to_string(), "d".to_string())
            ]
        );
    }

    #[test]
    fn page_rules() {
        let snap = Snapshot {
            dmx_channels: Some(16),
            dmx_modes: vec![("Étendu".into(), 40)],
            ..Default::default()
        };
        let eq = |id: &str, inst: &str, dmx: Option<DmxPatch>| Element::Equipment {
            id: id.into(),
            instance_id: inst.into(),
            x: 10.0,
            y: 10.0,
            w: 30.0,
            h: 20.0,
            rot: 0.0,
            label: None,
            image: None,
            show: vec!["name".into()],
            frame: None,
            snapshot: snap.clone(),
            dmx,
        };
        let mut p = PageData {
            name: "Plan de feu".into(),
            elements: vec![
                eq(
                    "e1",
                    "i1",
                    Some(DmxPatch {
                        console: 1,
                        universe: 1,
                        address: 480,
                        mode: Some("Étendu".into()),
                    }),
                ),
                eq("e2", "i1", None),
                Element::Link {
                    id: "l".into(),
                    from: Anchor {
                        element: Some("e1".into()),
                        x: 0.0,
                        y: 0.0,
                    },
                    to: Anchor {
                        element: Some("zz".into()),
                        x: 0.0,
                        y: 0.0,
                    },
                    style: LinkStyle::Curve,
                    arrow_start: false,
                    arrow_end: true,
                    label: None,
                    color: None,
                    width: 0.5,
                    signal_id: None,
                },
            ],
            ..Default::default()
        };
        let e: Vec<&str> = validate_page(&mut p).iter().map(|e| e.0).collect();
        assert_eq!(
            e,
            vec!["dmx", "elements", "elements"],
            "dépassement 512, doublon, lien vers rien"
        );
        assert_eq!(channels(&snap, Some("Étendu")), Some(40));
        assert_eq!(channels(&snap, None), Some(16));
        assert!(Nature::Lighting.accepts(&["L".into()]));
        assert!(!Nature::Video.accepts(&["A".into()]));
        assert!(Nature::Schematic.accepts(&[]));
    }
}
