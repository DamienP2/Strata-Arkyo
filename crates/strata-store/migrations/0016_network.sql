-- Réseau (section 11.3) : VLAN du projet, réglages des ports réseau (par référence de port)
-- et groupes multicast des flux IP (clé instance:flux:sortie). Une saisie manuelle prime sur
-- l'attribution automatique.
CREATE TABLE net_vlans (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    vid         INTEGER NOT NULL,
    data        TEXT NOT NULL,
    UNIQUE (project_id, vid)
);

CREATE TABLE net_ports (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    port_ref    TEXT NOT NULL,
    data        TEXT NOT NULL,
    PRIMARY KEY (project_id, port_ref)
);

CREATE TABLE net_flows (
    project_id  TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    address     TEXT NOT NULL,
    udp         INTEGER NOT NULL,
    manual      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (project_id, key)
);
