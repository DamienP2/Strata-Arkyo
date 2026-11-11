-- Câblage du synoptique (section 10).
CREATE TABLE cables (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    -- Extrémités : adresses de ports (voir strata_domain::synoptic::PortRef).
    a           TEXT NOT NULL,
    b           TEXT NOT NULL,
    data        TEXT NOT NULL DEFAULT '{}',
    rev         INTEGER NOT NULL DEFAULT 1,
    created_by  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX cables_project ON cables (project_id);

-- Un port ne porte qu'un câble : chaque extrémité est unique dans le projet.
CREATE TABLE cable_ends (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    port        TEXT NOT NULL,
    cable_id    TEXT NOT NULL REFERENCES cables (id) ON DELETE CASCADE,
    PRIMARY KEY (project_id, port)
);
CREATE INDEX cable_ends_cable ON cable_ends (cable_id);

-- Position des nœuds, par vue du synoptique (élément, lieu, générale).
CREATE TABLE syn_layout (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    view        TEXT NOT NULL,
    node        TEXT NOT NULL,
    x           REAL NOT NULL,
    y           REAL NOT NULL,
    PRIMARY KEY (project_id, view, node)
);
