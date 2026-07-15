// Serez package manager
//
// Package layout on disk:
//   $SEREZ_PACKAGES/   (or ~/.serez/packages/)
//     <name>/
//       index.sz        ← default entry point
//       <submod>.sz     ← named submodule
//
// serez.json (project manifest):
//   { "name": "...", "version": "...", "description": "...", "author": "...", "dependencies": { "pkg": "version", ... } }
//
// Registry layout (SEREZ_REGISTRY env var or ~/.serez/registry/):
//   <name>/
//     <version>/
//       index.sz
//
// HTTP registry (SEREZ_REGISTRY_URL env var or https://packages.serezcode.org):
//   GET  /api/packages/<name>/latest          → plain-text version string
//   GET  /api/packages/<name>/<version>.zip   → zip archive of package files
//   GET  /api/packages/<name>/stats           → JSON with download stats
//   POST /api/publish                         → publish a new package version
//   DEL  /api/unpublish/<name>/<version>      → yank a version

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

// ── Manifest ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SerezManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub dependencies: HashMap<String, String>,
    pub permissions: Vec<String>,
    pub scripts: HashMap<String, String>,
    /// Executable commands a package exposes: command name -> entry `.sz` file
    /// (relative to the package dir). Lets `sz run <command>` launch a package.
    pub bin: HashMap<String, String>,
}

impl SerezManifest {
    /// Read and parse `serez.json` from `dir`.
    pub fn load(dir: &Path) -> Result<SerezManifest, String> {
        let path = dir.join("serez.json");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot read serez.json: {}", e))?;
        SerezManifest::parse(&raw)
    }

    fn parse(raw: &str) -> Result<SerezManifest, String> {
        // Minimal hand-rolled JSON parser for the specific manifest shape.
        let raw = raw.trim();
        if !raw.starts_with('{') || !raw.ends_with('}') {
            return Err("serez.json must be a JSON object".to_string());
        }
        let mut name = String::new();
        let mut version = String::new();
        let mut description = String::new();
        let mut author = String::new();
        let mut dependencies: HashMap<String, String> = HashMap::new();
        let mut permissions: Vec<String> = Vec::new();
        let mut scripts: HashMap<String, String> = HashMap::new();
        let mut bin: HashMap<String, String> = HashMap::new();

        // Extract top-level string fields and the dependencies object.
        let inner = &raw[1..raw.len() - 1];

        // Simple tokenizer: extract quoted keys and values
        let mut chars = inner.chars().peekable();
        loop {
            // skip whitespace and commas
            while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
                chars.next();
            }
            if chars.peek().is_none() { break; }

            // Expect a key
            if chars.peek() != Some(&'"') { break; }
            let key = read_json_string(&mut chars)?;

            // Expect ':'
            skip_ws_and(&mut chars, ':');

            // Either a quoted string or '{' for object
            match chars.peek() {
                Some('"') => {
                    let val = read_json_string(&mut chars)?;
                    match key.as_str() {
                        "name"        => name = val,
                        "version"     => version = val,
                        "description" => description = val,
                        "author"      => author = val,
                        _             => {}
                    }
                }
                Some('{') => {
                    if key == "dependencies" {
                        dependencies = parse_string_map(&mut chars)?;
                    } else if key == "scripts" {
                        scripts = parse_string_map(&mut chars)?;
                    } else if key == "bin" {
                        bin = parse_string_map(&mut chars)?;
                    } else {
                        skip_value(&mut chars);
                    }
                }
                Some('[') => {
                    if key == "permissions" {
                        permissions = parse_string_array(&mut chars)?;
                    } else {
                        skip_value(&mut chars);
                    }
                }
                _ => { skip_value(&mut chars); }
            }
        }

        if name.is_empty() {
            return Err("serez.json: 'name' field is required".to_string());
        }
        if version.is_empty() {
            return Err("serez.json: 'version' field is required".to_string());
        }
        Ok(SerezManifest { name, version, description, author, dependencies, permissions, scripts, bin })
    }
}

// ── Package resolution ────────────────────────────────────────────────────────

/// Returns the local project package directory: <cwd>/packages/
pub fn local_packages_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("packages")
}

/// Returns the global fallback package directory: $SEREZ_PACKAGES or ~/.serez/packages/
pub fn packages_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SEREZ_PACKAGES") {
        return PathBuf::from(p);
    }
    if let Some(home) = home_dir() {
        return home.join(".serez").join("packages");
    }
    PathBuf::from(".serez/packages")
}

/// Returns the registry directory: $SEREZ_REGISTRY or ~/.serez/registry/
pub fn registry_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SEREZ_REGISTRY") {
        return PathBuf::from(p);
    }
    if let Some(home) = home_dir() {
        return home.join(".serez").join("registry");
    }
    PathBuf::from(".serez/registry")
}

/// Returns the HTTP registry base URL: $SEREZ_REGISTRY_URL or the official registry.
pub fn registry_url() -> String {
    std::env::var("SEREZ_REGISTRY_URL")
        .unwrap_or_else(|_| "https://packages.serezcode.org".to_string())
}

/// Install a package from the local registry or HTTP registry into ./packages/.
/// pkg_spec = "name" or "name@version".
/// When `record` is true, the resolved dependency is written back into the
/// project's serez.json (used by `sz install <pkg>`; skipped by `sz install`
/// which already reads its list from the manifest).
pub fn install_package(pkg_spec: &str, record: bool, global: bool) -> Result<(), String> {
    let (pkg_name, pkg_version) = parse_pkg_spec(pkg_spec);
    let registry = registry_dir();

    // Resolve version: explicit → local registry latest → HTTP latest
    let version = if let Some(v) = pkg_version {
        v
    } else {
        let pkg_reg_dir = registry.join(&pkg_name);
        if pkg_reg_dir.exists() {
            find_latest_version(&pkg_reg_dir)
                .ok_or_else(|| format!("No versions of '{}' found in local registry", pkg_name))?
        } else {
            fetch_latest_version(&pkg_name)?
        }
    };

    let dest = if global { packages_dir() } else { local_packages_dir() }.join(&pkg_name);
    if dest.exists() {
        println!("Package '{}' already installed, updating...", pkg_name);
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to remove old version: {}", e))?;
    }

    // Try local registry first, fall back to HTTP
    let src = registry.join(&pkg_name).join(&version);
    if src.exists() {
        copy_dir_recursive(&src, &dest)
            .map_err(|e| format!("Failed to install '{}@{}': {}", pkg_name, version, e))?;
        if global {
            println!("✅ Installed {}@{} → {} (global)", pkg_name, version, dest.display());
        } else {
            println!("✅ Installed {}@{} → ./packages/{}", pkg_name, version, pkg_name);
        }
    } else {
        download_package(&pkg_name, &version, global)?;
    }

    // Record the resolved dependency in serez.json (local installs only — a
    // global package is not a project dependency). Manifest write failure is a
    // warning, not a hard error, since the package is already on disk.
    if record && !global {
        if let Ok(cwd) = std::env::current_dir() {
            if let Err(e) = record_dependency(&cwd, &pkg_name, &version) {
                eprintln!("⚠ Installed, but could not update serez.json: {}", e);
            }
        }
    }

    Ok(())
}

