-- Compteurs de numérotation des câbles (section 10.7) : un par élément (ou le projet pour
-- les liaisons entre éléments), préfixe et sens. Un numéro attribué n'est jamais recyclé.
CREATE TABLE cable_counters (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    scope       TEXT NOT NULL,
    prefix      TEXT NOT NULL,
    dir         INTEGER NOT NULL,
    last        INTEGER NOT NULL,
    PRIMARY KEY (project_id, scope, prefix, dir)
);
