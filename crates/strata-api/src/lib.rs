//! API REST de Strata Arkyo : socle (lot 1), exploitation (lot 2), catalogue (lot 3).
//!
//! Toutes les écritures passent par ici ; chaque route vérifie la session, puis
//! le droit du rôle sur le domaine concerné, puis les garde-fous métier.

pub mod admin;
pub mod auth;
pub mod cable_table;
pub mod cabling;
pub mod catalog;
pub mod catalog_io;
pub mod checks;
pub mod error;
pub mod live;
pub mod network;
pub mod ops;
pub mod panels;
pub mod prep;
pub mod projects;
pub mod racks;
pub mod recette;
pub mod regies;
pub mod settings;
pub mod synoptic;
pub mod templates;
pub mod web;

use crate::auth::{
    Member, Session, burn_verification, hash_password, new_token, removal_cookie, session_cookie,
    verify_password,
};
use crate::error::{ApiError, ApiResult};
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use strata_domain::{
    Domain, Level, Permissions, builtin, check_account_creation, check_deactivation,
    check_password_reset, check_role_assignment, check_role_definition, check_role_deletion,
    generate_temporary_password, validate_color, validate_email, validate_name, validate_password,
    validate_role_name, validate_username,
};
use strata_store::{Account, NewAccount, Role, Store};

/// Version du moteur. `STRATA_BUILD_VERSION` permet, à la compilation, de fabriquer
/// une version d'essai sans toucher aux fichiers du dépôt.
pub const VERSION: &str = match option_env!("STRATA_BUILD_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// En-tête exigé sur toute requête d'écriture (protection contre les requêtes intersites).
pub const CSRF_HEADER: &str = "x-strata-request";

const MEMBERS_ADMIN: &str = "Vous n'avez pas le droit de gérer les membres et les rôles.";

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    /// Cookies marqués Secure (à activer quand le site est servi en HTTPS).
    pub secure_cookies: bool,
    /// Exploitation du site (sauvegardes, mises à jour) ; absente dans les tests unitaires de l'API.
    pub ops: Option<Arc<ops::Ops>>,
    /// Diffusion en temps réel et présence.
    pub hub: Arc<live::Hub>,
}