/// Initialize a serez.json in the current directory.
/// `yes` = skip prompts and use defaults (folder name as project name).
pub fn init_project(yes: bool) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;

    let manifest_path = cwd.join("serez.json");
    if manifest_path.exists() && !yes {
        print!("serez.json already exists. Overwrite? (y/N): ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let folder_name = cwd
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-project".to_string());

    let (name, version, description, author) = if yes {
        (folder_name, "1.0.0".to_string(), String::new(), String::new())
    } else {
        (
            prompt(&format!("name ({}): ", folder_name), &folder_name),
            prompt("version (1.0.0): ", "1.0.0"),
            prompt("description: ", ""),
            prompt("author: ", ""),
        )
    };

    let json = format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"{}\",\n  \"description\": \"{}\",\n  \"author\": \"{}\",\n  \"scripts\": {{\n    \"dev\": \"sz index.sz\"\n  }},\n  \"dependencies\": {{}},\n  \"permissions\": []\n}}\n",
        name, version, description, author
    );

    std::fs::write(&manifest_path, &json)
        .map_err(|e| format!("Cannot write serez.json: {}", e))?;

    println!("✅ Created serez.json");
    Ok(())
}

fn prompt(label: &str, default: &str) -> String {
    print!("{}", label);
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() { default.to_string() } else { trimmed }
}

/// Execute a script defined in serez.json's "scripts" section.
/// `sz run <name> [args...]` — resolves `<name>` in priority order:
///   1. a script declared in the project's serez.json (`scripts`)
///   2. a command declared by an installed package (`bin`), local then global
/// Extra args are forwarded to whichever target is run.
pub fn run_script(name: &str, extra_args: &[String]) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;

    // 1. Project script takes precedence (only if a project manifest exists).
    if let Ok(manifest) = SerezManifest::load(&cwd) {
        if let Some(cmd) = manifest.scripts.get(name).cloned() {
            return run_project_script(name, &cmd, &cwd, extra_args);
        }
    }

    // 2. A command exposed by an installed package's `bin`.
    if let Some((pkg_dir, entry)) = resolve_package_bin(&cwd, name)? {
        return run_package_bin(name, &pkg_dir, &entry, &cwd, extra_args);
    }

    // 3. Nothing matched — list what is available to help the user.
    Err(run_not_found_error(&cwd, name))
}

/// Run a project `scripts` entry through the shell, forwarding extra args.
fn run_project_script(name: &str, cmd: &str, cwd: &Path, extra_args: &[String]) -> Result<(), String> {
    let full = if extra_args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, extra_args.join(" "))
    };
    println!("▶ {}", full);

    let status = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", &full]).current_dir(cwd).status()
    } else {
        std::process::Command::new("sh").args(["-c", &full]).current_dir(cwd).status()
    }
    .map_err(|e| format!("Failed to execute script '{}': {}", name, e))?;

    if !status.success() {
        return Err(format!("Script '{}' exited with code {}", name, status.code().unwrap_or(1)));
    }
    Ok(())
}

/// Launch a package's `bin` entry as `sz <pkgdir>/<entry> <args>` with CWD kept
/// at the project root, so the entry's imports/permissions resolve from the
/// package dir while `project=.` still points at the user's app.
fn run_package_bin(name: &str, pkg_dir: &Path, entry: &str, cwd: &Path, extra_args: &[String]) -> Result<(), String> {
    if !entry.ends_with(".sz") {
        return Err(format!("Command '{}': bin entry '{}' must be a .sz file", name, entry));
    }
    let entry_path = pkg_dir.join(entry);
    if !entry_path.exists() {
        return Err(format!("Command '{}': entry not found at {}", name, entry_path.display()));
    }
    let sz_exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate the sz executable: {}", e))?;

    println!("▶ {} ({})", name, entry_path.display());
    let status = std::process::Command::new(sz_exe)
        .arg(&entry_path)
        .args(extra_args)
        .current_dir(cwd) // project root → `project=.` stays the user's app
        .status()
        .map_err(|e| format!("Failed to run command '{}': {}", name, e))?;

    if !status.success() {
        return Err(format!("Command '{}' exited with code {}", name, status.code().unwrap_or(1)));
    }
    Ok(())
}

/// Find a package that declares `command` in its `bin`. Searches local
/// `./packages` first, then the global packages dir (local shadows global for
/// the same package folder). Supports explicit `pkg:command` disambiguation.
/// Returns the package dir and entry file, or an error if two different
/// packages declare the same command.
fn resolve_package_bin(cwd: &Path, command: &str) -> Result<Option<(PathBuf, String)>, String> {
    let (pkg_filter, cmd_name) = match command.split_once(':') {
        Some((p, c)) => (Some(p.to_string()), c.to_string()),
        None => (None, command.to_string()),
    };

    let roots = [cwd.join("packages"), packages_dir()];
    let mut matches: Vec<(String, PathBuf, String)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for root in &roots {
        let dir_iter = match std::fs::read_dir(root) {
            Ok(d) => d,
            Err(_) => continue, // root may not exist; that's fine
        };
        for entry in dir_iter.flatten() {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }
            let folder = match pkg_dir.file_name().and_then(|s| s.to_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };
            if seen.contains(&folder) {
                continue; // already taken from a higher-priority root (local)
            }
            seen.push(folder.clone());

            let manifest = match SerezManifest::load(&pkg_dir) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if let Some(ref pf) = pkg_filter {
                if pf != &folder && pf != &manifest.name {
                    continue;
                }
            }
            if let Some(entry_file) = manifest.bin.get(&cmd_name) {
                matches.push((folder, pkg_dir, entry_file.clone()));
            }
        }
    }

    match matches.len() {
        0 => Ok(None),
        1 => {
            let (_folder, dir, entry) = matches.into_iter().next().unwrap();
            Ok(Some((dir, entry)))
        }
        _ => {
            let opts: Vec<String> =
                matches.iter().map(|(f, _, _)| format!("{}:{}", f, cmd_name)).collect();
            Err(format!(
                "Command '{}' is provided by multiple packages. Disambiguate with: sz run {}",
                cmd_name,
                opts.join(" | sz run ")
            ))
        }
    }
}

