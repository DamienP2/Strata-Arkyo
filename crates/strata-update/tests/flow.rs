//! Parcours complet d'une mise à jour : paquet signé, installation, essai à blanc,
//! bascule, confirmation, retour arrière automatique et manuel.

use ed25519_dalek::SigningKey;
use std::path::Path;
use strata_update::layout::MAX_FAILED_BOOTS;
use strata_update::signing::Envelope;
use strata_update::{
    BootDecision, Layout, ReleaseManifest, TrustedKeys, UpdateError, host_arch, package,
};

fn key() -> SigningKey {
    SigningKey::generate(&mut rand_core::OsRng)
}

/// Programme factice : répond à `version --short` et réussit `selftest`.
fn fake_binary(version: &str, selftest_ok: bool) -> Vec<u8> {
    format!(
        "#!/bin/sh\nif [ \"$1\" = version ]; then echo {version}; exit 0; fi\nif [ \"$1\" = selftest ]; then echo base saine; exit {}; fi\nexit 1\n",
        if selftest_ok { 0 } else { 1 }
    )
    .into_bytes()
}

/// Simule l'installation par le paquet Debian de la version 0.1.0.
fn initial_install(root: &Path) -> Layout {
    let dir = root.join("versions/0.1.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("strata"), fake_binary("0.1.0", true)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.join("strata"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(&dir, root.join("current")).unwrap();
    Layout::new(root)
}

#[test]
fn signed_package_round_trip_and_tampering() {
    let tmp = tempfile::tempdir().unwrap();
    let k = key();
    let path = package::pack(
        &fake_binary("0.2.0", true),
        "0.2.0",
        host_arch(),
        &k,
        tmp.path(),
    )
    .unwrap();
    let ok = package::open(&path, &TrustedKeys::new(vec![k.verifying_key()])).unwrap();
    assert_eq!(ok.info.version, "0.2.0");

    // Une autre clé : refus.
    let e = package::open(&path, &TrustedKeys::new(vec![key().verifying_key()])).unwrap_err();
    assert!(matches!(e, UpdateError::BadSignature), "{e}");
    // Aucune clé intégrée : refus explicite.
    let e = package::open(&path, &TrustedKeys::default()).unwrap_err();
    assert!(matches!(e, UpdateError::NoTrustedKey));

    // Mauvaise architecture.
    let other = package::pack(b"x", "0.3.0", "sparc", &k, tmp.path()).unwrap();
    let e = package::open(&other, &TrustedKeys::new(vec![k.verifying_key()])).unwrap_err();
    assert!(matches!(e, UpdateError::WrongArch { .. }), "{e}");
}

#[test]
fn a_swapped_binary_is_refused() {
    // Description et signature authentiques, programme remplacé.
    let tmp = tempfile::tempdir().unwrap();
    let k = key();
    let real = package::pack(
        &fake_binary("0.2.0", true),
        "0.2.0",
        host_arch(),
        &k,
        &tmp.path().join("a"),
    )
    .unwrap();
    let mut entries = std::collections::HashMap::new();
    let mut ar =
        tar::Archive::new(zstd::Decoder::new(std::fs::File::open(&real).unwrap()).unwrap());
    for e in ar.entries().unwrap() {
        let mut e = e.unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut e, &mut buf).unwrap();
        entries.insert(e.path().unwrap().to_string_lossy().to_string(), buf);
    }
    entries.insert("strata".into(), b"#!/bin/sh\necho pirate\n".to_vec());
    let forged = tmp.path().join("forged.saupd");
    let mut b =
        tar::Builder::new(zstd::Encoder::new(std::fs::File::create(&forged).unwrap(), 3).unwrap());
    for name in ["package.json", "package.sig", "strata"] {
        let data = &entries[name];
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, name, data.as_slice()).unwrap();
    }
    b.into_inner().unwrap().finish().unwrap();
    let e = package::open(&forged, &TrustedKeys::new(vec![k.verifying_key()])).unwrap_err();
    assert!(matches!(e, UpdateError::HashMismatch), "{e}");
}

#[test]
fn manifests_are_signed_too() {
    let k = key();
    let m = ReleaseManifest {
        format: 1,
        channel: "stable".into(),
        version: "0.2.0".into(),
        arch: host_arch().into(),
        package: "packages/x.saupd".into(),
        sha256: "00".into(),
        size: 1,
        published_at: "2026-11-11T00:00:00Z".into(),
        notes: vec!["Correctifs".into()],
    };
    let env = m.sign(&k).unwrap();
    let keys = TrustedKeys::new(vec![k.verifying_key()]);
    assert!(keys.verify(&env).is_ok());
    let forged = Envelope {
        payload: env.payload.replace("0.2.0", "9.9.9"),
        ..env
    };
    assert!(keys.verify(&forged).is_err());
}

