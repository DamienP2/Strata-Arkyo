# Installer et mettre à jour Strata Arkyo sur Ubuntu

Deux choses à retenir :

- **Installer** : télécharger le paquet `.deb` de la dernière version et l'installer avec `apt`.
- **Mettre à jour** : `sudo strata upgrade`, ou le bouton **Installer** dans Administration → Mises à jour.

Les versions sont publiées automatiquement par GitHub dans les [Releases](https://github.com/DamienP2/Strata-Arkyo/releases) du dépôt. Rien à compiler sur le serveur, ni clé, ni jeton.

Les commandes sont à taper dans un terminal sur le serveur, avec un compte qui a le droit `sudo`. Le texte après `#` est un commentaire.

---

## 1. Vérifier le serveur

| Élément | Minimum |
| --- | --- |
| Système | Ubuntu Server 22.04 LTS ou 24.04 LTS, 64 bits (x86_64) |
| Mémoire | 2 Go |
| Disque libre | 2 Go, plus la place des données et des sauvegardes |
| Réseau | Accès HTTPS sortant à `github.com` et `api.github.com` (pour les mises à jour) |

```sh
lsb_release -d     # Ubuntu 22.04.x ou 24.04.x
uname -m           # x86_64
```

---

## 2. Installer

```sh
sudo apt update && sudo apt install -y curl wget
cd /tmp
# Télécharge le paquet de la dernière version publiée
curl -s https://api.github.com/repos/DamienP2/Strata-Arkyo/releases/latest \
  | grep -o 'https://[^"]*_amd64\.deb' | head -1 | xargs wget -q --show-progress
sudo apt install ./strata-arkyo_*_amd64.deb
```

Le paquet fait tout le reste : utilisateur système `strata`, programme dans `/opt/strata-arkyo/`, commande `strata`, données dans `/var/lib/strata-arkyo`, service `strata-arkyo` démarré, port 8090 ouvert si le pare-feu ufw est actif.

Message attendu : `Strata Arkyo <version> est installé et écoute sur le port 8090.`

Vérifier :

```sh
systemctl status strata-arkyo --no-pager    # active (running)
curl http://localhost:8090/api/health       # {"status":"ok","version":"…","schema":…}
```

---

## 3. Créer le premier compte et se connecter

Le premier compte est **Propriétaire** (tous les droits) :

```sh
sudo -u strata strata user add --email vous@exemple.fr --name "Prénom Nom"
```

Notez le **mot de passe temporaire** affiché. Puis :

1. `hostname -I` donne l'adresse IP du serveur.
2. Sur un poste du réseau, ouvrez `http://<adresse-ip>:8090`.
3. Connectez-vous, puis choisissez votre mot de passe définitif.

Les autres comptes se créent dans Administration → Membres.

---

## 4. Mettre à jour

### Depuis l'application

**Administration → Mises à jour → Rechercher**, puis **Installer maintenant** ou **Cette nuit** (03:30). Le serveur vérifie aussi tout seul chaque jour et signale les nouvelles versions ; c'est toujours un administrateur qui décide de l'installation.

### En ligne de commande

```sh
sudo strata upgrade            # installe la dernière version et redémarre
sudo strata upgrade --check    # indique seulement s'il y a une nouvelle version
```

Dans les deux cas, le serveur :

1. télécharge la nouvelle version depuis GitHub et vérifie son empreinte ;
2. l'essaie sur une copie de la base ;
3. fait une sauvegarde complète du site ;
4. bascule et redémarre.

Si la nouvelle version ne démarre pas, il revient seul à l'ancienne. Les données ne sont jamais effacées.

**Revenir à la version précédente** à la main : Administration → Mises à jour → **Revenir à …**, ou :

```sh
sudo -u strata strata update rollback && sudo systemctl restart strata-arkyo
```

### Serveur déjà installé avant la version 0.2.0

Les versions compilées à la main avant la 0.2.0 ne connaissent pas encore `strata upgrade`. Installez une fois la dernière version comme à l'étape 2 (les données sont conservées) :

```sh
sudo -u strata strata backup --label "avant passage en 0.2"
cd /tmp
curl -s https://api.github.com/repos/DamienP2/Strata-Arkyo/releases/latest \
  | grep -o 'https://[^"]*_amd64\.deb' | head -1 | xargs wget -q --show-progress
sudo apt install ./strata-arkyo_*_amd64.deb
strata version
```

Ensuite, `sudo strata upgrade` suffit.

### Publier une nouvelle version (côté développement)

Rien à faire à la main : quand le numéro de version de `Cargo.toml` change sur la branche `main`, GitHub teste, compile et crée la Release `vX.Y.Z` (onglet **Actions → Publier une version**, une vingtaine de minutes). Les serveurs la voient ensuite à la prochaine recherche.

### Si la mise à jour échoue

| Message | Cause | Solution |
| --- | --- | --- |
| « Déjà à jour » | Aucune version plus récente publiée | Rien à faire |
| « ne contient pas encore le fichier … (publication en cours ?) » | GitHub est en train de publier | Réessayer dans quelques minutes |
| « téléchargement impossible … » | Le serveur ne joint pas GitHub | Vérifier l'accès HTTPS sortant (proxy, pare-feu) |
| « réponse 404 » | Le dépôt est devenu privé | Ajouter un jeton en lecture seule : `STRATA_UPDATE_TOKEN=…` dans `/etc/default/strata-arkyo` |
| « l'essai à blanc … a échoué » | La nouvelle version refuse la base | Rien n'a changé sur le serveur ; signaler le message |

---

## 5. Pare-feu (conseillé)

Activez ufw **sans vous couper l'accès SSH** :

```sh
sudo ufw allow OpenSSH
sudo ufw allow 8090/tcp comment "Strata Arkyo"
sudo ufw enable
```

Pour limiter l'accès au réseau du site (exemple `192.168.1.0/24`) :

```sh
sudo ufw delete allow 8090/tcp
sudo ufw allow from 192.168.1.0/24 to any port 8090 proto tcp
```

---

## 6. Réglages (facultatif)

Dans `/etc/default/strata-arkyo` (chaque ligne y est expliquée), puis `sudo systemctl restart strata-arkyo`.

| Réglage | Rôle | Par défaut |
| --- | --- | --- |
| `STRATA_LISTEN` | Adresse et port d'écoute | `0.0.0.0:8090` |
| `STRATA_DATA_DIR` | Dossier de données | `/var/lib/strata-arkyo` |
| `STRATA_UPDATE_URL` | Source des mises à jour ; `off` pour les désactiver | les Releases GitHub du projet |
| `STRATA_SECURE_COOKIES` | `true` seulement si le site est servi en HTTPS | désactivé |
| `RUST_LOG` | Détail du journal | `info` |

---

## 7. HTTPS avec un nom de domaine (facultatif)

Placez nginx devant. Le nom doit pointer vers le serveur et le port 80 doit être joignable pour Let's Encrypt.

```sh
sudo apt install -y nginx certbot python3-certbot-nginx
sudo tee /etc/nginx/sites-available/strata-arkyo >/dev/null <<'EOF'
server {
    listen 80;
    server_name strata.exemple.fr;
    client_max_body_size 200m;

    location / {
        proxy_pass http://127.0.0.1:8090;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;      # temps réel (WebSocket)
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

Puis dans `/etc/default/strata-arkyo` : `STRATA_LISTEN=127.0.0.1:8090` et `STRATA_SECURE_COOKIES=true`, et :

```sh
sudo systemctl restart strata-arkyo
sudo ufw delete allow 8090/tcp
```

---

## Sauvegardes

Automatiques : chaque jour à 03:00 (les 7 dernières sont gardées) et avant chaque mise à jour, dans `/var/lib/strata-arkyo/backups`.

```sh
sudo -u strata strata backup --label "avant intervention"   # à la demande
sudo -u strata strata backup --list                         # liste
sudo -u strata strata restore <nom> && sudo systemctl restart strata-arkyo   # restaurer
```

Copiez régulièrement ce dossier sur une autre machine. Pour sauvegarder directement sur un autre disque, changez le dossier dans Administration → Sauvegardes, puis autorisez-le :

```sh
sudo systemctl edit strata-arkyo
# [Service]
# ReadWritePaths=/mnt/nas/strata-arkyo
sudo systemctl restart strata-arkyo
```

---

## Commandes utiles

| Besoin | Commande |
| --- | --- |
| Mettre à jour | `sudo strata upgrade` |
| Version installée | `strata version` |
| État du service | `systemctl status strata-arkyo` |
| Redémarrer | `sudo systemctl restart strata-arkyo` |
| Journal en direct | `journalctl -u strata-arkyo -f` |
| Lister les comptes | `sudo -u strata strata user list` |
| Mot de passe oublié | `sudo -u strata strata user passwd vous@exemple.fr` |
| Vérifier la base | `sudo -u strata strata selftest` |

---

## Dépannage

- **Le service ne démarre pas** : `journalctl -u strata-arkyo -n 50 --no-pager`, la cause y est écrite.
- **« Address already in use »** : le port 8090 est pris (`sudo ss -ltnp | grep 8090`) ; changez `STRATA_LISTEN`.
- **La page ne s'ouvre pas depuis un autre poste** alors que `curl http://localhost:8090/api/health` répond : pare-feu (section 5), ou `STRATA_LISTEN` réglé sur `127.0.0.1`.
- **Connexion refusée après le passage en HTTPS** : `STRATA_SECURE_COOKIES=true` seulement si l'on passe bien par `https://`.

---

## Désinstaller

```sh
sudo apt remove strata-arkyo
```

Les données restent dans `/var/lib/strata-arkyo`. Pour les effacer définitivement (après en avoir gardé une copie) : `sudo rm -rf /var/lib/strata-arkyo`.

---

## Annexe : compiler soi-même (développement)

Seulement pour travailler sur le code ; une installation normale utilise le paquet publié.

```sh
sudo apt install -y build-essential pkg-config git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env"
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt install -y nodejs
git clone https://github.com/DamienP2/Strata-Arkyo.git && cd Strata-Arkyo
(cd web && npm ci && npm run build)      # l'interface, avant le programme
cargo install cargo-deb
cargo deb -p strata                      # target/debian/strata-arkyo_<version>_amd64.deb
```

Avec moins de 4 Go de mémoire, ajoutez un fichier d'échange avant de compiler (`sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile`).
