-- Faisceaux (section 10.4) : groupe nommé de câbles tirés ensemble ; un câble appartient à
-- un seul faisceau (champ harness_id du câble).
CREATE TABLE harnesses (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX harnesses_project ON harnesses (project_id);