/// Build a helpful "not found" message listing project scripts and the package
/// commands that `sz run` could have matched.
fn run_not_found_error(cwd: &Path, name: &str) -> String {
    let mut scripts: Vec<String> = Vec::new();
    if let Ok(manifest) = SerezManifest::load(cwd) {
        scripts = manifest.scripts.keys().cloned().collect();
    }
    let mut commands: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for root in [cwd.join("packages"), packages_dir()] {
        let dir_iter = match std::fs::read_dir(&root) { Ok(d) => d, Err(_) => continue };
        for entry in dir_iter.flatten() {
            let pkg_dir = entry.path();
            let folder = match pkg_dir.file_name().and_then(|s| s.to_str()) {
                Some(f) => f.to_string(), None => continue,
            };
            if seen.contains(&folder) { continue; }
            seen.push(folder.clone());
            if let Ok(m) = SerezManifest::load(&pkg_dir) {
                for cmd in m.bin.keys() {
                    commands.push(format!("{} (from {})", cmd, folder));
                }
            }
        }
    }
    scripts.sort();
    commands.sort();

    let mut msg = format!("'{}' not found — not a project script or a package command.", name);
    if !scripts.is_empty() {
        msg.push_str(&format!("\n  Project scripts: {}", scripts.join(", ")));
    }
    if !commands.is_empty() {
        msg.push_str(&format!("\n  Package commands: {}", commands.join(", ")));
    }
    msg
}

/// Remove a package from the local project packages directory.
pub fn uninstall_package(pkg_name: &str, global: bool) -> Result<(), String> {
    let dest = if global { packages_dir() } else { local_packages_dir() }.join(pkg_name);
    if !dest.exists() {
        let scope = if global { "the global store" } else { "./packages/" };
        return Err(format!("Package '{}' is not installed in {}", pkg_name, scope));
    }
    std::fs::remove_dir_all(&dest)
        .map_err(|e| format!("Failed to uninstall '{}': {}", pkg_name, e))?;
    println!("✅ Uninstalled {}{}", pkg_name, if global { " (global)" } else { "" });

    // Drop the dependency from the project serez.json only for local uninstalls
    // (a global package is not a project dependency).
    if !global {
        if let Ok(cwd) = std::env::current_dir() {
            if let Err(e) = remove_dependency(&cwd, pkg_name) {
                eprintln!("⚠ Uninstalled, but could not update serez.json: {}", e);
            }
        }
    }

    Ok(())
}

/// Remove every package from the global store (~/.serez/packages).
/// Used by `sz uninstall -g` with no package name.
pub fn uninstall_all_global() -> Result<(), String> {
    let dir = packages_dir();
    if !dir.is_dir() {
        println!("No global packages installed.");
        return Ok(());
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| format!("Cannot read global packages dir: {}", e))?
        .flatten()
    {
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            std::fs::remove_dir_all(&p)
                .map_err(|e| format!("Failed to remove global '{}': {}", name, e))?;
            println!("  removed {}", name);
            count += 1;
        }
    }
    println!("✅ Uninstalled {} global package(s) from {}", count, dir.display());
    Ok(())
}

/// Update a package to the latest PUBLISHED version. Unlike `install`, this
/// always queries the remote registry for the latest version (so a stale
/// local-registry copy never shadows a newer release). Local by default,
/// global with `global=true`.
pub fn update_package(name: &str, global: bool) -> Result<(), String> {
    let version = fetch_latest_version(name)?;
    println!("Updating {} → {} ...", name, version);
    install_package(&format!("{}@{}", name, version), !global, global)
}

/// Update every package: the project's serez.json dependencies (local), or
/// every package in the global store (when `global=true`).
pub fn update_all(global: bool) -> Result<(), String> {
    if global {
        let dir = packages_dir();
        if !dir.is_dir() {
            println!("No global packages installed.");
            return Ok(());
        }
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| format!("Cannot read global packages dir: {}", e))?
            .flatten()
        {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if let Err(e) = update_package(name, true) {
                        eprintln!("⚠ {}: {}", name, e);
                    }
                }
            }
        }
        return Ok(());
    }

    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let manifest = SerezManifest::load(&cwd)?;
    if manifest.dependencies.is_empty() {
        println!("No dependencies to update.");
        return Ok(());
    }
    let names: Vec<String> = manifest.dependencies.keys().cloned().collect();
    for name in names {
        if let Err(e) = update_package(&name, false) {
            eprintln!("⚠ {}: {}", name, e);
        }
    }
    Ok(())
}

/// Install all dependencies listed in serez.json from the current directory.
pub fn install_all() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let manifest = SerezManifest::load(&cwd)?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies to install.");
        return Ok(());
    }

    for (name, version) in &manifest.dependencies {
        let spec = format!("{}@{}", name, version);
        // Don't rewrite the manifest: these deps already come from it. Local.
        install_package(&spec, false, false)?;
    }
    Ok(())
}

// ── Manifest write-back ─────────────────────────────────────────────────────────
//
// These helpers edit serez.json in place: only the "dependencies" object is
// reformatted (canonical 2-space layout), the rest of the file — name, version,
// scripts, permissions, comments-free formatting — is preserved verbatim.

/// Insert or update a dependency in the project's serez.json.
/// No-op (with a hint) if there is no manifest in `dir`.
fn record_dependency(dir: &Path, name: &str, version: &str) -> Result<(), String> {
    let path = dir.join("serez.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => {
            println!("ℹ No serez.json found — run `sz init` to track dependencies.");
            return Ok(());
        }
    };
    let updated = upsert_dependency(&raw, name, version)?;
    std::fs::write(&path, &updated).map_err(|e| format!("Cannot write serez.json: {}", e))?;
    println!("   added {}@{} to serez.json", name, version);
    Ok(())
}

/// Remove a dependency from the project's serez.json if present.
/// No-op if there is no manifest or the dependency isn't listed.
fn remove_dependency(dir: &Path, name: &str) -> Result<(), String> {
    let path = dir.join("serez.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let (obj_start, obj_end) = match find_object_span(&raw, "dependencies") {
        Some(span) => span,
        None => return Ok(()),
    };
    let mut pairs = parse_ordered_pairs(&raw[obj_start + 1..obj_end])?;
    let before = pairs.len();
    pairs.retain(|(k, _)| k != name);
    if pairs.len() == before {
        return Ok(()); // dependency wasn't listed; leave the file untouched
    }
    let rendered = render_deps_object(&pairs);
    let mut out = String::with_capacity(raw.len());
    out.push_str(&raw[..obj_start]);
    out.push_str(&rendered);
    out.push_str(&raw[obj_end + 1..]);
    std::fs::write(&path, &out).map_err(|e| format!("Cannot write serez.json: {}", e))?;
    println!("   removed {} from serez.json", name);
    Ok(())
}

