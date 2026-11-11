-- Socle : rôles, comptes, sessions, journal d'audit.
-- Règle : les migrations sont additives. On ajoute des tables ou des colonnes
-- facultatives ; on ne renomme ni ne supprime jamais rien.

CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE roles (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    color            TEXT NOT NULL DEFAULT '#686D76',
    builtin          INTEGER NOT NULL DEFAULT 0,
    perm_projets     INTEGER NOT NULL DEFAULT 0,
    perm_conception  INTEGER NOT NULL DEFAULT 0,
    perm_catalogue   INTEGER NOT NULL DEFAULT 0,
    perm_membres     INTEGER NOT NULL DEFAULT 0,
    perm_parametres  INTEGER NOT NULL DEFAULT 0,
    rev              INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX roles_name ON roles (lower(name));

-- Niveaux : 0 aucun, 1 lecture, 2 écriture, 3 admin (cahier des charges, 3.2).
INSERT INTO roles (id, name, color, builtin, perm_projets, perm_conception, perm_catalogue, perm_membres, perm_parametres, created_at, updated_at) VALUES
    ('owner',        'Propriétaire',   '#0B0C0E', 1, 3, 3, 3, 3, 3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ('project_lead', 'Chef de projet', '#1F3DF0', 1, 2, 2, 2, 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ('technician',   'Technicien',     '#0C7A4B', 1, 2, 2, 1, 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ('viewer',       'Consultation',   '#686D76', 1, 1, 0, 1, 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

CREATE TABLE accounts (
    id                    TEXT PRIMARY KEY,
    email                 TEXT NOT NULL,
    username              TEXT,
    name                  TEXT NOT NULL,
    password_hash         TEXT NOT NULL,
    must_change_password  INTEGER NOT NULL DEFAULT 0,
    role_id               TEXT NOT NULL REFERENCES roles (id),
    active                INTEGER NOT NULL DEFAULT 1,
    removed               INTEGER NOT NULL DEFAULT 0,
    phone                 TEXT,
    job                   TEXT,
    lang                  TEXT NOT NULL DEFAULT 'fr',
    theme                 TEXT NOT NULL DEFAULT 'system',
    rev                   INTEGER NOT NULL DEFAULT 1,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);
CREATE UNIQUE INDEX accounts_email ON accounts (lower(email));
CREATE UNIQUE INDEX accounts_username ON accounts (username) WHERE username IS NOT NULL;
CREATE INDEX accounts_role ON accounts (role_id);

-- Le jeton de session n'est jamais stocké : seulement son empreinte SHA-256.
CREATE TABLE sessions (
    token_hash    TEXT PRIMARY KEY,
    account_id    TEXT NOT NULL REFERENCES accounts (id),
    created_at    TEXT NOT NULL,
    last_seen_at  TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    user_agent    TEXT
);
CREATE INDEX sessions_account ON sessions (account_id);

CREATE TABLE audit (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    at           TEXT NOT NULL,
    actor_id     TEXT,
    action       TEXT NOT NULL,
    object_type  TEXT NOT NULL,
    object_id    TEXT NOT NULL,
    before       TEXT,
    after        TEXT
);
CREATE INDEX audit_object ON audit (object_type, object_id);
