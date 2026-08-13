use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use woc_update::{
    apply_update, fetch_url, plan_fetch, url_parent, verify_manifest, verifying_key_from_hex,
    ArtifactStore, DirStore, HttpStore, InstallState, Manifest, UpdateError,
};

struct Args {
    prefix: PathBuf,
    manifest: String,
    store: Option<PathBuf>,
    once: bool,
    no_exec: bool,
    pubkey: Option<String>,
    already_copied: bool,
    _apply_from: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: woc-updater --prefix DIR --manifest PATH|URL [--store DIR] [--once] \\
       [--no-exec] [--pubkey HEX] [--already-copied] [--apply-from DIR]"
    );
    process::exit(2);
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn parse_args() -> Args {
    let raw: Vec<String> = env::args().collect();
    if raw.len() < 2 {
        usage();
    }

    let apply_from = arg_value(&raw, "--apply-from").map(PathBuf::from);
    let prefix = apply_from
        .clone()
        .or_else(|| arg_value(&raw, "--prefix").map(PathBuf::from));

    let manifest = arg_value(&raw, "--manifest");
    let store = arg_value(&raw, "--store").map(PathBuf::from);

    let (Some(prefix), Some(manifest)) = (prefix, manifest) else {
        usage();
    };

    Args {
        prefix,
        manifest,
        store,
        once: has_flag(&raw, "--once"),
        no_exec: has_flag(&raw, "--no-exec"),
        pubkey: arg_value(&raw, "--pubkey"),
        already_copied: has_flag(&raw, "--already-copied") || apply_from.is_some(),
        _apply_from: apply_from,
    }
}

fn default_pubkey_hex() -> &'static str {
    match option_env!("WOC_UPDATE_PUBKEY") {
        Some(s) if !s.is_empty() => s,
        _ => "1111111111111111111111111111111111111111111111111111111111111111",
    }
}

fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn load_manifest(path_or_url: &str) -> Result<Manifest, UpdateError> {
    let bytes = if is_http_url(path_or_url) {
        fetch_url(path_or_url)?
    } else {
        fs::read(path_or_url)?
    };
    Ok(serde_json::from_slice(&bytes)?)
}

fn read_install_state(prefix: &Path) -> Result<InstallState, UpdateError> {
    let path = prefix.join("install.json");
    if !path.exists() {
        return Ok(InstallState {
            rewrite_version: "0.0.0".into(),
            target: String::new(),
        });
    }
    let json = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}

fn exe_inside_prefix(exe: &Path, prefix: &Path) -> bool {
    let Ok(exe) = fs::canonicalize(exe) else {
        return false;
    };
    let Ok(prefix) = fs::canonicalize(prefix) else {
        return false;
    };
    exe.starts_with(&prefix)
}

#[cfg(unix)]
fn maybe_self_update(args: &Args, manifest: &Manifest) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;

    if args.already_copied {
        return Ok(());
    }

    let local = read_install_state(&args.prefix)?;
    if matches!(
        plan_fetch(&local, manifest)?,
        woc_update::FetchPlan::Nothing
    ) {
        return Ok(());
    }

    let exe = env::current_exe()?;
    if !exe_inside_prefix(&exe, &args.prefix) {
        return Ok(());
    }

    let temp = env::temp_dir().join(format!("woc-updater.{}", process::id()));
    fs::copy(&exe, &temp)?;
    let mut perms = fs::metadata(&temp)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&temp, perms)?;

    let mut cmd = process::Command::new(&temp);
    cmd.arg("--already-copied");
    for arg in env::args().skip(1) {
        cmd.arg(arg);
    }

    let err = cmd.exec();
    Err(UpdateError::Io(err))
}

#[cfg(not(unix))]
fn maybe_self_update(_args: &Args, _manifest: &Manifest) -> Result<(), UpdateError> {
    Ok(())
}

fn artifact_store<'a>(
    args: &Args,
    manifest_path: &str,
) -> Result<Box<dyn ArtifactStore + 'a>, UpdateError> {
    if let Some(store_dir) = &args.store {
        return Ok(Box::new(DirStore {
            root: store_dir.clone(),
        }));
    }
    if is_http_url(manifest_path) {
        let base = url_parent(manifest_path)
            .ok_or_else(|| UpdateError::Msg("manifest URL has no parent".into()))?;
        return Ok(Box::new(HttpStore::new(base)));
    }
    let manifest_file = Path::new(manifest_path);
    let parent = manifest_file
        .parent()
        .ok_or_else(|| UpdateError::Msg("manifest path has no parent".into()))?;
    Ok(Box::new(DirStore {
        root: parent.to_path_buf(),
    }))
}

#[cfg(unix)]
fn exec_client(prefix: &Path) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;

    let client = prefix.join("woc-client");
    if !client.exists() {
        return Ok(());
    }
    let meta = fs::metadata(&client)?;
    if meta.permissions().mode() & 0o111 == 0 {
        return Ok(());
    }
    let err = process::Command::new(&client).exec();
    Err(UpdateError::Io(err))
}

#[cfg(not(unix))]
fn exec_client(prefix: &Path) -> Result<(), UpdateError> {
    let client = prefix.join("woc-client");
    if client.exists() {
        let st = process::Command::new(&client).status()?;
        process::exit(st.code().unwrap_or(1));
    }
    Ok(())
}

fn run() -> Result<(), UpdateError> {
    let args = parse_args();
    let manifest = load_manifest(&args.manifest)?;

    let pubkey_hex = args
        .pubkey
        .as_deref()
        .unwrap_or_else(|| default_pubkey_hex());
    let pk = verifying_key_from_hex(pubkey_hex)?;
    verify_manifest(&manifest, &pk)?;

    maybe_self_update(&args, &manifest)?;

    let store = artifact_store(&args, &args.manifest)?;
    apply_update(&args.prefix, &manifest, store.as_ref())?;

    if !args.no_exec {
        exec_client(&args.prefix)?;
    }

    let _ = args.once;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("woc-updater: {e}");
        process::exit(1);
    }
}
