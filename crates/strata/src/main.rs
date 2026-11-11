//! `strata` : le moteur de Strata Arkyo et sa ligne de commande d'administration.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use strata_api::auth::hash_password;
use strata_api::{AppState, VERSION, router};
use strata_domain::{
    builtin, generate_temporary_password, validate_email, validate_name, validate_username,
};
use strata_store::{NewAccount, Store};

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
    },
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
        } => serve(&cli.data_dir, listen, secure_cookies).await,
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

async fn serve(data_dir: &Path, listen: SocketAddr, secure_cookies: bool) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=warn,sqlx=warn".into()),
        )
        .without_time() // journald horodate déjà chaque ligne
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

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

    let app = router(AppState {
        store: store.clone(),
        secure_cookies,
    });
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .with_context(|| format!("impossible d'écouter sur {listen}"))?;
    tracing::info!(%listen, "Strata Arkyo à l'écoute");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    store.close().await;
    tracing::info!("arrêt propre");
    Ok(())
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