impl AppState {
    pub fn new(store: Store, secure_cookies: bool, ops: Option<Arc<ops::Ops>>) -> AppState {
        AppState {
            store,
            secure_cookies,
            ops,
            hub: Arc::new(live::Hub::default()),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/setup", get(setup_status).post(setup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .route("/members", get(list_members).post(create_member))
        .route("/members/{id}", patch(update_member).delete(remove_member))
        .route("/members/{id}/reset-password", post(reset_password))
        .route("/roles", get(list_roles).post(create_role))
        .route("/roles/{id}", patch(update_role).delete(delete_role))
        .route("/roles/{id}/duplicate", post(duplicate_role))
        .route("/audit", get(audit))
        .nest("/admin", admin::routes())
        .nest("/catalog", catalog::routes())
        .merge(projects::routes())
        .merge(racks::routes())
        .merge(templates::routes())
        .merge(panels::routes())
        .merge(checks::routes())
        .merge(regies::routes())
        .merge(settings::routes())
        .merge(synoptic::routes())
        .merge(cabling::routes())
        .merge(cable_table::routes())
        .merge(prep::routes())
        .merge(recette::routes())
        .merge(network::routes())
        .route("/live", get(live::socket))
        .layer(middleware::from_fn(csrf));
    Router::new()
        .nest("/api", api)
        .fallback(web::serve)
        .with_state(state)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

/// Refuse toute écriture sans l'en-tête `X-Strata-Request: 1`.
async fn csrf(req: Request, next: Next) -> Response {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    let marked = req.headers().get(CSRF_HEADER).and_then(|v| v.to_str().ok()) == Some("1");
    if safe || marked {
        next.run(req).await
    } else {
        ApiError::Csrf.into_response()
    }
}

async fn health(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let schema = st.store.schema_version().await?;
    Ok(Json(
        json!({ "status": "ok", "version": VERSION, "schema": schema }),
    ))
}

// ----- Vues renvoyées -----

#[derive(Serialize)]
struct MemberView {
    #[serde(flatten)]
    account: Account,
    role_name: String,
}

async fn member_view(store: &Store, account: Account) -> ApiResult<MemberView> {
    let role_name = store
        .role(&account.role_id)
        .await?
        .map(|r| r.name)
        .unwrap_or_default();
    Ok(MemberView { account, role_name })
}

#[derive(Serialize)]
struct MeView {
    account: Account,
    role: Role,
    is_owner: bool,
    permissions: Permissions,
    must_change_password: bool,
}

async fn me_view(store: &Store, account: Account) -> ApiResult<MeView> {
    let role = store
        .role(&account.role_id)
        .await?
        .ok_or(ApiError::NotFound("Rôle"))?;
    let is_owner = role.id == builtin::OWNER;
    let permissions = if is_owner {
        Permissions::all(Level::Admin)
    } else {
        role.permissions
    };
    Ok(MeView {
        must_change_password: account.must_change_password,
        account,
        role,
        is_owner,
        permissions,
    })
}

/// Ouvre une session et pose le cookie.
async fn open_session(
    st: &AppState,
    jar: CookieJar,
    account: &Account,
    headers: &HeaderMap,
) -> ApiResult<CookieJar> {
    let (token, hash) = new_token();
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let expires = chrono::Utc::now() + chrono::Duration::days(auth::SESSION_DAYS);
    st.store
        .insert_session(&hash, &account.id, expires, ua)
        .await?;
    Ok(jar.add(session_cookie(token, st.secure_cookies)))
}

// ----- Premier administrateur -----

async fn setup_status(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({ "needed": st.store.count_accounts().await? == 0 }),
    ))
}

#[derive(Deserialize)]
struct SetupBody {
    email: String,
    name: Option<String>,
    username: Option<String>,
    password: String,
}

/// Crée le premier administrateur : seulement s'il n'existe aucun compte, et seulement depuis la machine.
async fn setup(
    State(st): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(b): Json<SetupBody>,
) -> ApiResult<(StatusCode, CookieJar, Json<MeView>)> {
    if !addr.ip().is_loopback() {
        return Err(ApiError::Forbidden(
            "Le premier administrateur se crée depuis la machine serveur, ou avec la commande « strata user add ».".into(),
        ));
    }
    if st.store.count_accounts().await? > 0 {
        return Err(ApiError::Conflict(
            "Un compte existe déjà : connectez-vous.".into(),
        ));
    }
    let email = validate_email(&b.email)?;
    let username = validate_username(b.username.as_deref())?;
    let name = validate_name(b.name.as_deref(), &email)?;
    validate_password(&b.password)?;
    let hash = hash_password(&b.password);
    let account = st
        .store
        .insert_account(NewAccount {
            email: &email,
            username: username.as_deref(),
            name: &name,
            password_hash: &hash,
            must_change_password: false,
            role_id: builtin::OWNER,
        })
        .await?;
    st.store
        .audit::<Account, _>(
            Some(&account.id),
            "create",
            "account",
            &account.id,
            None,
            Some(&account),
        )
        .await?;
    let jar = open_session(&st, jar, &account, &headers).await?;
    Ok((
        StatusCode::CREATED,
        jar,
        Json(me_view(&st.store, account).await?),
    ))
}

// ----- Connexion -----

#[derive(Deserialize)]
struct LoginBody {
    login: String,
    password: String,
}

async fn login(
    State(st): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(b): Json<LoginBody>,
) -> ApiResult<(CookieJar, Json<MeView>)> {
    // Un seul message d'échec, quelle que soit la cause.
    let Some((account, hash)) = st.store.credentials(&b.login).await? else {
        burn_verification(&b.password);
        return Err(ApiError::LoginFailed);
    };
    if !verify_password(&b.password, &hash) || !account.active || account.removed {
        return Err(ApiError::LoginFailed);
    }
    let jar = open_session(&st, jar, &account, &headers).await?;
    Ok((jar, Json(me_view(&st.store, account).await?)))
}

async fn logout(
    State(st): State<AppState>,
    s: Session,
    jar: CookieJar,
) -> ApiResult<(CookieJar, StatusCode)> {
    // Une déconnexion libère la main sur les écrans de conception.
    for l in st.store.release_session_locks(&s.token_hash).await? {
        let cid = l.view.trim_start_matches("rack:");
        st.hub.change(
            format!("project:{}", l.project_id),
            "lock",
            cid,
            "released",
            None,
            &s.account.id,
            None,
        );
    }
    st.store.delete_session(&s.token_hash).await?;
    Ok((jar.add(removal_cookie()), StatusCode::NO_CONTENT))
}

async fn me(State(st): State<AppState>, s: Session) -> ApiResult<Json<MeView>> {
    Ok(Json(me_view(&st.store, s.account).await?))
}

#[derive(Deserialize)]
struct PasswordBody {
    current: String,
    new: String,
}

/// Change son mot de passe : exige l'ancien et ferme les autres sessions.
async fn change_password(
    State(st): State<AppState>,
    s: Session,
    Json(b): Json<PasswordBody>,
) -> ApiResult<Json<MeView>> {
    let hash = st
        .store
        .password_hash(&s.account.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    if !verify_password(&b.current, &hash) {
        return Err(ApiError::Invalid(
            "Le mot de passe actuel est incorrect.".into(),
        ));
    }
    validate_password(&b.new)?;
    let account = st
        .store
        .set_password(&s.account.id, &hash_password(&b.new), false)
        .await?;
    st.store
        .delete_sessions(&account.id, Some(&s.token_hash))
        .await?;
    st.store
        .audit::<Value, Value>(
            Some(&account.id),
            "password_change",
            "account",
            &account.id,
            None,
            None,
        )
        .await?;
    Ok(Json(me_view(&st.store, account).await?))
}

// ----- Membres -----

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    removed: bool,
}

/// La liste des membres est visible de tout membre connecté.
async fn list_members(
    State(st): State<AppState>,
    Member(_): Member,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    let mut out = Vec::new();
    for a in st.store.list_accounts(q.removed).await? {
        out.push(member_view(&st.store, a).await?);
    }
    Ok(Json(json!({ "members": out })))
}

#[derive(Deserialize)]
struct CreateMemberBody {
    email: String,
    name: Option<String>,
    username: Option<String>,
    role_id: Option<String>,
}

#[derive(Serialize)]
struct CreatedMember {
    member: MemberView,
    /// Affiché une seule fois ; la personne doit le changer à sa première connexion.
    temporary_password: String,
    reactivated: bool,
}

async fn create_member(
    State(st): State<AppState>,
    Member(s): Member,
    Json(b): Json<CreateMemberBody>,
) -> ApiResult<(StatusCode, Json<CreatedMember>)> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let email = validate_email(&b.email)?;
    let username = validate_username(b.username.as_deref())?;
    let name = validate_name(b.name.as_deref(), &email)?;
    let role_id = b.role_id.unwrap_or_else(|| builtin::TECHNICIAN.into());
    let role = st
        .store
        .role(&role_id)
        .await?
        .ok_or(ApiError::NotFound("Rôle"))?;
    check_account_creation(&s.actor, &role.info())?;

