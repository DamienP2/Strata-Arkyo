-- Préparation et livraison (section 11.4) : saisie par ligne (la ligne elle-même se déduit
-- de l'inventaire et du câblage) et état « câblé » de chaque liaison.
CREATE TABLE prep_lines (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    status      TEXT,
    supply      TEXT,
    reserve     INTEGER NOT NULL DEFAULT 0,
    note        TEXT,
    updated_by  TEXT,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (project_id, key)
);

CREATE TABLE cable_done (
    cable_id    TEXT PRIMARY KEY REFERENCES cables (id) ON DELETE CASCADE,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    done_at     TEXT NOT NULL,
    done_by     TEXT,
    note        TEXT
);
