-- Racks (cahier des charges, section 9) : placements, circuits électriques, verrous d'édition.

CREATE TABLE circuits (
    id            TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    container_id  TEXT NOT NULL REFERENCES containers (id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    phase         TEXT NOT NULL,
    rating_a      REAL NOT NULL,
    position      INTEGER NOT NULL,
    rev           INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX circuits_container ON circuits (container_id, position);

-- Un exemplaire d'équipement (instance_id) ou un accessoire (article_id seul) posé sur une face.
CREATE TABLE placements (
    id            TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    container_id  TEXT NOT NULL REFERENCES containers (id) ON DELETE CASCADE,
    article_id    TEXT NOT NULL REFERENCES articles (id) ON DELETE CASCADE,
    instance_id   TEXT REFERENCES instances (id) ON DELETE CASCADE,
    face          TEXT NOT NULL,
    u_start       INTEGER NOT NULL,
    width         TEXT NOT NULL DEFAULT 'full',
    col           INTEGER NOT NULL DEFAULT 0,
    circuit_id    TEXT REFERENCES circuits (id) ON DELETE SET NULL,
    rev           INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX placements_container ON placements (container_id);
CREATE UNIQUE INDEX placements_instance ON placements (instance_id) WHERE instance_id IS NOT NULL;
CREATE UNIQUE INDEX placements_accessory ON placements (article_id) WHERE instance_id IS NULL;

-- Verrou d'édition d'un écran de conception : une seule personne modifie à la fois.
CREATE TABLE locks (
    view          TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    token_hash    TEXT NOT NULL,
    account_id    TEXT NOT NULL,
    account_name  TEXT NOT NULL,
    taken_at      TEXT NOT NULL,
    beat_at       TEXT NOT NULL
);