    let password = generate_temporary_password();
    let hash = hash_password(&password);
    let (account, reactivated) = match st.store.account_by_email(&email).await? {
        // Recréer un compte retiré le réactive : nouveau mot de passe, rôle mis à jour, sessions fermées.
        Some(old) if old.removed => {
            let a = st
                .store
                .reactivate_account(&old.id, &name, username.as_deref(), &role.id, &hash)
                .await?;
            st.store.delete_sessions(&a.id, None).await?;
            st.store
                .audit(
                    Some(&s.account.id),
                    "reactivate",
                    "account",
                    &a.id,
                    Some(&old),
                    Some(&a),
                )
                .await?;
            (a, true)
        }
        Some(_) => {
            return Err(ApiError::Conflict(
                "Un compte utilise déjà cet e-mail.".into(),
            ));
        }
        None => {
            let a = st
                .store
                .insert_account(NewAccount {
                    email: &email,
                    username: username.as_deref(),
                    name: &name,
                    password_hash: &hash,
                    must_change_password: true,
                    role_id: &role.id,
                })
                .await?;
            st.store
                .audit::<Account, _>(
                    Some(&s.account.id),
                    "create",
                    "account",
                    &a.id,
                    None,
                    Some(&a),
                )
                .await?;
            (a, false)
        }
    };
    let member = member_view(&st.store, account).await?;
    let status = if reactivated {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        status,
        Json(CreatedMember {
            member,
            temporary_password: password,
            reactivated,
        }),
    ))
}