/// Insert or update `name`→`version` inside the raw serez.json text. Splices a
/// freshly rendered "dependencies" object in place; if the manifest has no
/// "dependencies" key, one is appended before the closing brace.
fn upsert_dependency(raw: &str, name: &str, version: &str) -> Result<String, String> {
    if let Some((obj_start, obj_end)) = find_object_span(raw, "dependencies") {
        let mut pairs = parse_ordered_pairs(&raw[obj_start + 1..obj_end])?;
        upsert_pair(&mut pairs, name, version);
        let rendered = render_deps_object(&pairs);
        let mut out = String::with_capacity(raw.len() + name.len() + version.len() + 16);
        out.push_str(&raw[..obj_start]);
        out.push_str(&rendered);
        out.push_str(&raw[obj_end + 1..]);
        Ok(out)
    } else {
        insert_deps_key(raw, name, version)
    }
}

/// Locate the `{ ... }` value of a top-level object key. Returns the byte index
/// of the opening and matching closing brace (inclusive), or None if the key or
/// its object value is absent.
fn find_object_span(raw: &str, key: &str) -> Option<(usize, usize)> {
    let bytes = raw.as_bytes();
    let needle = format!("\"{}\"", key);
    let mut from = 0;
    while let Some(rel) = raw[from..].find(&needle) {
        let key_at = from + rel;
        // Skip whitespace then expect ':'.
        let mut i = key_at + needle.len();
        while i < bytes.len() && (bytes[i] as char).is_whitespace() { i += 1; }
        if i >= bytes.len() || bytes[i] != b':' {
            from = key_at + needle.len();
            continue;
        }
        i += 1;
        while i < bytes.len() && (bytes[i] as char).is_whitespace() { i += 1; }
        if i < bytes.len() && bytes[i] == b'{' {
            if let Some(close) = match_braces(bytes, i) {
                return Some((i, close));
            }
        }
        from = key_at + needle.len();
    }
    None
}

/// Given `bytes[open] == b'{'`, return the index of the matching `}`, honoring
/// quoted strings and escapes. None if unbalanced.
fn match_braces(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Parse the body of a string→string object (text between its braces) into an
/// ordered list, preserving the original key order.
fn parse_ordered_pairs(body: &str) -> Result<Vec<(String, String)>, String> {
    let mut chars = body.chars().peekable();
    let mut pairs = Vec::new();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek() {
            Some('"') => {}
            _ => break,
        }
        let key = read_json_string(&mut chars)?;
        skip_ws_and(&mut chars, ':');
        let val = read_json_string(&mut chars)?;
        pairs.push((key, val));
    }
    Ok(pairs)
}

/// Set `name`→`version`, updating in place if present or appending otherwise.
fn upsert_pair(pairs: &mut Vec<(String, String)>, name: &str, version: &str) {
    for p in pairs.iter_mut() {
        if p.0 == name {
            p.1 = version.to_string();
            return;
        }
    }
    pairs.push((name.to_string(), version.to_string()));
}

/// Render a dependencies object in canonical layout (2-space base indent,
/// 4-space entries). Empty maps render as `{}`.
fn render_deps_object(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return "{}".to_string();
    }
    let mut s = String::from("{\n");
    for (i, (k, v)) in pairs.iter().enumerate() {
        s.push_str("    \"");
        s.push_str(&json_escape(k));
        s.push_str("\": \"");
        s.push_str(&json_escape(v));
        s.push('"');
        if i + 1 < pairs.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  }");
    s
}

/// Append a `"dependencies"` key (with the single entry) just before the
/// manifest's final closing brace, adding a separating comma when needed.
fn insert_deps_key(raw: &str, name: &str, version: &str) -> Result<String, String> {
    let close = raw
        .rfind('}')
        .ok_or_else(|| "serez.json: malformed (no closing brace)".to_string())?;
    let head = raw[..close].trim_end();
    let needs_comma = !head.ends_with('{');
    let deps = render_deps_object(&[(name.to_string(), version.to_string())]);
    let mut out = String::with_capacity(raw.len() + deps.len() + 24);
    out.push_str(head);
    if needs_comma {
        out.push(',');
    }
    out.push_str("\n  \"dependencies\": ");
    out.push_str(&deps);
    out.push('\n');
    out.push_str(&raw[close..]);
    Ok(out)
}

/// Escape a string for embedding in a JSON double-quoted value.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

// ── HTTP registry ─────────────────────────────────────────────────────────────

/// Fetch the latest version string for a package from the HTTP registry.
fn fetch_latest_version(pkg_name: &str) -> Result<String, String> {
    let url = format!("{}/api/packages/{}/latest", registry_url(), pkg_name);
    let response = ureq::get(&url).call().map_err(|e| match e {
        ureq::Error::Status(404, _) => format!(
            "Package '{}' not found in local registry or remote registry ({})",
            pkg_name, registry_url()
        ),
        other => format!("Failed to reach registry for '{}': {}", pkg_name, other),
    })?;
    let version = response
        .into_string()
        .map_err(|e| format!("Invalid response from registry: {}", e))?
        .trim()
        .to_string();
    if version.is_empty() {
        return Err(format!("Registry returned empty version for '{}'", pkg_name));
    }
    Ok(version)
}

/// Download a package zip from the HTTP registry and extract it to ./packages/<name>/.
fn download_package(pkg_name: &str, version: &str, global: bool) -> Result<(), String> {
    let url = format!("{}/api/packages/{}/{}.zip", registry_url(), pkg_name, version);
    println!("Downloading {}@{} from {}...", pkg_name, version, registry_url());

    let response = ureq::get(&url).call().map_err(|e| match e {
        ureq::Error::Status(404, _) => format!(
            "Package '{}@{}' not found in remote registry",
            pkg_name, version
        ),
        other => format!("Download failed for '{}@{}': {}", pkg_name, version, other),
    })?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read download for '{}@{}': {}", pkg_name, version, e))?;

    let dest = if global { packages_dir() } else { local_packages_dir() }.join(pkg_name);
    extract_zip(&bytes, &dest)
        .map_err(|e| format!("Failed to extract '{}@{}': {}", pkg_name, version, e))?;

    if global {
        println!("✅ Installed {}@{} → {} (global, remote)", pkg_name, version, dest.display());
    } else {
        println!("✅ Installed {}@{} → ./packages/{} (remote)", pkg_name, version, pkg_name);
    }
    Ok(())
}

// ── Registry credentials ──────────────────────────────────────────────────────
//
// `sz publish` / `sz unpublish` authenticate with a per-user token. The first
// time, the CLI asks for the username/password of an account created on the
// registry website, exchanges them for a long-lived token via POST /api/login,
// and stores it in ~/.serez/credentials.json — after that it's just `sz publish`.
// The legacy SEREZ_API_KEY env var still works (sent as x-api-key) while the
// registry keeps accepting it.

