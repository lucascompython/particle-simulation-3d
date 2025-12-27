use argh::FromArgs;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
struct CommandError {
    command: String,
    status: std::process::ExitStatus,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Command '{}' failed with status: {}",
            self.command, self.status
        )
    }
}

impl Error for CommandError {}

/// Build helper for particle-simulation-3d
#[derive(FromArgs)]
#[argh(help_triggers("-h", "--help"))]
struct Args {
    /// build for wasm target using Trunk
    #[argh(switch, short = 'w')]
    wasm: bool,

    /// enable wasm-rayon feature and atomics (requires --wasm)
    #[argh(switch, short = 'r')]
    wasm_rayon: bool,

    /// build for a native target
    #[argh(option, short = 't')]
    target: Option<String>,

    /// enable native CPU optimizations (requires --target)
    #[argh(switch, short = 'n')]
    native: bool,

    /// set the public URL for Trunk build
    #[argh(option, short = 'p')]
    public_url: Option<String>,

    /// use local trunk binary (for CI)
    #[argh(switch, short = 'c')]
    ci: bool,
}

struct ConfigGuard {
    path: PathBuf,
}

impl ConfigGuard {
    fn new(path: PathBuf) -> Self {
        ConfigGuard { path }
    }
}

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        let final_content = r#"[alias]
release = "run --manifest-path ./release/Cargo.toml --"
"#;

        println!("Writing minimal config (aliases only) to {:?}", self.path);
        match fs::write(&self.path, final_content) {
            Ok(_) => (),
            Err(e) => eprintln!("Error writing final config to {:?}: {}", self.path, e),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = argh::from_env();

    if args.target.is_none() && !args.wasm {
        eprintln!("Error: At least --target=<triple> or --wasm flag is required.");
        eprintln!("Usage: cargo release [OPTIONS]\nSee: cargo release --help");
        std::process::exit(1);
    }
    if args.wasm_rayon && !args.wasm {
        eprintln!("Error: --wasm-rayon requires --wasm");
        std::process::exit(1);
    }
    if args.native && args.target.is_none() {
        eprintln!("Error: --native requires --target");
        std::process::exit(1);
    }

    let project_root = env::current_dir()?;
    let config_path = project_root.join(".cargo/config.toml");

    let build_time_content = r#"[alias]
release = "run --manifest-path ./release/Cargo.toml --"

[unstable]
build-std = ["std", "panic_abort"]
build-std-features = [""]
trim-paths = true
"#;

    println!("Writing build-time config to {:?}", config_path);
    fs::write(&config_path, build_time_content).map_err(|e| {
        format!(
            "Failed to write build-time config to {:?}: {}",
            config_path, e
        )
    })?;

    let _config_guard = ConfigGuard::new(config_path);

    let base_rustflags = "-Csymbol-mangling-version=v0 -Zlocation-detail=none ";

    let mut success = true;

    if args.wasm {
        match build_wasm(&args, &project_root, base_rustflags) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Wasm build failed: {}", e);
                success = false;
            }
        }
    }

    if success && let Some(target) = args.target.as_ref() {
        match build_native(&args, target, &project_root, base_rustflags) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Native build failed: {}", e);
                success = false;
            }
        }
    }

    if success {
        println!("Build finished successfully.");
        Ok(())
    } else {
        eprintln!("Build failed.");
        std::process::exit(1);
    }
}

fn run_command(
    cmd_path: &Path,
    args: &[&str],
    env_vars: &[(&str, &str)],
    cwd: &Path,
) -> Result<(), Box<dyn Error>> {
    let cmd_name = cmd_path
        .file_name()
        .unwrap_or(OsStr::new("command"))
        .to_string_lossy();
    println!("Running: {} {}", cmd_path.display(), args.join(" "));
    for (key, val) in env_vars {
        println!("  Env: {}={}", key, val);
    }

    let mut command = Command::new(cmd_path);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for (key, val) in env_vars {
        command.env(key, val);
    }

    let status = command
        .spawn()
        .map_err(|e| format!("Failed to spawn command '{}': {}", cmd_name, e))?
        .wait()
        .map_err(|e| format!("Failed to wait for command '{}': {}", cmd_name, e))?;

    if !status.success() {
        Err(Box::new(CommandError {
            command: format!("{} {}", cmd_path.display(), args.join(" ")),
            status,
        }))
    } else {
        Ok(())
    }
}

fn build_wasm(
    args: &Args,
    project_root: &Path,
    base_rustflags: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Building particle-simulation-3d for web...");
    let mut wasm_rustflags = format!(
        "{} -C target-feature=-nontrapping-fptoint -Zunstable-options -Cpanic=immediate-abort",
        base_rustflags
    );
    let mut trunk_args = vec!["build", "--release"];

    if args.wasm_rayon {
        println!("Enabling wasm-rayon feature and atomics...");

        // https://github.com/RReverser/wasm-bindgen-rayon#using-config-files
        wasm_rustflags.extend([
            " -C",
            "target-feature=+atomics,+bulk-memory",
            " -C",
            "link-arg=--shared-memory",
            " -C",
            "link-arg=--max-memory=1073741824",
            " -C",
            "link-arg=--import-memory",
            " -C",
            "link-arg=--export=__wasm_init_tls",
            " -C",
            "link-arg=--export=__tls_size",
            " -C",
            "link-arg=--export=__tls_align",
            " -C",
            "link-arg=--export=__tls_base",
        ]);

        trunk_args.push("--features");
        trunk_args.push("wasm-rayon");
    }

    let trunk_cmd_path = if args.ci {
        println!("CI mode enabled: Using local trunk binary ./trunk");
        let local_trunk = project_root.join("trunk");
        if !local_trunk.exists() {
            return Err(format!(
                "Error: CI mode specified, but '{:?}' not found.",
                local_trunk
            )
            .into());
        }
        local_trunk
    } else {
        PathBuf::from("trunk")
    };

    let public_url_holder;
    if let Some(public_url) = &args.public_url {
        println!("Using public URL: {}", public_url);
        trunk_args.push("--public-url");
        public_url_holder = public_url.clone();
        trunk_args.push(&public_url_holder);
    }

    run_command(
        &trunk_cmd_path,
        &trunk_args,
        &[("RUSTFLAGS", &wasm_rustflags)],
        project_root,
    )
}

fn build_native(
    args: &Args,
    target: &str,
    project_root: &Path,
    base_rustflags: &str,
) -> Result<(), Box<dyn Error>> {
    let mut native_rustflags = format!(
        "{} -Zfmt-debug=none -Clink-args=-fuse-ld=lld -Clink-args=-Wl,--icf=all",
        base_rustflags
    );

    if args.native {
        println!(
            "Building particle-simulation-3d for {} with native CPU optimizations...",
            target
        );
        native_rustflags.push_str(" -C target-cpu=native");
    } else {
        println!("Building particle-simulation-3d for {}...", target);
    }

    let cargo_args = vec!["+nightly", "build", "--target", target, "--release"];

    run_command(
        Path::new("cargo"),
        &cargo_args,
        &[("RUSTFLAGS", &native_rustflags)],
        project_root,
    )
}
