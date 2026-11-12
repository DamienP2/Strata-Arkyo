//! `strata` : le moteur de Strata Arkyo et sa ligne de commande d'administration.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use strata_api::auth::hash_password;
use strata_api::ops::{EXIT_RESTART, META_BACKUP_DIR, Ops};
use strata_api::{AppState, VERSION, router};
use strata_backup::{Backups, Kind};
use strata_domain::{
    builtin, generate_temporary_password, validate_email, validate_name, validate_username,
};
use strata_store::{NewAccount, Store};
use strata_update::{BootDecision, Layout, TrustedKeys};

#[derive(Parser)]
#[command(
    name = "strata",
    version,
    about = "Strata Arkyo : moteur et administration du site"
)]
struct Cli {
    /// Dossier de données (base, fichiers, sauvegardes, journaux).
    #[arg(
        long,
        env = "STRATA_DATA_DIR",
        default_value = "/var/lib/strata-arkyo",
        global = true
    )]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lance le serveur (utilisé par le service systemd).
    Serve {
        /// Adresse et port d'écoute.
        #[arg(long, env = "STRATA_LISTEN", default_value = "0.0.0.0:8090")]
        listen: SocketAddr,
        /// Marque les cookies Secure (site servi en HTTPS).
        #[arg(long, env = "STRATA_SECURE_COOKIES")]
        secure_cookies: bool,
        /// Adresse du serveur de publication des mises à jour (https://… ou file://…).
        #[arg(long, env = "STRATA_UPDATE_URL")]
        update_url: Option<String>,
        /// Secondes entre l'annonce d'un redémarrage et la bascule.
        #[arg(long, env = "STRATA_UPDATE_COUNTDOWN", default_value_t = 60)]
        countdown: u64,
    },
    /// Sauvegarde complète du site (base, fichiers, réglages).
    Backup {
        /// Libellé de la sauvegarde.
        #[arg(long)]
        label: Option<String>,
        /// Liste les sauvegardes au lieu d'en créer une.
        #[arg(long)]
        list: bool,
    },
    /// Prépare la restauration d'une sauvegarde ; elle s'applique au prochain démarrage du service.
    Restore {
        /// Nom de la sauvegarde (voir « strata backup --list »).
        name: Option<String>,
        /// Annule la restauration en attente.
        #[arg(long)]
        cancel: bool,
    },
    /// Garde de démarrage (ExecStartPre du service) : compte les démarrages après une
    /// bascule et rétablit l'ancienne version au troisième échec.
    #[command(hide = true)]
    BootGuard,
    /// Met à jour vers la dernière version publiée sur GitHub : `sudo strata upgrade`.
    Upgrade {
        /// Indique seulement si une nouvelle version existe.
        #[arg(long)]
        check: bool,
        /// Ne redémarre pas le service (usage interne).
        #[arg(long, hide = true)]
        no_restart: bool,
    },
    /// Mises à jour avancées : état, installation d'un fichier, retour arrière.
    #[command(subcommand)]
    Update(UpdateCommand),
    /// Applique les migrations de la base sans lancer le serveur.
    Migrate,
    /// Vérifie que la base s'ouvre, migre et passe le contrôle d'intégrité.
    Selftest,
    /// Gère les comptes depuis la machine.
    #[command(subcommand)]
    User(UserCommand),
    /// Affiche la version.
    Version {
        /// Seulement le numéro, pour les scripts.
        #[arg(long)]
        short: bool,
    },
}

#[derive(Subcommand)]
enum UpdateCommand {
    /// Versions installées et état de la dernière bascule.
    Status,
    /// Installe un paquet signé (.saupd) : vérification, essai à blanc, sauvegarde, bascule.
    Install { package: PathBuf },
    /// Revient à la version précédente.
    Rollback,
}

