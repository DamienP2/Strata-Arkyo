# Strata Arkyo

Conception, câblage, préparation et recette d'installations audiovisuelles : régies broadcast, cars, plateaux, salles et prestations événementielles.

Strata Arkyo est un seul programme Rust installé sur un serveur Ubuntu du site. Il sert l'interface web, porte les règles métier, diffuse le temps réel et se met à jour par OTA signée. Les données vivent dans un seul dossier, autour d'une base SQLite.

- Cahier des charges et revue d'ingénieur : document « Cahier des charges fonctionnel — Strata Arkyo »
- Architecture technique : document « Architecture technique — Strata Arkyo »
- Maquette interactive : [`maquette/`](maquette/)

## État d'avancement

| Lot | Contenu | État |
| --- | --- | --- |
| 1 | Socle : binaire, ligne de commande, service systemd, paquet .deb, SQLite et migrations, comptes, rôles, journal d'audit | Fait |
| 2 | Mises à jour OTA, sauvegardes, points de restauration | À venir |
| 3 à 10 | Catalogue, projets, racks, synoptique, livrables, terrain, réseau, plans | À venir |

## Organisation du dépôt

| Dossier | Rôle |
| --- | --- |
| `crates/strata` | Binaire `strata` : serveur et ligne de commande d'administration |
| `crates/strata-api` | API REST : sessions, membres, rôles, journal |
| `crates/strata-domain` | Règles métier sans entrée-sortie : droits, garde-fous, validations |
| `crates/strata-store` | Base SQLite, migrations additives, sessions, audit |
| `packaging/` | Service systemd, réglages, scripts du paquet Debian |
| `maquette/` | Maquette interactive de l'interface |

## Développer

Rust 1.85 ou plus récent.

```sh
cargo test --workspace          # tests unitaires et de bout en bout
cargo clippy --workspace --all-targets
cargo fmt --all
```

Lancer le moteur en local avec un dossier de données jetable :

```sh
export STRATA_DATA_DIR=./data
cargo run -p strata -- user add --email vous@exemple.fr   # premier compte : Propriétaire
cargo run -p strata -- serve --listen 127.0.0.1:8090
curl localhost:8090/api/health
```

Le mot de passe affiché par `user add` est temporaire : il doit être changé à la première connexion (`POST /api/me/password`).

## Ligne de commande

| Commande | Effet |
| --- | --- |
| `strata serve` | Lance le serveur (port 8090 par défaut) |
| `strata user add --email … [--name …] [--username …] [--role …]` | Crée un compte et affiche son mot de passe temporaire. Le premier compte est Propriétaire |
| `strata user list [--all]` | Liste les comptes, retirés compris avec `--all` |
| `strata user passwd <login>` | Nouveau mot de passe temporaire, sessions fermées |
| `strata user remove <login>` | Retire un compte en gardant son historique |
| `strata migrate` | Applique les migrations |
| `strata selftest` | Vérifie la base : ouverture, migrations, intégrité |
| `strata version [--short]` | Affiche la version |

Réglages par variables d'environnement : `STRATA_DATA_DIR`, `STRATA_LISTEN`, `STRATA_SECURE_COOKIES`, `RUST_LOG`.

## API du socle

Toute requête d'écriture doit porter l'en-tête `X-Strata-Request: 1`. La session est un cookie `HttpOnly`, `SameSite=Strict`, valable 30 jours.

| Route | Effet | Droit requis |
| --- | --- | --- |
| `GET /api/health` | État du moteur et version du schéma | Aucun |
| `GET /api/setup`, `POST /api/setup` | Premier administrateur, seulement depuis la machine et s'il n'existe aucun compte | Aucun |
| `POST /api/auth/login`, `POST /api/auth/logout` | Connexion par e-mail ou identifiant, déconnexion | Aucun, puis session |
| `GET /api/me`, `POST /api/me/password` | Son compte, changement de mot de passe | Session |
| `GET /api/members` | Liste des membres (`?removed=true` pour les retirés) | Membre connecté |
| `POST /api/members`, `PATCH`/`DELETE /api/members/{id}`, `POST /api/members/{id}/reset-password` | Gestion des comptes | Membres et rôles : admin |
| `GET /api/roles`, `POST /api/roles`, `PATCH`/`DELETE /api/roles/{id}`, `POST /api/roles/{id}/duplicate` | Rôles et permissions | Lecture : membre ; écriture : Membres et rôles admin |
| `GET /api/audit` | Journal d'audit | Membres et rôles : admin |

Les garde-fous de la section 3.3 du cahier des charges sont appliqués par le moteur :
- personne n'accorde un niveau supérieur au sien ;
- un non-propriétaire ne modifie pas son propre rôle ;
- seul un propriétaire touche un propriétaire ;
- il reste toujours un propriétaire actif.

## Paquet Debian

```sh
cargo install cargo-deb
cargo deb -p strata            # produit target/debian/strata-arkyo_<version>_amd64.deb
sudo apt install ./target/debian/strata-arkyo_*_amd64.deb
sudo -u strata strata user add --email vous@exemple.fr
```

Le paquet installe :
- le programme dans `/opt/strata-arkyo/versions/<version>/`, désigné par le lien `/opt/strata-arkyo/current` ;
- le service `strata-arkyo` (systemd), lancé sous l'utilisateur `strata` ;
- les réglages dans `/etc/default/strata-arkyo` ;
- les données dans `/var/lib/strata-arkyo`.

Il ouvre aussi le port 8090 si ufw est actif. La désinstallation laisse les données en place.
