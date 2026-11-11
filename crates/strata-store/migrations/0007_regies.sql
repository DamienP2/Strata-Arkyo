-- Régies (section 6.2) : un ensemble ordonné de contenants.
-- Liée à un projet (project_id renseigné), une régie assemble des contenants du projet ;
-- dans la bibliothèque (project_id vide), c'est un modèle qui assemble des modèles de contenants.
CREATE TABLE regies (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    -- flight_case, car ou fixed
    kind        TEXT NOT NULL,
    service_type TEXT,
    notes       TEXT,
    -- Régie principale du projet, créée au premier contenant.
    main        INTEGER NOT NULL DEFAULT 0,
    position    INTEGER NOT NULL DEFAULT 0,
    rev         INTEGER NOT NULL DEFAULT 1,
    created_by  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX regies_project ON regies (project_id, position);
CREATE UNIQUE INDEX regies_main ON regies (project_id) WHERE main = 1;

-- Un membre est un contenant du projet, ou un modèle de contenant pour une régie de bibliothèque.
CREATE TABLE regie_members (
    regie_id      TEXT NOT NULL REFERENCES regies (id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    container_id  TEXT REFERENCES containers (id) ON DELETE CASCADE,
    template_id   TEXT REFERENCES container_templates (id) ON DELETE CASCADE,
    CHECK ((container_id IS NULL) <> (template_id IS NULL))
);
CREATE INDEX regie_members_regie ON regie_members (regie_id, position);
CREATE UNIQUE INDEX regie_members_container ON regie_members (regie_id, container_id) WHERE container_id IS NOT NULL;
CREATE INDEX regie_members_by_container ON regie_members (container_id);

-- Les projets qui ont déjà des contenants reçoivent leur régie principale.
INSERT INTO regies (id, project_id, name, kind, service_type, main, position, created_at, updated_at)
SELECT lower(hex(randomblob(16))), p.id, p.name || ' — Régie principale',
       COALESCE(json_extract(p.data, '$.regie'), 'flight_case'), json_extract(p.data, '$.service_type'), 1, 1,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM projects p
WHERE EXISTS (SELECT 1 FROM containers c WHERE c.project_id = p.id);

INSERT INTO regie_members (regie_id, position, container_id)
SELECT r.id, ROW_NUMBER() OVER (PARTITION BY r.id ORDER BY l.position, c.position), c.id
FROM regies r
JOIN containers c ON c.project_id = r.project_id
JOIN locations l ON l.id = c.location_id
WHERE r.main = 1;