#[derive(Subcommand)]
enum UserCommand {
    /// Ajoute un compte et affiche son mot de passe temporaire.
    Add {
        #[arg(long)]
        email: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        username: Option<String>,
        /// owner, project_lead, technician, viewer ou l'identifiant d'un rôle personnalisé.
        /// Par défaut : owner pour le premier compte, technician ensuite.
        #[arg(long)]
        role: Option<String>,
    },
    /// Liste les comptes.
    List {
        /// Inclut les comptes retirés.
        #[arg(long)]
        all: bool,
    },
    /// Donne un nouveau mot de passe temporaire et ferme les sessions du compte.
    Passwd { login: String },
    /// Retire un compte : il est désactivé, son historique est gardé.
    Remove { login: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Version { short } => {
            if short {
                println!("{VERSION}");
            } else {
                println!("Strata Arkyo {VERSION}");
            }
            Ok(())
        }
        Command::Serve {
            listen,
            secure_cookies,
            update_url,
            countdown,
        } => {
            let code = serve(
                &cli.data_dir,
                listen,
                secure_cookies,
                update_url,
                Duration::from_secs(countdown),
            )
            .await?;
            std::process::exit(code);
        }
        Command::Backup { label, list } => backup(&cli.data_dir, label, list).await,
        Command::Restore { name, cancel } => restore(&cli.data_dir, name, cancel).await,
        Command::Update(cmd) => update(&cli.data_dir, cmd).await,
        Command::Upgrade { check, no_restart } => {
            let code = upgrade(&cli.data_dir, check, no_restart).await?;
            std::process::exit(code);
        }
        Command::BootGuard => {
            let layout = Layout::from_env();
            if let BootDecision::RolledBack { from, to } = layout.guard_boot()? {
                eprintln!(
                    "La version {from} n'a pas démarré trois fois de suite : retour à la version {to}."
                );
            }
            Ok(())
        }
        Command::Migrate => {
            let store = open(&cli.data_dir).await?;
            println!("Base à jour : schéma {}.", store.schema_version().await?);
            Ok(())
        }
        Command::Selftest => selftest(&cli.data_dir).await,
        Command::User(cmd) => user(&cli.data_dir, cmd).await,
    }
}

fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("strata.db")
}

async fn open(data_dir: &Path) -> Result<Store> {
    std::fs::create_dir_all(data_dir).with_context(|| {
        format!(
            "impossible de créer le dossier de données {}",
            data_dir.display()
        )
    })?;
    Store::open(&db_path(data_dir)).await.with_context(|| {
        format!(
            "impossible d'ouvrir la base {}",
            db_path(data_dir).display()
        )
    })
}

/// Lance le service. Renvoie le code de sortie : 0 à l'arrêt, 75 pour un redémarrage demandé.
async fn serve(
    data_dir: &Path,
    listen: SocketAddr,
    secure_cookies: bool,
    update_url: Option<String>,
    countdown: Duration,
) -> Result<i32> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=warn,sqlx=warn".into()),
        )
        .without_time() // journald horodate déjà chaque ligne
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

    // 1. Le compteur de démarrages est tenu par « strata boot-guard » (ExecStartPre du service),
    //    lancé avec le programme du paquet Debian : il compte même une version qui plante ici.
    let layout = Layout::from_env();

    // 2. Restauration en attente, avant d'ouvrir la base.
    if let Some(aside) = strata_backup::failed_restore(data_dir) {
        bail!(
            "une restauration a échoué et les données d'origine sont dans {} : remettez strata.db et files/ \
             dans {}, supprimez {}/restore-failed.json, puis relancez le service",
            aside.display(),
            data_dir.display(),
            data_dir.display()
        );
    }
    std::fs::create_dir_all(data_dir)?;
    match strata_backup::apply_pending_restore(data_dir) {
        Ok(Some(aside)) => {
            tracing::warn!(set_aside = %aside.display(), "sauvegarde restaurée ; données précédentes mises de côté")
        }
        Ok(None) => {}
        Err(e @ strata_backup::BackupError::Fatal(_)) => bail!("{e}"),
        Err(e) => {
            tracing::error!(error = %e, "restauration annulée : les données d'origine sont en place")
        }
    }

    // 3. Base et exploitation.
    let store = open(data_dir).await?;
    let purged = store.purge_expired_sessions().await?;
    tracing::info!(
        version = VERSION,
        schema = store.schema_version().await?,
        purged,
        "base ouverte"
    );
    if store.count_accounts().await? == 0 {
        tracing::warn!(
            "aucun compte : créez le premier administrateur avec « strata user add --email … » \
             ou depuis http://localhost:{} sur cette machine",
            listen.port()
        );
    }
    let keys = TrustedKeys::embedded();
    if keys.is_empty() {
        tracing::warn!(
            "aucune clé de publication intégrée : les mises à jour signées sont désactivées"
        );
    }
    let (ops, restart_rx) = Ops::new(
        data_dir.to_path_buf(),
        layout.clone(),
        keys,
        update_url,
        countdown,
        VERSION,
    );
    ops.spawn_background(store.clone());

    // Purge des sessions expirées, toutes les heures.
    let purge_store = store.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = purge_store.purge_expired_sessions().await {
                tracing::warn!(error = %e, "purge des sessions");
            }
        }
    });

    // Points de restauration automatiques des projets modifiés (section 13.2).
    let points_store = store.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            strata_api::points::auto_points(&points_store).await;
        }
    });

    let state = AppState::new(store.clone(), secure_cookies, Some(ops));
    // Les présences sans battement expirent.
    let hub = state.hub.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            t.tick().await;
            hub.sweep();
        }
    });
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .with_context(|| format!("impossible d'écouter sur {listen}"))?;
    tracing::info!(%listen, "Strata Arkyo à l'écoute");

    // 4. Le service est prêt : la version est confirmée.
    if layout.is_managed() {
        if let Err(e) = layout.confirm_boot(VERSION) {
            tracing::warn!(error = %e, "confirmation du démarrage");
        }
    }

    let mut watch_restart = restart_rx.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        tokio::select! {
            _ = shutdown_signal() => {},
            _ = watch_restart.changed() => {},
        }
    })
    .await?;
    store.close().await;
    if *restart_rx.borrow() {
        tracing::info!("arrêt pour redémarrage");
        Ok(EXIT_RESTART)
    } else {
        tracing::info!("arrêt propre");
        Ok(0)
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}

