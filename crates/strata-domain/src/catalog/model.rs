//! Modèle du catalogue (cahier des charges, section 5).
//!
//! Les référentiels partagent une forme commune (nom, actif, ordre) et portent leurs
//! données propres ; les fiches (équipements, câbles, modèles de câble, adaptateurs)
//! sont décrites en entier, telles qu'elles s'échangent entre sites.

use serde::{Deserialize, Serialize};

// ----- Référentiels -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Function,
    Signal,
    Connector,
    Brand,
    SystemCategory,
    Category,
    EquipmentType,
    Phase,
    Dimension,
}

impl RefKind {
    pub const ALL: [RefKind; 9] = [
        RefKind::Function,
        RefKind::Phase,
        RefKind::SystemCategory,
        RefKind::Category,
        RefKind::EquipmentType,
        RefKind::Brand,
        RefKind::Connector,
        RefKind::Signal,
        RefKind::Dimension,
    ];

    /// Segment d'URL de l'API.
    pub fn path(self) -> &'static str {
        match self {
            RefKind::Function => "functions",
            RefKind::Signal => "signals",
            RefKind::Connector => "connectors",
            RefKind::Brand => "brands",
            RefKind::SystemCategory => "system-categories",
            RefKind::Category => "categories",
            RefKind::EquipmentType => "types",
            RefKind::Phase => "phases",
            RefKind::Dimension => "dimensions",
        }
    }

    pub fn from_path(p: &str) -> Option<RefKind> {
        RefKind::ALL.into_iter().find(|k| k.path() == p)
    }

    /// Nom au singulier, pour les messages.
    pub fn label(self) -> &'static str {
        match self {
            RefKind::Function => "Fonction",
            RefKind::Signal => "Signal",
            RefKind::Connector => "Connecteur",
            RefKind::Brand => "Marque",
            RefKind::SystemCategory => "Catégorie système",
            RefKind::Category => "Catégorie",
            RefKind::EquipmentType => "Type",
            RefKind::Phase => "Phase",
            RefKind::Dimension => "Préréglage",
        }
    }

    /// Le nom est unique parmi les frères (types) plutôt que dans tout le référentiel.
    pub fn unique_among_siblings(self) -> bool {
        matches!(self, RefKind::EquipmentType)
    }

    /// Les préréglages de dimensions se distinguent par leur valeur, pas par leur nom.
    pub fn named(self) -> bool {
        !matches!(self, RefKind::Dimension)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Redundancy {
    /// A / B
    Ab,
    /// Primary / Secondary
    PrimarySecondary,
    /// Bleu / Rouge
    BlueRed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FunctionData {
    /// Clé : majuscules, chiffres et tiret bas. Sert d'identifiant.
    pub key: String,
    pub color: String,
    /// Préfixe de numérotation des câbles.
    pub cable_prefix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SignalData {
    pub color: String,
    pub function: String,
    pub prefix: String,
    pub suffix: String,
    pub impedance_ohm: Option<f64>,
    pub min_bandwidth_mhz: Option<f64>,
    /// Connecteurs compatibles : au moins un.
    pub connectors: Vec<String>,
    pub ip: bool,
    pub multicast: bool,
    pub redundancy: Option<Redundancy>,
    /// Signal proposé par défaut pour ses connecteurs.
    pub default: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectorData {
    pub variants: Vec<String>,
    /// Identifiant du dessin du connecteur.
    pub symbol: String,
    pub color: String,
    pub functions: Vec<String>,
    /// Existe en coque standard (format D) pour les panels.
    pub d_format: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrandData {
    pub website: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemCategoryData {
    pub function: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryData {
    pub color: String,
    pub icon: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TypeData {
    /// Fonction du type ; un sous-type hérite de celle de son parent.
    pub function: String,
    pub icon: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionKind {
    HeightU,
    Width,
    DepthMm,
    RearReserveMm,
    CableLengthM,
    BendRadiusMm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionData {
    pub kind: DimensionKind,
    pub value: f64,
    /// Longueur de câble livrée en touret.
    #[serde(default)]
    pub reel: bool,
}

// ----- Fiches -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Equipment,
    Cable,
    CableModel,
    Adapter,
}

impl ItemKind {
    pub const ALL: [ItemKind; 4] = [
        ItemKind::Equipment,
        ItemKind::Cable,
        ItemKind::CableModel,
        ItemKind::Adapter,
    ];

    pub fn path(self) -> &'static str {
        match self {
            ItemKind::Equipment => "equipment",
            ItemKind::Cable => "cables",
            ItemKind::CableModel => "cable-models",
            ItemKind::Adapter => "adapters",
        }
    }

    pub fn from_path(p: &str) -> Option<ItemKind> {
        ItemKind::ALL.into_iter().find(|k| k.path() == p)
    }

    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Equipment => "Équipement",
            ItemKind::Cable => "Câble",
            ItemKind::CableModel => "Type de câble",
            ItemKind::Adapter => "Adaptateur",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Width {
    #[default]
    Full,
    Half,
    Third,
    Quarter,
}

impl Width {
    pub fn fraction(self) -> f64 {
        match self {
            Width::Full => 1.0,
            Width::Half => 0.5,
            Width::Third => 1.0 / 3.0,
            Width::Quarter => 0.25,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Width::Full => "pleine",
            Width::Half => "1/2",
            Width::Third => "1/3",
            Width::Quarter => "1/4",
        }
    }
}

/// Format physique : rackable, ou posé librement (sur une étagère).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Format {
    Rack {
        height_u: f64,
        #[serde(default)]
        width: Width,
        #[serde(default)]
        depth_mm: f64,
        /// Réserve de câblage arrière ; vide = réglage du site (150 mm par défaut).
        #[serde(default)]
        rear_reserve_mm: Option<f64>,
    },
    Free {
        #[serde(default)]
        length_mm: f64,
        #[serde(default)]
        width_mm: f64,
        #[serde(default)]
        height_mm: f64,
    },
}

impl Default for Format {
    fn default() -> Self {
        Format::Rack {
            height_u: 1.0,
            width: Width::Full,
            depth_mm: 0.0,
            rear_reserve_mm: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Face {
    Front,
    #[default]
    Rear,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    In,
    Out,
    #[default]
    Bidir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Gender {
    M,
    #[default]
    F,
}

/// Genre d'une extrémité de câble ou d'adaptateur : M, F ou neutre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EndGender {
    #[default]
    M,
    F,
    N,
}

impl EndGender {
    pub fn letter(self) -> &'static str {
        match self {
            EndGender::M => "M",
            EndGender::F => "F",
            EndGender::N => "N",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkRole {
    Terminal,
    Switching,
    Transit,
}

fn yes() -> bool {
    true
}

fn one() -> u32 {
    1
}

/// Groupe de ports identiques sur une face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortGroup {
    #[serde(default)]
    pub face: Face,
    pub connector_id: String,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub signal_id: Option<String>,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)]
    pub gender: Gender,
    #[serde(default = "one")]
    pub quantity: u32,
    #[serde(default)]
    pub label: Option<String>,
    /// Sans rôle, le port n'apparaît pas dans le réseau.
    #[serde(default)]
    pub network_role: Option<NetworkRole>,
    #[serde(default = "yes")]
    pub redundancy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpControl {
    #[default]
    OutOfBand,
    InBand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpFlow {
    pub signal_id: String,
    /// Essence du flux ; à défaut, déduite de la famille du signal.
    #[serde(default)]
    pub essence: Option<crate::network::Essence>,
    #[serde(default)]
    pub inputs: u32,
    #[serde(default)]
    pub outputs: u32,
}

/// Flux IP SMPTE ST 2110 : chaque sortie donne un groupe multicast au plan d'adressage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IpSpec {
    pub control: IpControl,
    pub flows: Vec<IpFlow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmxMode {
    pub name: String,
    pub channels: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dmx {
    pub default_channels: u32,
    #[serde(default)]
    pub modes: Vec<DmxMode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotPlacement {
    pub equipment_id: String,
    /// Premier emplacement occupé, à partir de 1.
    pub slot: u32,
}

/// Un équipement modulaire est soit un châssis hôte, soit un composant, jamais les deux.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Modular {
    Host {
        slots: u32,
        #[serde(default)]
        default_composition: Vec<SlotPlacement>,
    },
    Component {
        slots: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomValue {
    Text {
        #[serde(default)]
        value: Option<String>,
    },
    Number {
        #[serde(default)]
        value: Option<f64>,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        step: Option<f64>,
        #[serde(default)]
        unit: Option<String>,
    },
    Bool {
        #[serde(default)]
        value: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomField {
    pub name: String,
    #[serde(flatten)]
    pub value: CustomValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equipment {
    pub name: String,
    pub reference: Option<String>,
    pub brand_id: Option<String>,
    pub category_id: Option<String>,
    pub type_id: Option<String>,
    /// Visuel en baie (moniteur, switch…) ; vide : déduit du type.
    pub visual: Option<String>,
    /// Clés de fonctions.
    pub functions: Vec<String>,
    pub phases: Vec<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub active: Option<bool>,
    pub format: Format,
    pub weight_kg: Option<f64>,
    pub power_w: Option<f64>,
    pub dual_psu: bool,
    pub btu_h: Option<f64>,
    pub ip: Option<IpSpec>,
    pub dmx: Option<Dmx>,
    pub modular: Option<Modular>,
    pub ports: Vec<PortGroup>,
    pub custom: Vec<CustomField>,
    pub documents: Vec<Document>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CableEnd {
    pub connector_id: String,
    #[serde(default)]
    pub gender: EndGender,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cable {
    /// Vide : proposé à partir des connecteurs (« ConnG G ↔ ConnD G »).
    pub name: String,
    pub reference: Option<String>,
    pub function: Option<String>,
    pub brand_id: Option<String>,
    pub model_id: Option<String>,
    pub left: CableEnd,
    pub right: CableEnd,
    pub usages: Vec<String>,
    pub lengths_m: Vec<f64>,
    pub custom_min_m: Option<f64>,
    pub custom_max_m: Option<f64>,
    pub notes: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CableModel {
    pub name: String,
    pub reference: Option<String>,
    pub function: Option<String>,
    pub brand_id: Option<String>,
    pub impedance_ohm: Option<f64>,
    pub bandwidth_mhz: Option<f64>,
    pub diameter_mm: Option<f64>,
    pub conductor: Option<String>,
    pub shielding: Option<String>,
    pub jacket: Option<String>,
    pub fire_rating: Option<String>,
    pub custom: Vec<CustomField>,
    pub datasheet_url: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdapterIn {
    pub connector_id: String,
    #[serde(default)]
    pub gender: EndGender,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterOut {
    pub connector_id: String,
    #[serde(default)]
    pub gender: EndGender,
    #[serde(default = "one")]
    pub quantity: u32,
    #[serde(default)]
    pub label: Option<String>,
}

/// Exactement une entrée et au moins une sortie.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adapter {
    pub name: String,
    pub reference: Option<String>,
    pub function: Option<String>,
    pub notes: Option<String>,
    pub active: Option<bool>,
    pub input: AdapterIn,
    pub outputs: Vec<AdapterOut>,
}
