use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::{self, FromStr};

fn main() {
    let target_hermit =
        matches!(env::var_os("CARGO_CFG_TARGET_OS"), Some(os) if os == OsStr::new("hermit"));

    if !target_hermit {
        eprintln!("target os wrong");
        return;
    }

    let kernel = KernelSrc::find().expect("kernel is not found");
    kernel.build()
}

struct KernelSrc {
    src_dir: PathBuf,
}

impl KernelSrc {
    fn find() -> Option<Self> {
        let src_dir = PathBuf::from_str("/Users/lsw/Code/hermit/kernel").ok()?;
        let manifest_path = src_dir.join("Cargo.toml");
        if manifest_path.exists() {
            Some(Self { src_dir })
        } else {
            None
        }
    }

    fn build(self) {
        let target_dir = target_dir();
        let arch = env::var_os("CARGO_CFG_TARGET_ARCH").unwrap();
        let profile = env::var("PROFILE").expect("PROFILE was not set");
        let mut cargo = cargo();

        cargo
            .current_dir(&self.src_dir)
            .arg("run")
            .arg("--package=xtask")
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("--")
            .arg("build")
            .arg("--arch")
            .arg(&arch)
            .args([
                "--profile",
                match profile.as_str() {
                    "debug" => "dev",
                    profile => profile,
                },
            ])
            .arg("--target-dir")
            .arg(&target_dir);

        if has_feature("instrument") {
            cargo.arg("--instrument-mcount");
        }

        if has_feature("randomize-layout") {
            cargo.arg("--randomize-layout");
        }

        // Control enabled features via this crate's features
        cargo.arg("--no-default-features");
        forward_features(
            &mut cargo,
            [
                "acpi", "dhcpv4", "fsgsbase", "pci", "pci-ids", "smp", "tcp", "udp", "trace",
                "vga", "rtl8139", "fs",
            ]
            .into_iter(),
        );

        println!("cargo:warning=$ {cargo:?}");
        let status = cargo.status().expect("failed to start kernel build");
        assert!(status.success());

        let lib_location = target_dir
            .join(&arch)
            .join(&profile)
            .canonicalize()
            .unwrap();

        eprintln!("hermit kernel is available at {}", lib_location.display());

        {
            //create symbolic link to the root
            let mut libhermit_dest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
            libhermit_dest.push("libhermit.a");
            let libhermit_src = lib_location.join("libhermit.a");
            let mut ln = Command::new("ln");
            ln.arg("-sf").arg(libhermit_src).arg(libhermit_dest);
            let status = ln
                .status()
                .expect("failed to create symbolic link to libhermit.a");
            assert!(status.success());
        }

        self.rerun_if_changed_cargo(&self.src_dir.join("Cargo.toml"));
        self.rerun_if_changed_cargo(&self.src_dir.join("hermit-builtins/Cargo.toml"));

        println!(
            "cargo:rerun-if-changed={}",
            self.src_dir.join("rust-toolchain.toml").display()
        );
        println!("cargo:rerun-if-env-changed=HERMIT_LOG_LEVEL_FILTER");

        println!("cargo:rustc-link-search=native={}", lib_location.display());
        println!("cargo:rustc-link-lib=static=hermit");
    }

    fn rerun_if_changed_cargo(&self, cargo_toml: &Path) {
        let mut cargo = cargo();

        let output = cargo
            .arg("tree")
            .arg(format!("--manifest-path={}", cargo_toml.display()))
            .arg("--prefix=none")
            .arg("--workspace")
            .output()
            .unwrap();

        let output = str::from_utf8(&output.stdout).unwrap();

        let path_deps = output.lines().filter_map(|dep| {
            let mut split = dep.split(&['(', ')']);
            split.next();
            let path = split.next()?;
            path.starts_with('/').then_some(path)
        });

        for path_dep in path_deps {
            println!("cargo:rerun-if-changed={path_dep}/src");
            println!("cargo:rerun-if-changed={path_dep}/Cargo.toml");
            if Path::new(path_dep).join("Cargo.lock").exists() {
                println!("cargo:rerun-if-changed={path_dep}/Cargo.lock");
            }
            if Path::new(path_dep).join("build.rs").exists() {
                println!("cargo:rerun-if-changed={path_dep}/build.rs");
            }
        }
    }
}

fn target_dir() -> PathBuf {
    let mut target_dir: PathBuf = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    target_dir.push("hermit_kernel");
    target_dir
}

fn cargo() -> Command {
    let cargo = {
        let mut cargo_home = PathBuf::from(env::var_os("CARGO_HOME").unwrap());
        cargo_home.push("bin");
        cargo_home.push("cargo");
        if cargo_home.exists() {
            cargo_home
        } else {
            PathBuf::from("cargo")
        }
    };

    let mut cargo = Command::new(cargo);
    // Remove rust-toolchain-specific environment variables from kernel cargo
    cargo.env_remove("LD_LIBRARY_PATH");
    env::vars()
        .filter(|(key, _value)| key.starts_with("CARGO") || key.starts_with("RUST"))
        .for_each(|(key, _value)| {
            cargo.env_remove(&key);
        });

    cargo
}

fn has_feature(feature: &str) -> bool {
    let mut var = "CARGO_FEATURE_".to_string();

    var.extend(feature.chars().map(|c| match c {
        '-' => '_',
        c => c.to_ascii_uppercase(),
    }));

    env::var_os(&var).is_some()
}

fn forward_features<'a>(cmd: &mut Command, features: impl Iterator<Item = &'a str>) {
    let features = features.filter(|f| has_feature(f)).collect::<Vec<_>>();
    if !features.is_empty() {
        cmd.args(["--features", &features.join(" ")]);
    }
}