enum RegistryAuth {
    /// Legacy shared key (SEREZ_API_KEY env var), sent as `x-api-key`.
    LegacyKey(String),
    /// Per-user token, sent as `Authorization: Bearer …`.
    Bearer(String),
}

fn credentials_path() -> PathBuf {
    if let Some(home) = home_dir() {
        return home.join(".serez").join("credentials.json");
    }
    PathBuf::from(".serez/credentials.json")
}

/// Load the stored token, but only if it was issued by the current registry
/// (SEREZ_REGISTRY_URL may point elsewhere, e.g. a local dev registry).
fn load_credentials() -> Option<String> {
    let raw = std::fs::read_to_string(credentials_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    if v.get("registry")?.as_str()? != registry_url() {
        return None;
    }
    Some(v.get("token")?.as_str()?.to_string())
}

fn save_credentials(username: &str, token: &str) -> Result<(), String> {
    let path = credentials_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
    }
    let json = serde_json::json!({
        "registry": registry_url(),
        "username": username,
        "token": token,
    });
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap())
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))
}

fn delete_credentials() {
    let _ = std::fs::remove_file(credentials_path());
}

/// `sz logout`: remove the stored registry credential. The next
/// `sz publish` / `sz unpublish` will ask for username/password again.
pub fn logout() -> Result<(), String> {
    let path = credentials_path();
    if !path.exists() {
        println!("No registry session — nothing to log out.");
        return Ok(());
    }
    let username = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("username").and_then(|u| u.as_str()).map(String::from));
    std::fs::remove_file(&path)
        .map_err(|e| format!("Cannot remove {}: {}", path.display(), e))?;
    match username {
        Some(u) => println!("✅ Logged out {} (removed {})", u, path.display()),
        None => println!("✅ Logged out (removed {})", path.display()),
    }
    Ok(())
}

fn prompt_line(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{}", prompt);
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("Cannot read input: {}", e))?;
    Ok(line.trim().to_string())
}

/// Read a password without echoing it (raw mode via crossterm). Falls back to
/// a plain visible read when stdin is not a terminal (e.g. piped input).
fn prompt_password(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{}", prompt);
    std::io::stdout().flush().ok();

    // With piped stdin (tests, scripts) the console isn't where the input is:
    // read the pipe plainly. Raw-mode masking only makes sense on a real TTY.
    let is_tty = {
        use std::io::IsTerminal;
        std::io::stdin().is_terminal()
    };
    if !is_tty || crossterm::terminal::enable_raw_mode().is_err() {
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|e| format!("Cannot read input: {}", e))?;
        return Ok(line.trim_end_matches(['\r', '\n']).to_string());
    }

    use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
    let mut pw = String::new();
    let result = loop {
        match read() {
            Ok(Event::Key(k)) => {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                match k.code {
                    KeyCode::Enter => break Ok(pw.clone()),
                    KeyCode::Backspace => {
                        pw.pop();
                    }
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        break Err("Login cancelled".to_string());
                    }
                    KeyCode::Char(c) => pw.push(c),
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(e) => break Err(format!("Cannot read input: {}", e)),
        }
    };
    let _ = crossterm::terminal::disable_raw_mode();
    println!();
    result
}

/// Interactive login: asks for username/password, exchanges them for a token
/// at POST /api/login and stores it in ~/.serez/credentials.json.
fn interactive_login() -> Result<String, String> {
    println!(
        "Log in with your registry account (create one at {}/register).",
        registry_url()
    );
    let username = prompt_line("Username: ")?;
    if username.is_empty() {
        return Err("Username cannot be empty".to_string());
    }
    let password = prompt_password("Password: ")?;

    let body = serde_json::json!({ "username": username, "password": password });
    let resp = ureq::post(&format!("{}/api/login", registry_url()))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| match e {
            ureq::Error::Status(401, _) => "Invalid username or password".to_string(),
            other => format!("Login failed: {}", other),
        })?;

    let text = resp
        .into_string()
        .map_err(|e| format!("Invalid response from registry: {}", e))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid response from registry: {}", e))?;
    let token = v
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "Registry response did not include a token".to_string())?;

    save_credentials(&username, token)?;
    println!(
        "✅ Logged in as {} (credential stored in {})",
        username,
        credentials_path().display()
    );
    Ok(token.to_string())
}

/// Resolve how to authenticate against the registry, in order:
/// SEREZ_API_KEY (legacy) → stored credential → interactive login.
/// The bool is true when the auth came from the stored credential, so callers
/// can retry with a fresh login if the token turns out to be revoked.
fn registry_auth() -> Result<(RegistryAuth, bool), String> {
    if let Ok(k) = std::env::var("SEREZ_API_KEY") {
        return Ok((RegistryAuth::LegacyKey(k), false));
    }
    if let Some(token) = load_credentials() {
        return Ok((RegistryAuth::Bearer(token), true));
    }
    Ok((RegistryAuth::Bearer(interactive_login()?), false))
}

fn with_auth(req: ureq::Request, auth: &RegistryAuth) -> ureq::Request {
    match auth {
        RegistryAuth::LegacyKey(k) => req.set("x-api-key", k),
        RegistryAuth::Bearer(t) => req.set("Authorization", &format!("Bearer {}", t)),
    }
}

/// Pull the `error` field out of a JSON error body; falls back to the raw text.
fn extract_api_error(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
        .unwrap_or_else(|| body.trim().to_string())
}

// ── Publish / Unpublish / Info ────────────────────────────────────────────────

fn build_publish_body(manifest: &SerezManifest, zip_bytes: &[u8]) -> (Vec<u8>, String) {
    let boundary = "SerezPkgBoundary7MA4YWxkTrZu0gW";
    let mut body: Vec<u8> = Vec::new();

    for (key, val) in &[
        ("name",        manifest.name.as_str()),
        ("version",     manifest.version.as_str()),
        ("description", manifest.description.as_str()),
        ("author",      manifest.author.as_str()),
    ] {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", key).as_bytes(),
        );
        body.extend_from_slice(val.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"zip\"; filename=\"{}-{}.zip\"\r\nContent-Type: application/zip\r\n\r\n",
            manifest.name, manifest.version
        )
        .as_bytes(),
    );
    body.extend_from_slice(zip_bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    (body, format!("multipart/form-data; boundary={}", boundary))
}

/// POST the publish body. On HTTP errors returns (status, server message);
/// transport errors use status 0.
fn send_publish(body: &[u8], content_type: &str, auth: &RegistryAuth) -> Result<(), (u16, String)> {
    let url = format!("{}/api/publish", registry_url());
    with_auth(ureq::post(&url), auth)
        .set("Content-Type", content_type)
        .send_bytes(body)
        .map(|_| ())
        .map_err(|e| match e {
            ureq::Error::Status(code, r) => (code, extract_api_error(&r.into_string().unwrap_or_default())),
            other => (0, format!("{}", other)),
        })
}

