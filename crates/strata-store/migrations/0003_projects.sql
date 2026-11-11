-- Projets et inventaire (cahier des charges, sections 7 et 8).
-- Supprimer un projet supprime tout ce qui lui appartient (clés étrangères en cascade).

CREATE TABLE projects (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    name_norm     TEXT NOT NULL,
    status        TEXT NOT NULL,
    intervention  TEXT NOT NULL,
    data          TEXT NOT NULL,
    -- Compteur des repères d'exemplaires « #XXXX » déjà attribués.
    tag_seq       INTEGER NOT NULL DEFAULT 0,
    rev           INTEGER NOT NULL DEFAULT 1,
    created_by    TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE UNIQUE INDEX projects_name ON projects (name_norm);

CREATE TABLE locations (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    code        TEXT NOT NULL,
    position    INTEGER NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX locations_project ON locations (project_id, position);

CREATE TABLE containers (
    id           TEXT PRIMARY KEY,
    project_id   TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    location_id  TEXT NOT NULL REFERENCES locations (id),
    name         TEXT NOT NULL,
    name_norm    TEXT NOT NULL,
    position     INTEGER NOT NULL,
    data         TEXT NOT NULL,
    rev          INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE UNIQUE INDEX containers_name ON containers (project_id, name_norm);
CREATE INDEX containers_location ON containers (location_id);

-- Un article est dans un contenant, ou libre dans un emplacement (container_id vide).
CREATE TABLE articles (
    id            TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    location_id   TEXT NOT NULL REFERENCES locations (id),
    container_id  TEXT REFERENCES containers (id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    nature        TEXT NOT NULL,
    item_id       TEXT,
    quantity      INTEGER NOT NULL,
    data          TEXT NOT NULL DEFAULT '{}',
    rev           INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX articles_container ON articles (container_id);
CREATE INDEX articles_project ON articles (project_id);
CREATE INDEX articles_item ON articles (item_id);

CREATE TABLE instances (
    id             TEXT PRIMARY KEY,
    project_id     TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    article_id     TEXT NOT NULL REFERENCES articles (id) ON DELETE CASCADE,
    tag            TEXT NOT NULL,
    serial         TEXT,
    mac            TEXT,
    inventory_tag  TEXT,
    rev            INTEGER NOT NULL DEFAULT 1,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE UNIQUE INDEX instances_tag ON instances (project_id, tag);
CREATE INDEX instances_article ON instances (article_id);

CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    data        TEXT NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_by  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX tasks_project ON tasks (project_id, position);

-- L'historique d'un projet agrège tout ce qui lui appartient.
ALTER TABLE audit ADD COLUMN project_id TEXT;
CREATE INDEX audit_project ON audit (project_id, id);
