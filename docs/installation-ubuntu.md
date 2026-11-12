# Installer Strata Arkyo sur Ubuntu, pas à pas

Ce guide installe Strata Arkyo sur un serveur Ubuntu, depuis le code source jusqu'à la première connexion dans le navigateur. Comptez 30 à 45 minutes, la compilation en prend la plus grande part.

Les commandes sont à taper dans un terminal sur le serveur, connecté avec un compte qui a le droit `sudo`. Le texte après `#` est un commentaire : inutile de le taper.

---

## Étape 0 — Vérifier le serveur

| Élément | Minimum | Conseillé |
| --- | --- | --- |
| Système | Ubuntu Server 22.04 LTS ou 24.04 LTS, 64 bits (x86_64) | 24.04 LTS |
| Mémoire | 2 Go (4 Go pendant la compilation, voir l'étape 1) | 4 Go ou plus |
| Disque libre | 10 Go (dont 6 Go pour la compilation) | 20 Go, plus la place des sauvegardes |
| Réseau | Accès à Internet pendant l'installation | Adresse IP fixe sur le réseau du site |

Vérifiez la version et l'architecture :

```sh
lsb_release -d     # attendu : Ubuntu 22.04.x LTS ou 24.04.x LTS
uname -m           # attendu : x86_64
free -h            # mémoire disponible
df -h /            # place libre sur le disque
```

> **Compiler ailleurs ?** Vous pouvez faire les étapes 2 à 6 sur un autre poste Ubuntu, puis copier sur le serveur le seul fichier `.deb` obtenu (étape 6) et reprendre à l'étape 7. Le serveur n'a alors besoin ni de Rust, ni de Node.js.

---

## Étape 1 — Mettre le système à jour

```sh
sudo apt update
sudo apt upgrade -y
sudo apt install -y build-essential pkg-config git curl ca-certificates
```

Si le serveur a **moins de 4 Go de mémoire**, ajoutez un fichier d'échange pour que la compilation n'échoue pas :

```sh
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

---

## Étape 2 — Installer Rust

Strata Arkyo demande Rust 1.85 ou plus récent. Les paquets Ubuntu sont trop anciens : on passe par `rustup`.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version    # attendu : rustc 1.85.0 ou plus récent
```

Si `rustc --version` affiche une version plus ancienne : `rustup update stable`.

---

## Étape 3 — Installer Node.js

L'interface web demande Node.js 20 ou plus récent. Celui d'Ubuntu est trop ancien : on installe la version 22 depuis NodeSource.

```sh
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
sudo apt install -y nodejs
node --version     # attendu : v22.x
npm --version
```

---

## Étape 4 — Récupérer le code source

Le dépôt est privé : GitHub demande de vous identifier. Utilisez votre identifiant GitHub et, comme mot de passe, un **jeton d'accès personnel** (GitHub → Settings → Developer settings → Personal access tokens, droit de lecture sur le dépôt).

```sh
cd ~
git clone https://github.com/DamienP2/Strata-Arkyo.git
cd Strata-Arkyo
```

Pour installer une branche ou une version précise plutôt que la branche principale :

```sh
git checkout <nom-de-branche-ou-de-version>
```

---

## Étape 5 — Compiler l'interface web

L'interface est compilée en fichiers statiques que le programme embarque. **Elle doit être compilée avant le programme**, sinon celui-ci n'affiche qu'une page d'attente.

```sh
cd ~/Strata-Arkyo/web
npm ci
npm run build
cd ..
ls web/dist/index.html    # le fichier doit exister
```

---

## Étape 6 — Compiler le programme et fabriquer le paquet Debian

> **Conseillé :** si vous voulez faire les mises à jour suivantes depuis l'application (OTA), créez d'abord votre clé de publication (section **Mises à jour OTA**, étape A1) et faites `export STRATA_RELEASE_PUBKEY=$(cat ~/strata-keys/release.pub)` avant la commande ci-dessous. Sinon, vous pourrez le faire plus tard.

```sh
cargo install cargo-deb          # outil de fabrication du paquet, une seule fois
cargo deb -p strata              # compile en mode release, puis fabrique le paquet
ls target/debian/                # attendu : strata-arkyo_<version>_amd64.deb
```

La première compilation dure de 10 à 30 minutes selon le serveur.

---

## Étape 7 — Installer le paquet

```sh
sudo apt install ./target/debian/strata-arkyo_*_amd64.deb
```

(Si vous avez copié le `.deb` depuis un autre poste, remplacez le chemin par celui du fichier, en gardant le `./` devant.)

Le paquet fait tout seul le reste :

- il crée l'utilisateur système `strata` ;
- il installe le programme dans `/opt/strata-arkyo/` et la commande `strata` ;
- il crée le dossier de données `/var/lib/strata-arkyo` ;
- il installe, active et démarre le service `strata-arkyo` ;
- il ouvre le port 8090 si le pare-feu ufw est actif.

Le message attendu à la fin : `Strata Arkyo <version> est installé et écoute sur le port 8090.`

---

## Étape 8 — Vérifier que le service tourne

```sh
systemctl status strata-arkyo --no-pager    # attendu : active (running)
curl http://localhost:8090/api/health       # attendu : {"status":"ok","version":"…","schema":…}
```

Si le service n'est pas `active (running)`, voyez la section **Dépannage** plus bas.

---

## Étape 9 — Créer le premier compte

Le premier compte créé est **Propriétaire** : il a tous les droits.

```sh
sudo -u strata strata user add --email vous@exemple.fr --name "Prénom Nom"
```

La commande affiche un **mot de passe temporaire** : notez-le, il ne sera plus affiché.

---

## Étape 10 — Se connecter

1. Cherchez l'adresse IP du serveur :
   ```sh
   hostname -I
   ```
2. Sur un poste du même réseau, ouvrez un navigateur à l'adresse `http://<adresse-ip>:8090`.
3. Connectez-vous avec l'e-mail et le mot de passe temporaire de l'étape 9.
4. Choisissez votre mot de passe définitif quand l'application le demande.

Les autres comptes se créent ensuite depuis l'application (Administration → Membres), ou avec `strata user add`.

---

## Étape 11 — Pare-feu (conseillé)

Si ufw n'est pas encore actif, activez-le **sans vous couper l'accès SSH** :

```sh
sudo ufw allow OpenSSH
sudo ufw allow 8090/tcp comment "Strata Arkyo"
sudo ufw enable
sudo ufw status
```

Pour limiter l'accès au réseau du site (exemple : `192.168.1.0/24`) :

```sh
sudo ufw delete allow 8090/tcp
sudo ufw allow from 192.168.1.0/24 to any port 8090 proto tcp
```

---

## Étape 12 — Réglages (facultatif)

Les réglages sont dans `/etc/default/strata-arkyo`. Chaque ligne y est expliquée.

```sh
sudo nano /etc/default/strata-arkyo
sudo systemctl restart strata-arkyo     # après chaque modification
```

| Réglage | Rôle | Valeur par défaut |
| --- | --- | --- |
| `STRATA_LISTEN` | Adresse et port d'écoute | `0.0.0.0:8090` |
| `STRATA_DATA_DIR` | Dossier de données | `/var/lib/strata-arkyo` |
| `STRATA_UPDATE_URL` | Serveur des mises à jour en ligne | aucun (mise à jour hors ligne seulement) |
| `STRATA_SECURE_COOKIES` | À mettre à `true` seulement si le site est servi en HTTPS | désactivé |
| `RUST_LOG` | Niveau de détail du journal | `info` |

---

## Étape 13 — HTTPS avec un nom de domaine (facultatif)

Pour servir l'application en `https://strata.exemple.fr`, placez nginx devant. Le nom doit pointer vers le serveur ; Let's Encrypt doit pouvoir le joindre sur le port 80.

```sh
sudo apt install -y nginx certbot python3-certbot-nginx
sudo tee /etc/nginx/sites-available/strata-arkyo >/dev/null <<'EOF'
server {
    listen 80;
    server_name strata.exemple.fr;
    client_max_body_size 200m;          # dépôt de fichiers et de paquets de mise à jour

    location / {
        proxy_pass http://127.0.0.1:8090;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        # Temps réel (WebSocket)
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 1h;
    }
}
EOF
sudo ln -s /etc/nginx/sites-available/strata-arkyo /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
sudo ufw allow 'Nginx Full'
sudo certbot --nginx -d strata.exemple.fr
```

Puis, dans `/etc/default/strata-arkyo` :

```sh
STRATA_LISTEN=127.0.0.1:8090      # n'écoute plus que nginx
STRATA_SECURE_COOKIES=true
```

```sh
sudo systemctl restart strata-arkyo
sudo ufw delete allow 8090/tcp      # le port 8090 n'a plus à être ouvert
```

---

## Sauvegardes

Rien à faire pour qu'elles fonctionnent : une sauvegarde complète est faite chaque jour à 03:00 (les 7 dernières sont gardées) et avant chaque mise à jour, dans `/var/lib/strata-arkyo/backups`.

```sh
sudo -u strata strata backup --label "avant intervention"   # sauvegarde à la demande
sudo -u strata strata backup --list                         # liste des sauvegardes
```

**Conseil :** copiez régulièrement ce dossier sur une autre machine ou un NAS. Pour sauvegarder directement sur un autre disque, changez le dossier dans Administration → Sauvegardes, puis autorisez-le pour le service :

```sh
sudo systemctl edit strata-arkyo
# ajoutez ces deux lignes, enregistrez et quittez :
# [Service]
# ReadWritePaths=/mnt/nas/strata-arkyo
sudo systemctl restart strata-arkyo
```

---

## Mettre à jour depuis le code source

```sh
cd ~/Strata-Arkyo
git pull
(cd web && npm ci && npm run build)
cargo deb -p strata
sudo -u strata strata backup --label "avant mise à jour"
sudo apt install ./target/debian/strata-arkyo_*_amd64.deb
systemctl status strata-arkyo --no-pager
```

La base de données est migrée automatiquement au démarrage. Pour mettre à jour sans recompiler sur le serveur, voyez la section **Mises à jour OTA** ci-dessous.

---

## Mises à jour depuis GitHub (recommandé)

GitHub compile, signe et publie chaque version dans les **Releases** du dépôt. Le serveur y lit les nouvelles versions et l'administrateur les installe en un clic depuis l'application. Plus rien à compiler sur le serveur.

### 1. Une seule fois : la clé de publication sur GitHub

Sur votre poste (Ubuntu, avec Rust, voir étape 2) :

```sh
cd ~/Strata-Arkyo
cargo run --release -p strata-release -- keygen --out ~/strata-keys
cat ~/strata-keys/release.key     # clé privée
cat ~/strata-keys/release.pub     # clé publique
```

Sur GitHub, dans le dépôt : **Settings → Secrets and variables → Actions → New repository secret**, créez deux secrets :

| Nom | Valeur |
| --- | --- |
| `STRATA_RELEASE_KEY` | le contenu de `release.key` |
| `STRATA_RELEASE_PUBKEY` | le contenu de `release.pub` |

Rangez ensuite `release.key` en lieu sûr hors du poste (coffre, clé USB). Avec elle, n'importe qui peut signer une mise à jour acceptée par vos serveurs ; sans elle, il faudrait réinstaller les serveurs avec une nouvelle clé.

### 2. Publier une version

1. Sur GitHub, onglet **Actions** → **Publier une version** → **Run workflow**, branche `main`.
2. Comptez 15 à 25 minutes : tests, compilation, signature.
3. La version apparaît dans **Releases** (`v0.2.0`, puis `v0.2.1`…) avec trois fichiers : le paquet `.saupd`, `manifest.json` et le `.deb`.

Le numéro publié est celui de `Cargo.toml`. Une version déjà publiée est refusée : il faut d'abord monter le numéro. Un numéro avec tiret (ex. `0.3.0-beta.1`) part en préversion, sur le canal Bêta.

### 3. Une seule fois sur le serveur : installer une version signée et brancher GitHub

**a. Installer le `.deb` de la Release.** Il contient votre clé publique, ce qui permet ensuite les mises à jour OTA. Téléchargez-le depuis la page Releases (bouton du fichier `strata-arkyo_0.2.0_amd64.deb`), copiez-le sur le serveur, puis :

```sh
sudo -u strata strata backup --label "avant passage aux mises à jour GitHub"
sudo apt install ./strata-arkyo_0.2.0_amd64.deb
strata version                       # Strata Arkyo 0.2.0
```

**b. Créer un jeton GitHub en lecture seule** (le dépôt est privé) : GitHub → photo de profil → **Settings → Developer settings → Personal access tokens → Fine-grained tokens → Generate new token**.
- *Repository access* : **Only select repositories** → `Strata-Arkyo`.
- *Permissions* → *Repository permissions* → **Contents : Read-only**. Rien d'autre.
- Une durée de validité d'un an, à noter dans l'agenda pour le renouveler.

**c. Indiquer la source et le jeton au serveur :**

```sh
sudo nano /etc/default/strata-arkyo
```

Ajoutez (ou décommentez) ces deux lignes :

```sh
STRATA_UPDATE_URL=github:DamienP2/Strata-Arkyo
STRATA_UPDATE_TOKEN=github_pat_xxxxxxxxxxxxxxxx
```

```sh
sudo chmod 600 /etc/default/strata-arkyo     # le jeton n'est lisible que par root
sudo systemctl restart strata-arkyo
```

Le serveur doit pouvoir joindre `api.github.com` et `objects.githubusercontent.com` en HTTPS (port 443 sortant).

### 4. Ensuite, à chaque nouvelle version

1. **Publier** : Actions → Publier une version → Run workflow (étape 2).
2. **Installer** : dans l'application, **Administration → Mises à jour → Rechercher**. La nouvelle version s'affiche ; choisissez **Installer maintenant** ou **Cette nuit (03:30)**.

Le serveur vérifie aussi tout seul une fois par jour et signale la nouvelle version ; l'installation reste toujours décidée par un administrateur. Avant de basculer, il vérifie la signature, fait un essai sur une copie de la base et une sauvegarde. Si la nouvelle version ne démarre pas, il revient seul à l'ancienne.

### Si ça ne marche pas

| Message dans Administration → Mises à jour | Cause | Solution |
| --- | --- | --- |
| « Aucune source de mise à jour configurée » | `STRATA_UPDATE_URL` absent | Étape 3c |
| « réponse 404 … dépôt privé sans jeton » | Jeton absent, expiré ou sans accès au dépôt | Refaire le jeton (3b), puis 3c |
| « réponse 401 … jeton refusé » | Jeton invalide ou mal copié | Recopier le jeton, redémarrer le service |
| « aucune clé de publication dans cette version » | La version installée n'a pas été compilée par le workflow | Installer le `.deb` de la Release (3a) |
| « signature invalide » | Les secrets GitHub ne correspondent pas à la clé de la version installée | Vérifier les deux secrets (étape 1), republier |
| « ne contient pas le fichier manifest.json » | Release créée à la main, ou workflow encore en cours | Attendre la fin du workflow ; publier uniquement par le workflow |
| Le workflow échoue à « Clés de publication présentes » | Secrets absents | Étape 1 |

---

## Mises à jour OTA sans GitHub (paquet déposé à la main)

Une mise à jour OTA se fait depuis **Administration → Mises à jour**, sans recompiler sur le serveur : on dépose un paquet signé `.saupd` (ou le serveur le télécharge lui-même). Le moteur vérifie la signature, essaie la nouvelle version sur une copie de la base, fait une sauvegarde, bascule, puis redémarre. Si la nouvelle version ne démarre pas, il revient seul à l'ancienne.

Deux conditions, vérifiées par le moteur :

1. **La version installée connaît votre clé de publication.** Le moteur n'accepte que les paquets signés par une clé qu'il a reçue à la compilation (`STRATA_RELEASE_PUBKEY`). Un moteur compilé sans clé, comme avec les étapes 5 et 6 ci-dessus, refuse toute mise à jour OTA.
2. **Le paquet porte un numéro de version plus élevé** que la version installée (`strata version`).

### A. Une seule fois : créer la clé et passer le serveur en OTA

À faire sur la machine où vous compilez (le serveur ou un autre poste Ubuntu).

**A1. Créer la paire de clés.**

```sh
cd ~/Strata-Arkyo
cargo run --release -p strata-release -- keygen --out ~/strata-keys
```

Cela crée `~/strata-keys/release.key` (clé **privée**) et `~/strata-keys/release.pub` (clé publique).

> **La clé privée ne se perd pas et ne se partage pas.** Copiez `release.key` sur un support sauvegardé (coffre, clé USB rangée). Ne la mettez jamais dans le dépôt Git. Sans elle, plus aucune mise à jour OTA n'est possible : il faudrait repasser par l'étape A3 avec une nouvelle clé. Avec elle, n'importe qui pourrait signer une mise à jour acceptée par vos serveurs.

**A2. Compiler en intégrant la clé publique.**

```sh
cd ~/Strata-Arkyo
git pull
(cd web && npm ci && npm run build)
export STRATA_RELEASE_PUBKEY=$(cat ~/strata-keys/release.pub)
cargo deb -p strata
```

`STRATA_RELEASE_PUBKEY` doit être défini **à chaque compilation** : sinon la version produite refusera les mises à jour suivantes. Pour ne pas l'oublier, ajoutez la ligne `export …` à la fin de `~/.bashrc`.

**A3. Installer ce paquet une dernière fois « à la main ».**

C'est la dernière fois que le `.deb` sert : la version installée connaît désormais votre clé.

```sh
sudo -u strata strata backup --label "avant passage en OTA"
sudo apt install ./target/debian/strata-arkyo_*_amd64.deb
strata version                                   # attendu : Strata Arkyo 0.2.0
systemctl status strata-arkyo --no-pager
```

Les données sont conservées et la base est migrée au démarrage.

### B. À chaque nouvelle version : fabriquer et déposer le paquet signé

**B1. Monter le numéro de version** dans `Cargo.toml` (ligne `version = "…"` de `[workspace.package]`), par exemple de `0.2.0` à `0.2.1`. C'est déjà fait dans le code quand une nouvelle version vous est livrée.

**B2. Compiler et signer.**

```sh
cd ~/Strata-Arkyo
git pull
(cd web && npm ci && npm run build)
export STRATA_RELEASE_PUBKEY=$(cat ~/strata-keys/release.pub)
cargo build --release -p strata
target/release/strata version                    # vérifiez le nouveau numéro
cargo run --release -p strata-release -- pack \
    --binary target/release/strata --key ~/strata-keys/release.key --out dist
ls dist/                                          # strata-arkyo_<version>_x86_64.saupd
```

**B3. Installer depuis l'application.**

1. Connectez-vous avec un compte qui a le droit **Paramètres et site : admin**.
2. **Administration → Mises à jour → Installer un paquet hors ligne**, puis choisissez le fichier `.saupd`.
3. Le moteur vérifie, essaie, sauvegarde, puis annonce le redémarrage aux utilisateurs connectés (60 s par défaut) et bascule.
4. Après le redémarrage, la page indique la nouvelle version.

Le même paquet s'installe aussi en ligne de commande sur le serveur :

```sh
sudo -u strata strata update install /chemin/vers/strata-arkyo_0.2.1_x86_64.saupd
sudo systemctl restart strata-arkyo
```

**Revenir en arrière :** Administration → Mises à jour → bouton « Revenir à <version précédente> », ou `sudo -u strata strata update rollback` puis `sudo systemctl restart strata-arkyo`.

### C. Facultatif : mises à jour proposées automatiquement

Au lieu de déposer le fichier, vous pouvez publier les versions sur un serveur web (ou une clé USB) que le moteur consulte chaque jour :

```sh
cargo run --release -p strata-release -- publish \
    --package dist/strata-arkyo_0.2.1_x86_64.saupd \
    --channel stable --key ~/strata-keys/release.key --site ~/strata-site --note "Corrections"
```

Servez le dossier `~/strata-site` en HTTPS, puis indiquez son adresse au moteur dans `/etc/default/strata-arkyo` :

```sh
STRATA_UPDATE_URL=https://updates.exemple.fr/strata-arkyo
```

```sh
sudo systemctl restart strata-arkyo
```

L'administration signale alors chaque nouvelle version ; l'installation reste toujours décidée par un administrateur, tout de suite ou la nuit à 03:30.

### Messages d'erreur fréquents

| Message | Cause | Solution |
| --- | --- | --- |
| « aucune clé de publication dans cette version » | La version installée a été compilée sans clé | Faire les étapes A2 et A3 |
| « signature invalide » | Paquet signé avec une autre clé que celle intégrée au moteur | Signer avec la bonne `release.key`, ou refaire A2 et A3 |
| « La version … n'est pas plus récente que la version installée » | Numéro de version identique ou plus ancien | Monter le numéro (B1) puis refaire B2 |
| Bouton « Installer un paquet » grisé | Installation qui ne vient pas du paquet Debian (lancée à la main) | Installer avec le `.deb` (étape 7) |
| Le dépôt du fichier échoue derrière nginx | Taille d'envoi limitée | `client_max_body_size 200m;` (étape 13) |

---

## Commandes utiles

| Besoin | Commande |
| --- | --- |
| État du service | `systemctl status strata-arkyo` |
| Redémarrer | `sudo systemctl restart strata-arkyo` |
| Journal en direct | `journalctl -u strata-arkyo -f` |
| Journal du dernier démarrage | `journalctl -u strata-arkyo -b --no-pager` |
| Version installée | `strata version` |
| Lister les comptes | `sudo -u strata strata user list` |
| Mot de passe oublié | `sudo -u strata strata user passwd vous@exemple.fr` |
| Vérifier la base | `sudo -u strata strata selftest` |

---

## Dépannage

**Le service ne démarre pas.** Lisez la fin du journal, la cause y est écrite :

```sh
journalctl -u strata-arkyo -n 50 --no-pager
```

**« Address already in use ».** Un autre programme occupe le port 8090. Trouvez-le avec `sudo ss -ltnp | grep 8090`, ou changez `STRATA_LISTEN` (étape 12).

**La page ne s'ouvre pas depuis un autre poste, mais `curl http://localhost:8090/api/health` répond.** C'est le pare-feu (étape 11), ou `STRATA_LISTEN` est réglé sur `127.0.0.1`.

**La page dit « cette version a été compilée sans l'interface web ».** L'interface n'a pas été compilée avant le programme. Refaites les étapes 5, 6 et 7.

**La compilation s'arrête avec « killed » ou « signal: 9 ».** La mémoire manque : ajoutez le fichier d'échange de l'étape 1 et relancez `cargo deb -p strata`.

**« No space left on device » pendant la compilation.** Libérez de la place, par exemple en supprimant les fichiers intermédiaires : `cargo clean` dans `~/Strata-Arkyo` (le `.deb` est à refaire ensuite).

**Connexion refusée après le passage en HTTPS.** Vérifiez que `STRATA_SECURE_COOKIES=true` n'est activé **que** si vous passez bien par `https://`.

---

## Désinstaller

```sh
sudo apt remove strata-arkyo
```

Les données restent dans `/var/lib/strata-arkyo`. Pour les effacer **définitivement** (après en avoir gardé une copie) :

```sh
sudo rm -rf /var/lib/strata-arkyo
```