fn publish_error_message(code: u16, msg: String, version: &str) -> String {
    match code {
        401 => "Unauthorized — log in again (delete ~/.serez/credentials.json or check SEREZ_API_KEY)".to_string(),
        403 if msg.is_empty() => "This package belongs to another user".to_string(),
        403 => msg,
        409 => format!("Version {} already exists in the registry", version),
        400 => format!("Bad request: {}", msg),
        0 => format!("Publish failed: {}", msg),
        _ => format!("Publish failed ({}): {}", code, msg),
    }
}

/// Publish the package in the current directory to the registry.
/// Reads serez.json, zips the package directory recursively (honoring .szignore),
/// and POSTs to /api/publish. Logs in interactively the first time and reuses
/// the stored credential afterwards.
pub fn publish_package() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let manifest = SerezManifest::load(&cwd)?;

    let (auth, from_store) = registry_auth()?;

    println!("Publishing {}@{} ...", manifest.name, manifest.version);
    let zip_bytes = create_package_zip(&cwd)?;
    let (body, ct) = build_publish_body(&manifest, &zip_bytes);

    match send_publish(&body, &ct, &auth) {
        Ok(()) => {}
        // The stored token was revoked server-side: re-login once and retry.
        Err((401, _)) if from_store => {
            println!("Stored credential is no longer valid — please log in again.");
            delete_credentials();
            let token = interactive_login()?;
            send_publish(&body, &ct, &RegistryAuth::Bearer(token))
                .map_err(|(code, msg)| publish_error_message(code, msg, &manifest.version))?;
        }
        Err((code, msg)) => return Err(publish_error_message(code, msg, &manifest.version)),
    }

    println!("✅ Published {}@{}", manifest.name, manifest.version);
    Ok(())
}

fn send_unpublish(pkg_name: &str, version: &str, auth: &RegistryAuth) -> Result<(), (u16, String)> {
    let url = format!("{}/api/unpublish/{}/{}", registry_url(), pkg_name, version);
    with_auth(ureq::delete(&url), auth)
        .call()
        .map(|_| ())
        .map_err(|e| match e {
            ureq::Error::Status(code, r) => (code, extract_api_error(&r.into_string().unwrap_or_default())),
            other => (0, format!("{}", other)),
        })
}

fn unpublish_error_message(code: u16, msg: String, pkg_name: &str, version: &str) -> String {
    match code {
        401 => "Unauthorized — log in again (delete ~/.serez/credentials.json or check SEREZ_API_KEY)".to_string(),
        403 if msg.is_empty() => "This package belongs to another user".to_string(),
        403 => msg,
        404 => format!("{}@{} not found in registry", pkg_name, version),
        0 => format!("Unpublish failed: {}", msg),
        _ => format!("Unpublish failed ({}): {}", code, msg),
    }
}

/// Remove a published version from the registry (yank).
/// pkg_spec = "name@version"
pub fn unpublish_package_remote(pkg_spec: &str) -> Result<(), String> {
    let (pkg_name, version) = parse_pkg_spec(pkg_spec);
    let version = version.ok_or_else(|| "Usage: sz unpublish <package>@<version>".to_string())?;

    let (auth, from_store) = registry_auth()?;

    match send_unpublish(&pkg_name, &version, &auth) {
        Ok(()) => {}
        Err((401, _)) if from_store => {
            println!("Stored credential is no longer valid — please log in again.");
            delete_credentials();
            let token = interactive_login()?;
            send_unpublish(&pkg_name, &version, &RegistryAuth::Bearer(token))
                .map_err(|(code, msg)| unpublish_error_message(code, msg, &pkg_name, &version))?;
        }
        Err((code, msg)) => return Err(unpublish_error_message(code, msg, &pkg_name, &version)),
    }

    println!("✅ Unpublished {}@{}", pkg_name, version);
    Ok(())
}

/// Show stats and version list for a package in the registry.
pub fn info_package(pkg_name: &str) -> Result<(), String> {
    let url = format!("{}/api/packages/{}/stats", registry_url(), pkg_name);
    let body = ureq::get(&url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => format!("Package '{}' not found", pkg_name),
            other => format!("Failed to reach registry: {}", other),
        })?
        .into_string()
        .map_err(|e| format!("Invalid response: {}", e))?;

    // Minimal display — extract numbers with basic string search
    let total   = extract_json_number(&body, "total").unwrap_or(0);
    let weekly  = extract_json_number(&body, "weekly").unwrap_or(0);
    let monthly = extract_json_number(&body, "monthly").unwrap_or(0);

    println!("\nPackage: {}", pkg_name);
    println!("  Total downloads:   {}", total);
    println!("  Weekly downloads:  {}", weekly);
    println!("  Monthly downloads: {}", monthly);

    // Extract versions array entries
    println!("\nVersions:");
    let mut search = body.as_str();
    while let Some(idx) = search.find("\"version\":") {
        search = &search[idx + 10..];
        if let Some(start) = search.find('"') {
            let inner = &search[start + 1..];
            if let Some(end) = inner.find('"') {
                let ver = &inner[..end];
                let yanked = search.contains("\"yanked\":1") || search.find("\"yanked\":1").map_or(false, |i| i < 60);
                if yanked {
                    println!("  {} (unpublished)", ver);
                } else {
                    println!("  {}", ver);
                }
            }
        }
    }
    println!();
    Ok(())
}

fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\":", key);
    let idx = json.find(&needle)?;
    let after = json[idx + needle.len()..].trim_start();
    let end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
    after[..end].parse().ok()
}

/// Zip all .sz files in dir into an in-memory buffer.
fn create_package_zip(dir: &Path) -> Result<Vec<u8>, String> {
    use std::io::Write;

    let patterns = read_szignore(dir);
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_package_files(dir, dir, &patterns, &mut files)?;
    files.sort();

    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (rel, path) in &files {
        zip.start_file(rel, options)
            .map_err(|e| format!("Zip error on '{}': {}", rel, e))?;
        let content = std::fs::read(path)
            .map_err(|e| format!("Cannot read '{}': {}", rel, e))?;
        zip.write_all(&content)
            .map_err(|e| format!("Zip write error: {}", e))?;
    }

    let buf = zip.finish().map_err(|e| format!("Zip finish error: {}", e))?;
    Ok(buf.into_inner())
}

