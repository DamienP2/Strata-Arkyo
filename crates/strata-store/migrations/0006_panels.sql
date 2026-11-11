-- Bibliothèque de panels de brassage (section 6.3).
-- Un panel posé dans un projet en est une copie, gardée dans l'article : la bibliothèque
-- n'est jamais modifiée depuis un projet.
CREATE TABLE panels (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    name_norm   TEXT NOT NULL,
    data        TEXT NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_by  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE UNIQUE INDEX panels_name ON panels (name_norm);
