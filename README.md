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
| 2 | Mises à jour OTA signées avec retour arrière, paquet hors ligne, sauvegardes du site, restauration | Fait (points de restauration par projet : avec les projets, lot 4) |
| 3 | Catalogue : référentiels et pack de départ, fiches équipements, câbles, modèles de câble, adaptateurs, doublons, compatibilité câble et signal, import CSV/XLSX, export, échange entre sites, retour arrière | Fait (API ; écrans web à venir) |
| 4 à 10 | Projets, racks, synoptique, livrables, terrain, réseau, plans | À venir |

## Organisation du dépôt

| Dossier | Rôle |
| --- | --- |
| `crates/strata` | Binaire `strata` : serveur et ligne de commande d'administration |
| `web/` | Interface web (Svelte 5, TypeScript), système de design « Strates » |
| `crates/strata-api` | API REST : sessions, membres, rôles, journal, exploitation, catalogue |
| `crates/strata-domain` | Règles métier sans entrée-sortie : droits, garde-fous, validations, modèle du catalogue, rapprochement des colonnes d'import |
| `crates/strata-store` | Base SQLite, migrations additives, sessions, audit, catalogue et instantanés |
| `crates/strata-update` | Paquets signés, versions côte à côte, bascule, garde de démarrage, retour arrière |
| `crates/strata-backup` | Sauvegardes vérifiées du site, restauration au démarrage |
| `crates/strata-release` | Outil de l'éditeur : clés, paquets `.saupd`, manifestes de canal |
| `packaging/` | Service systemd, réglages, scripts du paquet Debian |
| `maquette/` | Maquettes interactives : `index.html` (v1), `v2/` (design « Strates » retenu) |

## Développer

Rust 1.85 ou plus récent, Node.js 20 ou plus récent pour l'interface web.

