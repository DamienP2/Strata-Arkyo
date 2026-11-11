-- Catalogue (cahier des charges, section 5) : référentiels, fiches, imports, instantanés.
-- Les données propres à chaque genre vivent dans une colonne JSON : le moteur les
-- vérifie à l'écriture, et une version plus ancienne relit sans peine une fiche enrichie.

CREATE TABLE ref_items (
    kind        TEXT NOT NULL,
    id          TEXT NOT NULL,
    name        TEXT NOT NULL,
    -- Nom sans accents ni casse : sert aux contrôles d'unicité et à la recherche.
    name_norm   TEXT NOT NULL,
    parent_id   TEXT,
    data        TEXT NOT NULL DEFAULT '{}',
    active      INTEGER NOT NULL DEFAULT 1,
    position    INTEGER NOT NULL DEFAULT 0,
    builtin     INTEGER NOT NULL DEFAULT 0,
    protected   INTEGER NOT NULL DEFAULT 0,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (kind, id)
);
CREATE INDEX ref_items_name ON ref_items (kind, name_norm);

CREATE TABLE catalog_items (
    id              TEXT PRIMARY KEY,
    kind            TEXT NOT NULL,
    name            TEXT NOT NULL,
    name_norm       TEXT NOT NULL,
    reference       TEXT,
    reference_norm  TEXT,
    brand_id        TEXT,
    data            TEXT NOT NULL,
    active          INTEGER NOT NULL DEFAULT 1,
    rev             INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    created_by      TEXT,
    updated_by      TEXT
);
CREATE INDEX catalog_items_kind_name ON catalog_items (kind, name_norm);
CREATE INDEX catalog_items_reference ON catalog_items (reference_norm) WHERE reference_norm IS NOT NULL;

-- Fichier d'import lu et mis de côté entre l'analyse et l'exécution (24 h).
CREATE TABLE import_sessions (
    id          TEXT PRIMARY KEY,
    account_id  TEXT NOT NULL,
    filename    TEXT NOT NULL,
    headers     TEXT NOT NULL,
    rows        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL
);

-- État d'une partie du catalogue avant un échange ou un import, pour revenir en arrière.
CREATE TABLE catalog_snapshots (
    id          TEXT PRIMARY KEY,
    created_at  TEXT NOT NULL,
    actor_id    TEXT,
    reason      TEXT NOT NULL,
    sections    TEXT NOT NULL,
    data        TEXT NOT NULL
);