#[tokio::test]
async fn install_switch_confirm() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = initial_install(tmp.path());
    let k = key();
    let path = package::pack(
        &fake_binary("0.2.0", true),
        "0.2.0",
        host_arch(),
        &k,
        tmp.path(),
    )
    .unwrap();
    let pkg = package::open(&path, &TrustedKeys::new(vec![k.verifying_key()])).unwrap();
    layout.install_binary(&pkg).unwrap();
    layout.selftest("0.2.0", tmp.path()).await.unwrap();
    layout.switch_to("0.2.0").unwrap();
    assert_eq!(layout.current_version().as_deref(), Some("0.2.0"));
    assert_eq!(layout.on_boot("0.2.0").unwrap(), BootDecision::Continue);
    layout.confirm_boot("0.2.0").unwrap();
    let s = layout.state();
    assert_eq!(s.pending, None);
    assert_eq!(s.previous.as_deref(), Some("0.1.0"));
    // Les deux versions restent installées ; la précédente sert au retour manuel.
    let installed: Vec<_> = layout
        .installed()
        .into_iter()
        .map(|v| (v.version, v.active))
        .collect();
    assert_eq!(
        installed,
        [("0.2.0".to_string(), true), ("0.1.0".to_string(), false)]
    );
    assert_eq!(layout.rollback().unwrap(), "0.1.0");
    assert_eq!(layout.current_version().as_deref(), Some("0.1.0"));
}

#[tokio::test]
async fn a_failing_selftest_never_switches() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = initial_install(tmp.path());
    let k = key();
    let path = package::pack(
        &fake_binary("0.2.0", false),
        "0.2.0",
        host_arch(),
        &k,
        tmp.path(),
    )
    .unwrap();
    let pkg = package::open(&path, &TrustedKeys::new(vec![k.verifying_key()])).unwrap();
    layout.install_binary(&pkg).unwrap();
    let e = layout.selftest("0.2.0", tmp.path()).await.unwrap_err();
    assert!(matches!(e, UpdateError::SelftestFailed { .. }), "{e}");
    assert_eq!(layout.current_version().as_deref(), Some("0.1.0"));

    // Un programme qui annonce une autre version que son paquet est refusé aussi.
    let path = package::pack(
        &fake_binary("0.9.9", true),
        "0.3.0",
        host_arch(),
        &k,
        tmp.path(),
    )
    .unwrap();
    let pkg = package::open(&path, &TrustedKeys::new(vec![k.verifying_key()])).unwrap();
    layout.install_binary(&pkg).unwrap();
    assert!(layout.selftest("0.3.0", tmp.path()).await.is_err());
}

#[tokio::test]
async fn third_failed_boot_brings_back_the_previous_version() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = initial_install(tmp.path());
    let k = key();
    let path = package::pack(
        &fake_binary("0.2.0", true),
        "0.2.0",
        host_arch(),
        &k,
        tmp.path(),
    )
    .unwrap();
    let pkg = package::open(&path, &TrustedKeys::new(vec![k.verifying_key()])).unwrap();
    layout.install_binary(&pkg).unwrap();
    layout.switch_to("0.2.0").unwrap();
    // Trois démarrages qui n'arrivent jamais à la confirmation.
    for _ in 0..MAX_FAILED_BOOTS {
        assert_eq!(layout.on_boot("0.2.0").unwrap(), BootDecision::Continue);
    }
    // Au quatrième lancement, retour automatique.
    assert_eq!(
        layout.on_boot("0.2.0").unwrap(),
        BootDecision::RolledBack {
            from: "0.2.0".into(),
            to: "0.1.0".into()
        }
    );
    assert_eq!(layout.current_version().as_deref(), Some("0.1.0"));
    assert_eq!(layout.state().rolled_back_from.as_deref(), Some("0.2.0"));
    // L'ancienne version démarre normalement.
    assert_eq!(layout.on_boot("0.1.0").unwrap(), BootDecision::Continue);
}

#[test]
fn the_guard_counts_boots_of_a_version_that_crashes_early() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = initial_install(tmp.path());
    let dir = tmp.path().join("versions/0.2.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("strata"), b"binaire illisible").unwrap();
    layout.switch_to("0.2.0").unwrap();
    for _ in 0..MAX_FAILED_BOOTS {
        assert_eq!(layout.guard_boot().unwrap(), BootDecision::Continue);
    }
    assert!(matches!(
        layout.guard_boot().unwrap(),
        BootDecision::RolledBack { .. }
    ));
    assert_eq!(layout.current_version().as_deref(), Some("0.1.0"));
}