L'interface web (`web/`, Svelte 5 en TypeScript) se compile en fichiers statiques que le binaire `strata` embarque. Compilez-la **avant** le moteur ; sans elle, le moteur embarque une page d'attente (les tests n'en ont pas besoin).

```sh
cd web && npm ci && npm run build && cd ..   # produit web/dist, embarqué par cargo build
```

Pendant le développement de l'interface, lancez le moteur sur le port 8090, puis `npm run dev` dans `web/` : Vite sert l'interface avec rechargement à chaud et relaie `/api` vers le moteur. `npm run check` vérifie les types.

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
| `strata backup [--label …]`, `strata backup --list` | Sauvegarde complète vérifiée, ou liste des sauvegardes |
| `strata restore <nom>`, `strata restore --cancel` | Prépare une restauration, appliquée au prochain démarrage ; ou l'annule |
| `strata update status` | Versions installées, bascule en attente, dernier retour arrière |
| `strata update install <paquet.saupd>` | Installation hors ligne : vérification, essai à blanc, sauvegarde, bascule |
| `strata update rollback` | Retour à la version précédente |
| `strata migrate` | Applique les migrations |
| `strata selftest` | Vérifie la base : ouverture, migrations, intégrité |
| `strata version [--short]` | Affiche la version |

Réglages par variables d'environnement : `STRATA_DATA_DIR`, `STRATA_LISTEN`, `STRATA_SECURE_COOKIES`, `STRATA_UPDATE_URL`, `STRATA_UPDATE_COUNTDOWN`, `STRATA_INSTALL_DIR`, `RUST_LOG`.

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
| `GET /api/admin/system`, `PATCH /api/admin/settings` | État du serveur ; dossier de sauvegarde, canal, recherche automatique | Paramètres et site : admin |
| `GET /api/admin/update`, `POST /api/admin/update/check`, `POST /api/admin/update/install` (`now` ou `night`), `POST /api/admin/update/upload`, `DELETE /api/admin/update/schedule`, `POST /api/admin/update/rollback` | Mises à jour | Paramètres et site : admin |
| `GET`/`POST /api/admin/backups`, `GET /api/admin/backups/{nom}/download`, `DELETE /api/admin/backups/{nom}`, `POST /api/admin/backups/{nom}/restore`, `DELETE /api/admin/restore` | Sauvegardes et restauration | Paramètres et site : admin |

Les garde-fous de la section 3.3 du cahier des charges sont appliqués par le moteur :
- personne n'accorde un niveau supérieur au sien ;
- un non-propriétaire ne modifie pas son propre rôle ;
- seul un propriétaire touche un propriétaire ;
- il reste toujours un propriétaire actif.

## API du catalogue

Lire demande « Catalogue : lecture ». Écrire une fiche demande « Catalogue : écriture ». Modifier un référentiel demande « Paramètres et site : écriture », car il change les listes de tout le site. L'échange entre sites et le retour arrière demandent « Catalogue : admin ».

| Route | Effet |
| --- | --- |
| `GET /api/catalog/refs`, `GET /api/catalog/refs/{genre}` | Référentiels avec leur nombre d'usages (`functions`, `signals`, `connectors`, `brands`, `system-categories`, `categories`, `types`, `phases`, `dimensions`) |
| `POST /api/catalog/refs/{genre}`, `PATCH`/`DELETE /api/catalog/refs/{genre}/{id}` | Création, modification (avec `rev`), suppression. Un élément utilisé se désactive au lieu de se supprimer ; les catégories système sont protégées |
| `POST /api/catalog/refs/dimensions/restore` | Remet les préréglages de dimensions d'origine qui manquent |
| `GET /api/catalog/items?kind=&q=&active=&brand_id=` | Fiches, recherche sans accents ni casse sur le nom et la référence |
| `POST /api/catalog/items`, `GET`/`PUT /api/catalog/items/{id}`, `POST /api/catalog/items/{id}/duplicate` | Fiches (`equipment`, `cables`, `cable-models`, `adapters`) ; chaque refus nomme ses champs (`fields`), une révision dépassée répond `409 stale` |
| `POST /api/catalog/items/delete` | Suppression de 1 à 500 fiches ; une fiche encore utilisée par une autre est épargnée |
| `POST /api/catalog/duplicates`, `GET /api/catalog/compat?model_id=&signal_id=` | Doublons probables ; compatibilité d'un modèle de câble avec un signal |
| `POST /api/catalog/import/analyze?filename=`, `POST /api/catalog/import/{id}/values`, `POST /api/catalog/import/{id}/run`, `DELETE /api/catalog/import/{id}` | Import de tableurs (CSV en UTF-8 ou Windows-1252, XLSX, XLS, ODS) : colonnes rapprochées, valeurs inconnues à associer ou créer, essai à blanc, doublons ignorés, mis à jour ou créés, globalement ou ligne par ligne |
| `GET /api/catalog/export?format=csv\|xlsx&full=` | Export des équipements ; la version simple se réimporte telle quelle, `full=true` donne une ligne par groupe de ports |
| `GET`/`POST /api/catalog/exchange` | Catalogue JSON entre sites, par sections ; fusion (le plus récent l'emporte) ou remplacement |
| `GET /api/catalog/snapshots`, `POST /api/catalog/snapshots/{id}/restore` | Instantanés pris avant chaque import ou échange, et retour arrière |

## Mises à jour OTA

1. Chaque version s'installe dans `/opt/strata-arkyo/versions/<version>/` ; le lien `current` désigne celle qui tourne.
2. Le moteur lit le manifeste signé du canal (`<STRATA_UPDATE_URL>/<stable|beta>/manifest.json`). La recherche automatique, quotidienne, ne fait que signaler.
3. Sur décision de l'administrateur, immédiate ou la nuit à 03:30, il exécute dans l'ordre :
   - téléchargement du paquet, vérification de son empreinte et de sa signature Ed25519 ;
   - installation de la nouvelle version à côté de l'actuelle ;
   - essai à blanc sur une copie de la base ;
   - sauvegarde du site ;
   - bascule du lien et arrêt avec le code 75.
4. systemd relance le service sur la nouvelle version.
5. Avant chaque démarrage, `strata boot-guard` (programme du paquet Debian, que l'OTA ne remplace jamais) compte les démarrages. Au troisième échec, il rétablit l'ancienne version.

Sans Internet, le même paquet signé se dépose dans l'administration (`POST /api/admin/update/upload`) ou s'installe avec `strata update install`.

### Publier une version (éditeur)

```sh
cargo run -p strata-release -- keygen --out ~/strata-keys          # une seule fois ; garder release.key hors du dépôt
export STRATA_RELEASE_PUBKEY=$(cat ~/strata-keys/release.pub)       # clé publique intégrée au moteur
cargo deb -p strata
cargo run -p strata-release -- pack --binary target/release/strata --key ~/strata-keys/release.key --out dist
cargo run -p strata-release -- publish --package dist/strata-arkyo_<version>_x86_64.saupd \
    --channel stable --key ~/strata-keys/release.key --site site --note "…"
```

Le dossier `site/` se sert tel quel par n'importe quel serveur web, ou se copie sur une clé USB (`STRATA_UPDATE_URL=file:///media/usb/site`). Un moteur compilé sans `STRATA_RELEASE_PUBKEY` refuse toute mise à jour signée.

## Sauvegardes

La sauvegarde complète du site se fait chaque jour à 03:00 (les 7 dernières sont gardées), à la demande, et automatiquement avant chaque mise à jour.
- **Contenu :** base copiée à chaud, fichiers déposés et réglages.
- **Vérification :** chaque archive est relue après écriture.
- **Emplacement :** `/var/lib/strata-arkyo/backups` par défaut, réglable dans l'administration. Un dossier hors du dossier de données doit aussi être ajouté à `ReadWritePaths` du service (`sudo systemctl edit strata-arkyo`).
- **Échec :** une sauvegarde automatique ratée reste signalée jusqu'au succès suivant.

Une restauration est préparée (archive vérifiée, extraite), annulable, puis appliquée au démarrage suivant. Les données actuelles sont alors mises de côté dans `set-aside/<date>/`, jamais effacées. Si la restauration échoue sans pouvoir remettre les données d'origine, le moteur refuse de démarrer sur une base vide et indique le dossier à remettre en place.

## Paquet Debian

```sh
cargo install cargo-deb
(cd web && npm ci && npm run build)   # l'interface, embarquée dans le binaire
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
