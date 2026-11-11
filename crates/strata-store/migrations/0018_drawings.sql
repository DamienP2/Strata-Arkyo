-- Dessins et plan de feu (section 11.2) : une page par ligne, ordonnée dans sa nature ; le
-- contenu (éléments, réglages du document) est enregistré d'un bloc, avec sa révision.
CREATE TABLE drawings (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    nature      TEXT NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0,
    data        TEXT NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL
);
CREATE INDEX drawings_project ON drawings (project_id, nature, position);