/// Read patterns from `<dir>/.szignore` (gitignore-like). Blank lines and lines
/// starting with `#` are skipped. Returns an empty list if the file is absent.
fn read_szignore(dir: &Path) -> Vec<String> {
    match std::fs::read_to_string(dir.join(".szignore")) {
        Ok(s) => s
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Recursively collect files under `dir` (relative to `root`), skipping ignored
/// paths, `.git/` and the `.szignore` file itself. Fills `out` with
/// (relative_path_with_forward_slashes, absolute_path).
fn collect_package_files(
    root: &Path,
    dir: &Path,
    patterns: &[String],
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Directory read error: {}", e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Never publish VCS metadata or the ignore file itself.
        if name == ".git" || name == ".szignore" {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if is_ignored(&rel, patterns) {
            continue;
        }

        if path.is_dir() {
            collect_package_files(root, &path, patterns, out)?;
        } else if path.is_file() {
            out.push((rel, path));
        }
    }
    Ok(())
}

/// gitignore-like match. A pattern may be an exact name/path, a directory
/// (`apps/` or `/apps/`) which excludes it and everything under it, or a glob
/// with `*` (e.g. `*.txt`).
fn is_ignored(rel: &str, patterns: &[String]) -> bool {
    let rel = rel.trim_start_matches("./");
    for pat in patterns {
        let p = pat.trim_start_matches("./").trim_start_matches('/');
        if p.is_empty() {
            continue;
        }
        // Directory pattern: "apps/" → the dir and everything beneath it.
        if let Some(d) = p.strip_suffix('/') {
            if rel == d
                || rel.starts_with(&format!("{}/", d))
                || rel.split('/').any(|seg| seg == d)
            {
                return true;
            }
            continue;
        }
        // Glob pattern: matched against the basename and the full relative path.
        if p.contains('*') {
            let base = rel.rsplit('/').next().unwrap_or(rel);
            if glob_match(p, base) || glob_match(p, rel) {
                return true;
            }
            continue;
        }
        // Plain name/path: exact path, basename anywhere, or as a directory prefix.
        let base = rel.rsplit('/').next().unwrap_or(rel);
        if rel == p
            || base == p
            || rel.starts_with(&format!("{}/", p))
            || rel.split('/').any(|seg| seg == p)
        {
            return true;
        }
    }
    false
}

/// Minimal wildcard matcher supporting `*` (matches any run of characters).
fn glob_match(pattern: &str, text: &str) -> bool {
    fn helper(p: &[u8], t: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p[0] == b'*' {
            helper(&p[1..], t) || (!t.is_empty() && helper(p, &t[1..]))
        } else if !t.is_empty() && p[0] == t[0] {
            helper(&p[1..], &t[1..])
        } else {
            false
        }
    }
    helper(pattern.as_bytes(), text.as_bytes())
}

/// Extract a zip archive into dest/, skipping any top-level directory wrapper.
fn extract_zip(data: &[u8], dest: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid zip archive: {}", e))?;

    // Detect a single common top-level wrapper dir (e.g. "serez-ai-1.0.0/") shared
    // by ALL entries, and strip it. If entries don't share one prefix (files at the
    // root plus subdirs like src/), strip nothing — preserve the layout as-is.
    let names: Vec<String> = (0..archive.len())
        .map(|i| {
            archive
                .by_index(i)
                .map(|f| f.name().to_string())
                .unwrap_or_default()
        })
        .collect();
    let prefix: Option<String> = names
        .first()
        .and_then(|name| name.find('/').map(|i| name[..=i].to_string()))
        .filter(|p| names.iter().all(|n| n.starts_with(p.as_str())));

    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Cannot create destination: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Zip read error at entry {}: {}", i, e))?;

        let raw_name = file.name().to_string();

        // Security: reject path traversal
        if raw_name.contains("..") {
            return Err(format!("Unsafe path in archive: '{}'", raw_name));
        }

        // Strip top-level prefix if present
        let rel = if let Some(ref pfx) = prefix {
            raw_name.strip_prefix(pfx.as_str()).unwrap_or(&raw_name)
        } else {
            &raw_name
        };

        if rel.is_empty() {
            continue;
        }

        let outpath = dest.join(rel);

        if raw_name.ends_with('/') {
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Cannot create dir '{}': {}", outpath.display(), e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create dir '{}': {}", parent.display(), e))?;
            }
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .map_err(|e| format!("Cannot read zip entry '{}': {}", raw_name, e))?;
            std::fs::write(&outpath, content)
                .map_err(|e| format!("Cannot write '{}': {}", outpath.display(), e))?;
        }
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_pkg_spec(spec: &str) -> (String, Option<String>) {
    if let Some(idx) = spec.find('@') {
        (spec[..idx].to_string(), Some(spec[idx + 1..].to_string()))
    } else {
        (spec.to_string(), None)
    }
}

