-- Liens entre projets : deux ports externes (câbles vers « x: ») de deux projets différents
-- sont déclarés comme une seule liaison. Supprimer l'un des câbles défait le lien.
CREATE TABLE project_links (
    id          TEXT PRIMARY KEY,
    a_project   TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    a_cable     TEXT NOT NULL UNIQUE REFERENCES cables (id) ON DELETE CASCADE,
    b_project   TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    b_cable     TEXT NOT NULL UNIQUE REFERENCES cables (id) ON DELETE CASCADE,
    note        TEXT,
    created_at  TEXT NOT NULL,
    created_by  TEXT
);
CREATE INDEX project_links_a ON project_links (a_project);
CREATE INDEX project_links_b ON project_links (b_project);