async fn target_account(store: &Store, id: &str) -> ApiResult<(Account, Role)> {
    let account = store
        .account(id)
        .await?
        .ok_or(ApiError::NotFound("Membre"))?;
    let role = store
        .role(&account.role_id)
        .await?
        .ok_or(ApiError::NotFound("Rôle"))?;
    Ok((account, role))
}

#[derive(Deserialize)]
struct UpdateMemberBody {
    role_id: Option<String>,
    active: Option<bool>,
}

async fn update_member(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
    Json(b): Json<UpdateMemberBody>,
) -> ApiResult<Json<MemberView>> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let (before, mut role) = target_account(&st.store, &id).await?;
    if before.removed {
        return Err(ApiError::Invalid(
            "Ce membre a été retiré : recréez son compte pour le réactiver.".into(),
        ));
    }
    let mut account = before.clone();

    if let Some(new_role_id) = b.role_id.filter(|r| *r != account.role_id) {
        let new_role = st
            .store
            .role(&new_role_id)
            .await?
            .ok_or(ApiError::NotFound("Rôle"))?;
        let owners = st.store.count_active_owners().await?;
        check_role_assignment(
            &s.actor,
            &account.id,
            &role.info(),
            &new_role.info(),
            owners,
        )?;
        account = st.store.set_account_role(&account.id, &new_role.id).await?;
        role = new_role;
    }

    match b.active {
        Some(false) if account.active => {
            let owners = st.store.count_active_owners().await?;
            check_deactivation(&s.actor, &account.id, &role.info(), owners)?;
            account = st.store.set_account_active(&account.id, false).await?;
            // La désactivation ferme ses sessions.
            st.store.delete_sessions(&account.id, None).await?;
        }
        Some(true) if !account.active => {
            if role.id == builtin::OWNER && !s.actor.is_owner() {
                return Err(strata_domain::Refusal::OwnerOnly.into());
            }
            account = st.store.set_account_active(&account.id, true).await?;
        }
        _ => {}
    }

    if account != before {
        st.store
            .audit(
                Some(&s.account.id),
                "update",
                "account",
                &account.id,
                Some(&before),
                Some(&account),
            )
            .await?;
    }
    Ok(Json(member_view(&st.store, account).await?))
}

/// Retirer un membre le désactive sans effacer son historique.
async fn remove_member(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let (before, role) = target_account(&st.store, &id).await?;
    if before.removed {
        return Ok(StatusCode::NO_CONTENT);
    }
    let owners = st.store.count_active_owners().await?;
    check_deactivation(&s.actor, &before.id, &role.info(), owners)?;
    let after = st.store.mark_account_removed(&before.id).await?;
    st.store.delete_sessions(&after.id, None).await?;
    st.store
        .audit(
            Some(&s.account.id),
            "remove",
            "account",
            &after.id,
            Some(&before),
            Some(&after),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reset_password(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let (account, role) = target_account(&st.store, &id).await?;
    if account.removed {
        return Err(ApiError::Invalid(
            "Ce membre a été retiré : recréez son compte pour le réactiver.".into(),
        ));
    }
    check_password_reset(&s.actor, &role.info())?;
    let password = generate_temporary_password();
    st.store
        .set_password(&account.id, &hash_password(&password), true)
        .await?;
    st.store.delete_sessions(&account.id, None).await?;
    st.store
        .audit::<Value, Value>(
            Some(&s.account.id),
            "password_reset",
            "account",
            &account.id,
            None,
            None,
        )
        .await?;
    Ok(Json(json!({ "temporary_password": password })))
}

// ----- Rôles -----

async fn list_roles(State(st): State<AppState>, Member(_): Member) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "roles": st.store.list_roles().await? })))
}

