-- Contrôles de construction en atelier (section 9.7) : une ligne par élément coché d'un rack.
-- La liste elle-même se déduit de l'inventaire ; seule la coche est gardée ici.
CREATE TABLE build_checks (
    container_id  TEXT NOT NULL REFERENCES containers (id) ON DELETE CASCADE,
    key           TEXT NOT NULL,
    checked_by    TEXT,
    checked_at    TEXT NOT NULL,
    PRIMARY KEY (container_id, key)
);
