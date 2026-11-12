-- Points de restauration de projet (section 13.2). Sans clé étrangère vers le projet : un
-- point survit à la suppression du projet et permet de le retrouver.
CREATE TABLE project_points (
    id            TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL,
    project_name  TEXT NOT NULL,
    label         TEXT,
    auto          INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    created_by    TEXT,
    size          INTEGER NOT NULL,
    data          BLOB NOT NULL
);
CREATE INDEX project_points_project ON project_points (project_id, created_at);