#[derive(Deserialize)]
struct RoleBody {
    name: Option<String>,
    color: Option<String>,
    permissions: Option<Permissions>,
}

async fn create_role(
    State(st): State<AppState>,
    Member(s): Member,
    Json(b): Json<RoleBody>,
) -> ApiResult<(StatusCode, Json<Role>)> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let name = validate_role_name(b.name.as_deref().unwrap_or(""))?;
    let color = validate_color(b.color.as_deref().unwrap_or("#686D76"))?;
    let perms = b.permissions.unwrap_or_default();
    check_role_definition(&s.actor, None, &perms)?;
    if st.store.role_name_taken(&name, None).await? {
        return Err(ApiError::Conflict("Un rôle porte déjà ce nom.".into()));
    }
    let role = st.store.insert_role(&name, &color, &perms).await?;
    st.store
        .audit::<Role, _>(
            Some(&s.account.id),
            "create",
            "role",
            &role.id,
            None,
            Some(&role),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(role)))
}

async fn update_role(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
    Json(b): Json<RoleBody>,
) -> ApiResult<Json<Role>> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let before = st
        .store
        .role(&id)
        .await?
        .ok_or(ApiError::NotFound("Rôle"))?;
    let perms = b.permissions.unwrap_or(before.permissions);
    check_role_definition(&s.actor, Some(&before.info()), &perms)?;
    let name = match b.name {
        Some(n) => validate_role_name(&n)?,
        None => before.name.clone(),
    };
    let color = match b.color {
        Some(c) => validate_color(&c)?,
        None => before.color.clone(),
    };
    if st.store.role_name_taken(&name, Some(&id)).await? {
        return Err(ApiError::Conflict("Un rôle porte déjà ce nom.".into()));
    }
    let after = st.store.update_role(&id, &name, &color, &perms).await?;
    st.store
        .audit(
            Some(&s.account.id),
            "update",
            "role",
            &id,
            Some(&before),
            Some(&after),
        )
        .await?;
    Ok(Json(after))
}

/// Duplique un rôle : « {nom} (copie) », avec un suffixe si le nom est pris.
async fn duplicate_role(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Role>)> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let source = st
        .store
        .role(&id)
        .await?
        .ok_or(ApiError::NotFound("Rôle"))?;
    check_role_definition(&s.actor, None, &source.permissions)?;
    let base: String = source.name.chars().take(48).collect();
    let mut name = format!("{base} (copie)");
    let mut n = 2;
    while st.store.role_name_taken(&name, None).await? {
        name = format!("{base} (copie {n})");
        n += 1;
    }
    let role = st
        .store
        .insert_role(&name, &source.color, &source.permissions)
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "duplicate",
            "role",
            &role.id,
            Some(&source),
            Some(&role),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(role)))
}

/// Supprime un rôle personnalisé ; ses membres passent Technicien.
async fn delete_role(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    let role = st
        .store
        .role(&id)
        .await?
        .ok_or(ApiError::NotFound("Rôle"))?;
    check_role_deletion(&s.actor, &role.info())?;
    let moved = st.store.delete_role(&id).await?;
    st.store
        .audit::<Role, Role>(
            Some(&s.account.id),
            "delete",
            "role",
            &id,
            Some(&role),
            None,
        )
        .await?;
    Ok(Json(json!({ "moved_members": moved })))
}

// ----- Journal -----

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<i64>,
}

async fn audit(
    State(st): State<AppState>,
    Member(s): Member,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Membres, Level::Admin, MEMBERS_ADMIN)?;
    Ok(Json(
        json!({ "entries": st.store.audit_entries(q.limit.unwrap_or(150)).await? }),
    ))
}
