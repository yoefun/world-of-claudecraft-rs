use std::env;
use std::path::PathBuf;
use std::process;
use woc_update::{pack_release, PackOpts, UpdateError};

fn usage() -> ! {
    eprintln!(
        "usage: woc-pack --layout DIR --out DIR --version VER --target TARGET \\
       --protocol-rev N --key HEX [--prev DIR --prev-version VER]"
    );
    process::exit(2);
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("woc-pack: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), UpdateError> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
    }

    let layout = arg_value(&args, "--layout").map(PathBuf::from);
    let out = arg_value(&args, "--out").map(PathBuf::from);
    let version = arg_value(&args, "--version");
    let target = arg_value(&args, "--target");
    let protocol_rev = arg_value(&args, "--protocol-rev");
    let key = arg_value(&args, "--key");
    let prev = arg_value(&args, "--prev").map(PathBuf::from);
    let prev_version = arg_value(&args, "--prev-version");

    let (Some(layout), Some(out), Some(version), Some(target), Some(protocol_rev), Some(key)) =
        (layout, out, version, target, protocol_rev, key)
    else {
        usage();
    };

    if prev.is_some() != prev_version.is_some() {
        return Err(UpdateError::Msg(
            "--prev and --prev-version must be given together".into(),
        ));
    }

    let protocol_rev: u32 = protocol_rev
        .parse()
        .map_err(|_| UpdateError::Msg("invalid --protocol-rev".into()))?;

    pack_release(PackOpts {
        layout: &layout,
        prev_layout: prev.as_deref(),
        prev_version: prev_version.as_deref(),
        out: &out,
        version: &version,
        target: &target,
        protocol_rev,
        signing_seed_hex: &key,
    })?;

    Ok(())
}
