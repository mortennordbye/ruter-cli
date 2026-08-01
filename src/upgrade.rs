//! `ruter upgrade` — check GitHub for a newer release and install it.
//!
//! The install itself is delegated to `scripts/install.sh` from `main`, which is the
//! same script the README tells people to curl. Reimplementing the download, checksum,
//! app bundle and codesign dance in Rust would duplicate logic that has to stay correct
//! in the shell script anyway.

use anyhow::{Context, Result, anyhow, bail};
use std::process::Command;
use std::time::Duration;

const REPO: &str = "mortennordbye/ruter-cli";
const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/mortennordbye/ruter-cli/main/scripts/install.sh";

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

pub fn run(check_only: bool) -> Result<()> {
    let latest = latest_release()?;
    let latest_version = latest.trim_start_matches('v');

    println!("Installert versjon: {CURRENT}");
    println!("Nyeste versjon:     {latest_version}");

    if !is_newer(latest_version, CURRENT) {
        println!("\nDu har allerede nyeste versjon.");
        return Ok(());
    }

    if check_only {
        println!("\nNy versjon tilgjengelig. Oppgrader med `ruter upgrade`.");
        return Ok(());
    }

    println!("\n==> Oppgraderer til {latest_version}");
    install(&latest)?;
    warn_if_shadowed();
    Ok(())
}

/// The install script always writes under `$HOME`. If the binary being replaced came from
/// somewhere else — Homebrew, `cargo install`, `/usr/local/bin` — the upgrade succeeds and
/// still leaves the old version on PATH, which otherwise looks like nothing happened.
fn warn_if_shadowed() {
    let bin_dir = std::env::var_os("RUTER_BIN_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/bin")));

    let (Some(bin_dir), Ok(running)) = (bin_dir, std::env::current_exe()) else { return };

    let resolve = |p: std::path::PathBuf| std::fs::canonicalize(p).ok();
    let (Some(installed), Some(running)) = (resolve(bin_dir.join("ruter")), resolve(running))
    else {
        return;
    };

    if installed != running {
        println!(
            "\nOBS: du kjørte {}, men den nye versjonen ble installert i {}.\n\
             Fjern den gamle, eller sørg for at den nye kommer først i PATH.",
            running.display(),
            installed.display()
        );
    }
}

/// Ask GitHub which tag `releases/latest` points at.
///
/// Reads the redirect target rather than the JSON API: no parsing, and it does not
/// spend the unauthenticated API rate limit that a shared IP can easily exhaust.
fn latest_release() -> Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest");

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("ruter-cli/", env!("CARGO_PKG_VERSION")))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .new_agent();

    let response = agent
        .get(&url)
        .call()
        .map_err(|e| anyhow!("nådde ikke GitHub for å sjekke etter oppdateringer: {e}"))?;

    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .context("GitHub svarte uventet — fant ingen release å sammenligne med")?;

    let tag = location
        .rsplit_once("/tag/")
        .map(|(_, tag)| tag)
        .filter(|tag| !tag.is_empty())
        .context("klarte ikke lese ut siste versjon fra GitHub")?;

    Ok(tag.to_string())
}

/// Run the published install script, pinned to the version we just resolved.
fn install(tag: &str) -> Result<()> {
    let script = fetch_installer()?;

    let mut child = Command::new("sh")
        .arg("-s")
        .env("RUTER_VERSION", tag)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("klarte ikke kjøre installasjonsskriptet")?;

    // The handle has to be dropped before waiting. `sh -s` reads its script from stdin
    // and keeps reading until EOF, so holding the pipe open across `wait()` deadlocks:
    // the install runs to completion and the process then hangs forever.
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin was piped");
        stdin
            .write_all(script.as_bytes())
            .context("klarte ikke sende installasjonsskriptet til sh")?;
    }

    let status = child.wait().context("klarte ikke kjøre installasjonsskriptet")?;

    if !status.success() {
        bail!("installasjonsskriptet feilet. Prøv manuelt:\n  curl -fsSL {INSTALL_URL} | sh");
    }
    Ok(())
}

fn fetch_installer() -> Result<String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("ruter-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();

    agent
        .get(INSTALL_URL)
        .call()
        .map_err(|e| anyhow!("kunne ikke hente installasjonsskriptet: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow!("kunne ikke lese installasjonsskriptet: {e}"))
}

/// Compare two `X.Y.Z` strings. Anything unparseable sorts as 0, so a malformed
/// tag can only ever look older — it never triggers an upgrade on its own.
fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

fn parts(version: &str) -> (u64, u64, u64) {
    let mut it = version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|n| n.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_newer_versions() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn same_or_older_is_not_newer() {
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
    }

    #[test]
    fn a_malformed_tag_never_looks_newer() {
        assert!(!is_newer("latest", "0.2.0"));
        assert!(!is_newer("", "0.2.0"));
    }

    #[test]
    fn prerelease_suffixes_are_ignored() {
        assert_eq!(parts("0.3.0-rc.1"), (0, 3, 0));
    }
}
