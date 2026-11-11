-- Annotations du synoptique (section 10.4) : zones et étiquettes, par vue.
CREATE TABLE syn_annotations (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    view        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    data        TEXT NOT NULL,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX syn_annotations_view ON syn_annotations (project_id, view);