async fn selftest(data_dir: &Path) -> Result<()> {
    let store = open(data_dir).await?;
    let schema = store.schema_version().await?;
    if !store.integrity_ok().await? {
        bail!(
            "contrôle d'intégrité en échec sur {}",
            db_path(data_dir).display()
        );
    }
    let roles = store.list_roles().await?.len();
    store.close().await;
    println!("Strata Arkyo {VERSION} : base saine, schéma {schema}, {roles} rôles.");
    Ok(())
}

async fn backups_of(store: &Store, data_dir: &Path) -> Result<Backups> {
    let dir = store
        .meta(META_BACKUP_DIR)
        .await?
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("backups"));
    Ok(Backups::new(data_dir, dir))
}

async fn backup(data_dir: &Path, label: Option<String>, list: bool) -> Result<()> {
    let store = open(data_dir).await?;
    let backups = backups_of(&store, data_dir).await?;
    if list {
        for b in backups.list() {
            let kind = match b.manifest.kind {
                Kind::Auto => "automatique",
                Kind::Manual => "manuelle",
                Kind::PreUpdate => "avant mise à jour",
            };
            println!(
                "{:<40} {:>10} Ko  {:<18} {}",
                b.name,
                b.size / 1024,
                kind,
                b.manifest.label.as_deref().unwrap_or("")
            );
        }
    } else {
        let info = backups.create(&store, Kind::Manual, label, VERSION).await?;
        println!(
            "Sauvegarde vérifiée : {} ({} Ko).",
            backups.dir().join(&info.name).display(),
            info.size / 1024
        );
    }
    store.close().await;
    Ok(())
}

async fn restore(data_dir: &Path, name: Option<String>, cancel: bool) -> Result<()> {
    if cancel {
        strata_backup::cancel_restore(data_dir)?;
        println!("Restauration annulée.");
        return Ok(());
    }
    let Some(name) = name else {
        bail!("indiquez le nom d'une sauvegarde (strata backup --list)")
    };
    let store = open(data_dir).await?;
    let manifest = backups_of(&store, data_dir).await?.stage_restore(&name)?;
    store.close().await;
    println!("Sauvegarde du {} vérifiée et prête.", manifest.created_at);
    println!("Elle s'appliquera au prochain démarrage : sudo systemctl restart strata-arkyo");
    println!(
        "Les données actuelles seront mises de côté, jamais effacées. Pour annuler : strata restore --cancel"
    );
    Ok(())
}