fn find_latest_version(pkg_reg_dir: &Path) -> Option<String> {
    std::fs::read_dir(pkg_reg_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .max()
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    // Try HOME (Unix) or USERPROFILE (Windows)
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

// ── JSON mini-parser helpers ──────────────────────────────────────────────────

fn read_json_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    // expects '"' as next char
    if chars.next() != Some('"') {
        return Err("Expected '\"'".to_string());
    }
    let mut s = String::new();
    loop {
        match chars.next() {
            None | Some('\0') => return Err("Unterminated string".to_string()),
            Some('"') => break,
            Some('\\') => {
                match chars.next() {
                    Some('"')  => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('n')  => s.push('\n'),
                    Some('t')  => s.push('\t'),
                    Some('r')  => s.push('\r'),
                    Some(c)    => { s.push('\\'); s.push(c); }
                    None       => return Err("Unterminated escape".to_string()),
                }
            }
            Some(c) => s.push(c),
        }
    }
    Ok(s)
}

fn skip_ws_and(chars: &mut std::iter::Peekable<std::str::Chars>, expect: char) {
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
    if chars.peek() == Some(&expect) {
        chars.next();
    }
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
}

fn parse_string_map(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<HashMap<String, String>, String> {
    // Consume '{'
    if chars.next() != Some('{') {
        return Err("Expected '{'".to_string());
    }
    let mut map = HashMap::new();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek() {
            None | Some('}') => { chars.next(); break; }
            Some('"') => {}
            _ => break,
        }
        let key = read_json_string(chars)?;
        skip_ws_and(chars, ':');
        let val = read_json_string(chars)?;
        map.insert(key, val);
    }
    Ok(map)
}

fn parse_string_array(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<Vec<String>, String> {
    if chars.next() != Some('[') {
        return Err("Expected '['".to_string());
    }
    let mut arr = Vec::new();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek() {
            None | Some(']') => { chars.next(); break; }
            Some('"') => {}
            _ => break,
        }
        arr.push(read_json_string(chars)?);
    }
    Ok(arr)
}

fn skip_value(chars: &mut std::iter::Peekable<std::str::Chars>) {
    // Minimal skip: handles strings, numbers, booleans, null, nested {}
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
    match chars.peek() {
        Some('"') => { let _ = read_json_string(chars); }
        Some('{') => {
            let mut depth = 0i32;
            for c in chars.by_ref() {
                if c == '{' { depth += 1; }
                if c == '}' { depth -= 1; if depth == 0 { break; } }
            }
        }
        Some('[') => {
            let mut depth = 0i32;
            for c in chars.by_ref() {
                if c == '[' { depth += 1; }
                if c == ']' { depth -= 1; if depth == 0 { break; } }
            }
        }
        _ => {
            while chars.peek().map_or(false, |c| !matches!(c, ',' | '}' | ']')) {
                chars.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parse() {
        let json = r#"{
          "name": "my-pkg",
          "version": "1.2.3",
          "dependencies": {
            "pkg-a": "0.1.0",
            "pkg-b": "2.0.0"
          }
        }"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.name, "my-pkg");
        assert_eq!(m.version, "1.2.3");
        assert_eq!(m.dependencies["pkg-a"], "0.1.0");
        assert_eq!(m.dependencies["pkg-b"], "2.0.0");
    }

    #[test]
    fn test_manifest_parse_bin() {
        let json = r#"{
          "name": "serez-apipack",
          "version": "1.1.0",
          "permissions": ["OS", "File", "Env"],
          "bin": { "apipack": "pack.sz" }
        }"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.bin["apipack"], "pack.sz");
        assert_eq!(m.permissions, vec!["OS", "File", "Env"]);
    }

    #[test]
    fn test_manifest_no_bin_is_empty() {
        let json = r#"{"name":"x","version":"1.0.0"}"#;
        let m = SerezManifest::parse(json).unwrap();
        assert!(m.bin.is_empty());
    }

    #[test]
    fn test_manifest_no_deps() {
        let json = r#"{"name":"simple","version":"1.0.0","dependencies":{}}"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.name, "simple");
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_manifest_scripts_parsed() {
        let json = r#"{
          "name": "my-app",
          "version": "1.0.0",
          "scripts": {
            "dev": "sz index.sz",
            "build": "sz apipack build"
          },
          "dependencies": {}
        }"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.scripts["dev"], "sz index.sz");
        assert_eq!(m.scripts["build"], "sz apipack build");
    }

    #[test]
    fn test_manifest_no_scripts_defaults_empty() {
        let json = r#"{"name":"no-scripts","version":"1.0.0"}"#;
        let m = SerezManifest::parse(json).unwrap();
        assert!(m.scripts.is_empty());
    }

    #[test]
    fn test_manifest_scripts_and_deps_coexist() {
        let json = r#"{
          "name": "full",
          "version": "2.0.0",
          "scripts": { "dev": "sz main.sz" },
          "dependencies": { "serez-http": "1.0.0" }
        }"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.scripts["dev"], "sz main.sz");
        assert_eq!(m.dependencies["serez-http"], "1.0.0");
    }

    #[test]
    fn test_pkg_spec_with_version() {
        let (name, ver) = parse_pkg_spec("foo@1.2.3");
        assert_eq!(name, "foo");
        assert_eq!(ver, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_pkg_spec_without_version() {
        let (name, ver) = parse_pkg_spec("foo");
        assert_eq!(name, "foo");
        assert_eq!(ver, None);
    }

    #[test]
    fn test_upsert_into_empty_deps() {
        let raw = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\",\n  \"dependencies\": {},\n  \"permissions\": []\n}\n";
        let out = upsert_dependency(raw, "serez-http", "1.2.0").unwrap();
        let m = SerezManifest::parse(&out).unwrap();
        assert_eq!(m.dependencies["serez-http"], "1.2.0");
        // The rest of the manifest survives untouched.
        assert_eq!(m.name, "app");
        assert_eq!(m.version, "1.0.0");
        assert!(out.contains("\"permissions\": []"));
    }

    #[test]
    fn test_upsert_appends_to_existing_deps() {
        let raw = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\",\n  \"dependencies\": {\n    \"a\": \"0.1.0\"\n  }\n}\n";
        let out = upsert_dependency(raw, "b", "2.0.0").unwrap();
        let m = SerezManifest::parse(&out).unwrap();
        assert_eq!(m.dependencies["a"], "0.1.0");
        assert_eq!(m.dependencies["b"], "2.0.0");
    }

    #[test]
    fn test_upsert_updates_existing_version() {
        let raw = "{\"name\":\"app\",\"version\":\"1.0.0\",\"dependencies\":{\"a\":\"0.1.0\"}}";
        let out = upsert_dependency(raw, "a", "0.2.0").unwrap();
        let m = SerezManifest::parse(&out).unwrap();
        assert_eq!(m.dependencies["a"], "0.2.0");
        assert_eq!(m.dependencies.len(), 1);
    }

    #[test]
    fn test_upsert_inserts_missing_deps_key() {
        let raw = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\"\n}\n";
        let out = upsert_dependency(raw, "a", "1.0.0").unwrap();
        let m = SerezManifest::parse(&out).unwrap();
        assert_eq!(m.name, "app");
        assert_eq!(m.dependencies["a"], "1.0.0");
    }

    #[test]
    fn test_upsert_preserves_scripts_block() {
        let raw = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\",\n  \"scripts\": {\n    \"dev\": \"sz index.sz\"\n  },\n  \"dependencies\": {}\n}\n";
        let out = upsert_dependency(raw, "serez-ui", "1.1.0").unwrap();
        let m = SerezManifest::parse(&out).unwrap();
        assert_eq!(m.scripts["dev"], "sz index.sz");
        assert_eq!(m.dependencies["serez-ui"], "1.1.0");
    }

    #[test]
    fn test_find_object_span_ignores_braces_in_strings() {
        // A string value containing '{' must not confuse brace matching.
        let raw = "{\"description\":\"a { brace\",\"dependencies\":{\"x\":\"1.0.0\"}}";
        let (start, end) = find_object_span(raw, "dependencies").unwrap();
        let body = &raw[start..=end];
        assert_eq!(body, "{\"x\":\"1.0.0\"}");
    }

    #[test]
    fn test_remove_pair_roundtrip() {
        let raw = "{\"name\":\"app\",\"version\":\"1.0.0\",\"dependencies\":{\"a\":\"1.0.0\",\"b\":\"2.0.0\"}}";
        let (start, end) = find_object_span(raw, "dependencies").unwrap();
        let mut pairs = parse_ordered_pairs(&raw[start + 1..end]).unwrap();
        pairs.retain(|(k, _)| k != "a");
        let rendered = render_deps_object(&pairs);
        let out = format!("{}{}{}", &raw[..start], rendered, &raw[end + 1..]);
        let m = SerezManifest::parse(&out).unwrap();
        assert!(!m.dependencies.contains_key("a"));
        assert_eq!(m.dependencies["b"], "2.0.0");
    }
}
