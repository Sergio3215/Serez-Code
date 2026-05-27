// Serez package manager
//
// Package layout on disk:
//   $SEREZ_PACKAGES/   (or ~/.serez/packages/)
//     <name>/
//       index.sz        ← default entry point
//       <submod>.sz     ← named submodule
//
// serez.json (project manifest):
//   { "name": "...", "version": "...", "dependencies": { "pkg": "version", ... } }
//
// Registry layout (SEREZ_REGISTRY env var or ~/.serez/registry/):
//   <name>/
//     <version>/
//       index.sz
//
// HTTP registry (SEREZ_REGISTRY_URL env var or https://registry.serezcode.org):
//   GET /packages/<name>/latest        → plain-text version string
//   GET /packages/<name>/<version>.zip → zip archive of package files

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

// ── Manifest ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SerezManifest {
    pub name: String,
    pub version: String,
    pub dependencies: HashMap<String, String>,
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
        let mut dependencies: HashMap<String, String> = HashMap::new();

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
                        "name"    => name = val,
                        "version" => version = val,
                        _         => {} // unknown fields ignored
                    }
                }
                Some('{') => {
                    if key == "dependencies" {
                        dependencies = parse_string_map(&mut chars)?;
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
        Ok(SerezManifest { name, version, dependencies })
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
        .unwrap_or_else(|_| "https://registry.serezcode.org".to_string())
}

/// Install a package from the local registry or HTTP registry into ./packages/.
/// pkg_spec = "name" or "name@version".
pub fn install_package(pkg_spec: &str) -> Result<(), String> {
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

    let dest = local_packages_dir().join(&pkg_name);
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
        println!("✅ Installed {}@{} → ./packages/{}", pkg_name, version, pkg_name);
    } else {
        download_package(&pkg_name, &version)?;
    }

    Ok(())
}

/// Remove a package from the local project packages directory.
pub fn uninstall_package(pkg_name: &str) -> Result<(), String> {
    let dest = local_packages_dir().join(pkg_name);
    if !dest.exists() {
        return Err(format!("Package '{}' is not installed in ./packages/", pkg_name));
    }
    std::fs::remove_dir_all(&dest)
        .map_err(|e| format!("Failed to uninstall '{}': {}", pkg_name, e))?;
    println!("✅ Uninstalled {}", pkg_name);
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
        install_package(&spec)?;
    }
    Ok(())
}

// ── HTTP registry ─────────────────────────────────────────────────────────────

/// Fetch the latest version string for a package from the HTTP registry.
/// GET <registry_url>/packages/<name>/latest → plain-text version e.g. "1.0.0"
fn fetch_latest_version(pkg_name: &str) -> Result<String, String> {
    let url = format!("{}/packages/{}/latest", registry_url(), pkg_name);
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
/// GET <registry_url>/packages/<name>/<version>.zip
fn download_package(pkg_name: &str, version: &str) -> Result<(), String> {
    let url = format!("{}/packages/{}/{}.zip", registry_url(), pkg_name, version);
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

    let dest = local_packages_dir().join(pkg_name);
    extract_zip(&bytes, &dest)
        .map_err(|e| format!("Failed to extract '{}@{}': {}", pkg_name, version, e))?;

    println!("✅ Installed {}@{} → ./packages/{} (remote)", pkg_name, version, pkg_name);
    Ok(())
}

/// Extract a zip archive into dest/, skipping any top-level directory wrapper.
fn extract_zip(data: &[u8], dest: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid zip archive: {}", e))?;

    // Detect common top-level prefix (e.g. "serez-ai-1.0.0/") to strip it
    let prefix: Option<String> = (archive.len() > 0)
        .then(|| {
            archive
                .by_index(0)
                .ok()
                .and_then(|f| {
                    let name = f.name().to_string();
                    name.find('/').map(|i| name[..=i].to_string())
                })
        })
        .flatten();

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
    fn test_manifest_no_deps() {
        let json = r#"{"name":"simple","version":"1.0.0","dependencies":{}}"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.name, "simple");
        assert!(m.dependencies.is_empty());
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
}