/// Lit une valeur de /etc/default/strata-arkyo (la ligne de commande ne passe pas par systemd).
fn site_setting(key: &str) -> Option<String> {
    if let Ok(v) = std::env::var(key) {
        return Some(v);
    }
    let text = std::fs::read_to_string("/etc/default/strata-arkyo").ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.trim().split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .map(|l| l.split_whitespace().nth(1) == Some("0"))
        })
        .unwrap_or(false)
}

/// `strata upgrade` : codes de sortie 0 (installée), 3 (déjà à jour), 1 (erreur).
async fn upgrade(data_dir: &Path, check: bool, no_restart: bool) -> Result<i32> {
    let source = site_setting("STRATA_UPDATE_URL");
    let token = site_setting("STRATA_UPDATE_TOKEN");
    if is_root() {
        // Les fichiers appartiennent à l'utilisateur strata : on travaille sous son nom,
        // puis on redémarre le service.
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new("runuser");
        cmd.args(["-u", "strata", "--"])
            .arg(&exe)
            .arg("--data-dir")
            .arg(data_dir)
            .arg("upgrade")
            .arg("--no-restart");
        if check {
            cmd.arg("--check");
        }
        for (k, v) in [
            ("STRATA_UPDATE_URL", &source),
            ("STRATA_UPDATE_TOKEN", &token),
        ] {
            if let Some(v) = v {
                cmd.env(k, v);
            }
        }
        let code = cmd
            .status()
            .context("lancement sous l'utilisateur strata")?
            .code()
            .unwrap_or(1);
        if code == 0 && !check {
            println!("Redémarrage du service…");
            let ok = std::process::Command::new("systemctl")
                .args(["restart", "strata-arkyo"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                println!("Strata Arkyo redémarré. Vérifiez avec : strata version");
            } else {
                println!("Redémarrez : sudo systemctl restart strata-arkyo");
            }
        }
        return Ok(code);
    }
    if let Some(t) = &token {
        // SAFETY : avant tout autre fil d'exécution qui lirait l'environnement.
        unsafe { std::env::set_var("STRATA_UPDATE_TOKEN", t) };
    }
    let layout = Layout::from_env();
    let store = open(data_dir).await?;
    let (ops, _rx) = Ops::new(
        data_dir.to_path_buf(),
        layout,
        TrustedKeys::embedded(),
        source,
        Duration::ZERO,
        VERSION,
    );
    println!("Version installée : {VERSION}. Recherche d'une nouvelle version…");
    let found = ops
        .check(&store)
        .await
        .map_err(|e| anyhow::anyhow!("{}", ops_message(e)))?;
    let Some(m) = found else {
        println!("Déjà à jour.");
        store.close().await;
        return Ok(3);
    };
    println!("Nouvelle version disponible : {}.", m.version);
    for n in &m.notes {
        println!("  · {n}");
    }
    if check {
        store.close().await;
        return Ok(0);
    }
    println!("Téléchargement, vérification et essai sur une copie de la base…");
    let v = ops
        .prepare_online(&store)
        .await
        .map_err(|e| anyhow::anyhow!("{}", ops_message(e)))?;
    println!("Sauvegarde du site et bascule…");
    ops.finish(&store, &v)
        .await
        .map_err(|e| anyhow::anyhow!("{}", ops_message(e)))?;
    store.close().await;
    println!("Version {v} installée.");
    if !no_restart {
        println!(
            "Redémarrez : sudo systemctl restart strata-arkyo (ou lancez directement sudo strata upgrade)"
        );
    }
    Ok(0)
}

fn ops_message(e: strata_api::ops::OpsError) -> String {
    use strata_api::ops::OpsError::*;
    match e {
        Conflict(m) | Invalid(m) | Unavailable(m) | Internal(m) => m,
    }
}

async fn update(data_dir: &Path, cmd: UpdateCommand) -> Result<()> {
    let layout = Layout::from_env();
    match cmd {
        UpdateCommand::Status => {
            println!("Version en cours : {VERSION}");
            if !layout.is_managed() {
                println!("Installation hors paquet : mises à jour désactivées.");
                return Ok(());
            }
            for v in layout.installed() {
                println!("  {} {}", if v.active { "●" } else { " " }, v.version);
            }
            let s = layout.state();
            if let Some(p) = s.pending {
                println!(
                    "Bascule vers {p} en attente de confirmation ({} démarrage(s)).",
                    s.attempts
                );
            }
            if let Some(from) = s.rolled_back_from {
                println!(
                    "Dernier retour arrière depuis {from}, le {}.",
                    s.rolled_back_at.unwrap_or_default()
                );
            }
        }
        UpdateCommand::Install { package } => {
            let store = open(data_dir).await?;
            let (ops, _rx) = Ops::new(
                data_dir.to_path_buf(),
                layout,
                TrustedKeys::embedded(),
                None,
                Duration::ZERO,
                VERSION,
            );
            let v = ops
                .prepare_file(&store, &package)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            ops.finish(&store, &v)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            store.close().await;
            println!(
                "Version {v} installée et désignée. Redémarrez : sudo systemctl restart strata-arkyo"
            );
        }
        UpdateCommand::Rollback => {
            let target = layout.rollback()?;
            println!(
                "Retour à la version {target}. Redémarrez : sudo systemctl restart strata-arkyo"
            );
        }
    }
    Ok(())
}

async fn user(data_dir: &Path, cmd: UserCommand) -> Result<()> {
    let store = open(data_dir).await?;
    match cmd {
        UserCommand::Add {
            email,
            name,
            username,
            role,
        } => {
            let email = validate_email(&email)?;
            let username = validate_username(username.as_deref())?;
            let name = validate_name(name.as_deref(), &email)?;
            let role_id = match role {
                Some(r) => r,
                None if store.count_accounts().await? == 0 => builtin::OWNER.to_string(),
                None => builtin::TECHNICIAN.to_string(),
            };
            let role = store
                .role(&role_id)
                .await?
                .with_context(|| format!("rôle inconnu : {role_id}"))?;
            let password = generate_temporary_password();
            let account = store
                .insert_account(NewAccount {
                    email: &email,
                    username: username.as_deref(),
                    name: &name,
                    password_hash: &hash_password(&password),
                    must_change_password: true,
                    role_id: &role.id,
                })
                .await?;
            store
                .audit::<strata_store::Account, _>(
                    None,
                    "create",
                    "account",
                    &account.id,
                    None,
                    Some(&account),
                )
                .await?;
            println!(
                "Compte créé : {} ({}), rôle {}.",
                account.name, account.email, role.name
            );
            println!("Mot de passe temporaire, affiché une seule fois : {password}");
            println!("Il devra être changé à la première connexion.");
        }
        UserCommand::List { all } => {
            let roles = store.list_roles().await?;
            for a in store.list_accounts(all).await? {
                let role = roles
                    .iter()
                    .find(|r| r.id == a.role_id)
                    .map(|r| r.name.as_str())
                    .unwrap_or("?");
                let state = if a.removed {
                    "retiré"
                } else if !a.active {
                    "désactivé"
                } else {
                    "actif"
                };
                println!(
                    "{:<32} {:<16} {:<16} {}",
                    a.email,
                    a.username.as_deref().unwrap_or("—"),
                    role,
                    state
                );
            }
        }
        UserCommand::Passwd { login } => {
            let (account, _) = store
                .credentials(&login)
                .await?
                .with_context(|| format!("compte inconnu : {login}"))?;
            let password = generate_temporary_password();
            store
                .set_password(&account.id, &hash_password(&password), true)
                .await?;
            store.delete_sessions(&account.id, None).await?;
            store
                .audit::<(), ()>(None, "password_reset", "account", &account.id, None, None)
                .await?;
            println!(
                "Nouveau mot de passe temporaire pour {} : {password}",
                account.email
            );
        }
        UserCommand::Remove { login } => {
            let (account, _) = store
                .credentials(&login)
                .await?
                .with_context(|| format!("compte inconnu : {login}"))?;
            if account.role_id == builtin::OWNER
                && account.active
                && store.count_active_owners().await? <= 1
            {
                bail!("Il doit rester au moins un propriétaire actif.");
            }
            let after = store.mark_account_removed(&account.id).await?;
            store.delete_sessions(&account.id, None).await?;
            store
                .audit(
                    None,
                    "remove",
                    "account",
                    &account.id,
                    Some(&account),
                    Some(&after),
                )
                .await?;
            println!(
                "Compte retiré : {}. Son historique est gardé.",
                account.email
            );
        }
    }
    store.close().await;
    Ok(())
}
