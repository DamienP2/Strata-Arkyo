-- Cartographie (section 11.1) : cartes, étages (niveau unique et figé), fonds de plan,
-- objets (un seul placement par contenant sur l'ensemble des cartes du projet), liens libres
-- entre points de connexion et contenants, tracés des liaisons entre contenants.
CREATE TABLE plan_maps (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    data        TEXT NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);
CREATE INDEX plan_maps_project ON plan_maps (project_id, position);

CREATE TABLE plan_floors (
    id          TEXT PRIMARY KEY,
    map_id      TEXT NOT NULL REFERENCES plan_maps (id) ON DELETE CASCADE,
    level       INTEGER NOT NULL,
    data        TEXT NOT NULL,
    UNIQUE (map_id, level)
);

CREATE TABLE plan_files (
    id            TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    content_type  TEXT NOT NULL,
    bytes         BLOB NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE plan_objects (
    id            TEXT PRIMARY KEY,
    project_id    TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    map_id        TEXT NOT NULL REFERENCES plan_maps (id) ON DELETE CASCADE,
    floor_id      TEXT NOT NULL REFERENCES plan_floors (id) ON DELETE CASCADE,
    container_id  TEXT REFERENCES containers (id) ON DELETE CASCADE,
    data          TEXT NOT NULL,
    UNIQUE (project_id, container_id)
);
CREATE INDEX plan_objects_map ON plan_objects (map_id);

CREATE TABLE plan_links (
    id          TEXT PRIMARY KEY,
    map_id      TEXT NOT NULL REFERENCES plan_maps (id) ON DELETE CASCADE,
    point_id    TEXT NOT NULL REFERENCES plan_objects (id) ON DELETE CASCADE,
    target_id   TEXT NOT NULL REFERENCES plan_objects (id) ON DELETE CASCADE,
    data        TEXT NOT NULL
);

CREATE TABLE plan_routes (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    map_id      TEXT NOT NULL REFERENCES plan_maps (id) ON DELETE CASCADE,
    waypoints   TEXT NOT NULL,
    PRIMARY KEY (project_id, key)
);
