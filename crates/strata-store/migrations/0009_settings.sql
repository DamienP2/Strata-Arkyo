-- Réglages mémorisés par projet (planche d'élévations…), en JSON, avec révision.
CREATE TABLE project_settings (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    data        TEXT NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (project_id, key)
);
