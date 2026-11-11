-- Étagères et bibliothèque de modèles de contenants (sections 6.1 et 9.3).

-- Un équipement non rackable se pose sur une étagère : son placement pointe vers celui de l'étagère.
ALTER TABLE placements ADD COLUMN shelf_id TEXT REFERENCES placements (id) ON DELETE CASCADE;
CREATE INDEX placements_shelf ON placements (shelf_id);

-- Modèle de contenant : le contenant et tout son contenu (articles, placements, circuits),
-- en JSON, recopié tel quel dans un projet.
CREATE TABLE container_templates (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    name_norm    TEXT NOT NULL,
    -- brouillon, inactif ou actif : seuls les modèles actifs sont proposés dans les projets.
    status       TEXT NOT NULL DEFAULT 'draft',
    -- Identifiant câble à un chiffre, unique sur le site (1 à 9 puis 0).
    cable_digit  INTEGER,
    data         TEXT NOT NULL,
    rev          INTEGER NOT NULL DEFAULT 1,
    created_by   TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE UNIQUE INDEX container_templates_name ON container_templates (name_norm);
CREATE UNIQUE INDEX container_templates_digit ON container_templates (cable_digit) WHERE cable_digit IS NOT NULL;
