-- Recette (section 11.5). Les points se calculent en direct (liaisons, équipements posés) ;
-- on garde le verdict, l'étiquette au moment du verdict (pour un élément supprimé) et un
-- journal daté. Les points libres sont saisis.
CREATE TABLE recette_points (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    kind        TEXT NOT NULL,
    label       TEXT NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'to_test',
    comment     TEXT,
    by_account  TEXT,
    at          TEXT,
    PRIMARY KEY (project_id, key)
);

CREATE TABLE recette_log (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    at          TEXT NOT NULL,
    by_account  TEXT,
    status      TEXT,
    comment     TEXT
);
CREATE INDEX recette_log_key ON recette_log (project_id, key, at);
