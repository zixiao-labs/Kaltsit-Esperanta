//! Managed CEF install under the application data directory.
//!
//! Production builds do not ship `libcef` inside the app bundle. Users (or the
//! auto-update component hook) download a pinned CEF artifact into
//! `paths::data_dir()/cef/<version>/`, which [`probe_libcef_path`] prefers.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow, bail};
use async_compression::futures::bufread::{BzDecoder, GzipDecoder};
use futures::io::BufReader;
use http_client::{AsyncBody, HttpClient};
use smol::fs;

/// Pinned CEF package version for managed installs.
pub const MANAGED_CEF_VERSION: &str = "131.3.5+g437feba+chromium-131.0.6778.205";

const DOWNLOAD_URL_ENV: &str = "ZETA_CEF_DOWNLOAD_URL";

/// Resolve the managed install root: `data_dir/cef/<version>`.
pub fn managed_cef_root() -> PathBuf {
    paths::data_dir()
        .join("cef")
        .join(sanitize_version(MANAGED_CEF_VERSION))
}

fn sanitize_version(version: &str) -> String {
    version
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Platform-specific relative path to the loadable CEF library inside a managed tree.
pub fn managed_libcef_relative_path() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Chromium Embedded Framework.framework/Chromium Embedded Framework"
    }
    #[cfg(target_os = "linux")]
    {
        "libcef.so"
    }
    #[cfg(target_os = "windows")]
    {
        "libcef.dll"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "libcef"
    }
}

/// Return the managed libcef path when the file already exists.
pub fn probe_managed_libcef_path() -> Option<PathBuf> {
    let path = managed_cef_root().join(managed_libcef_relative_path());
    path.is_file().then_some(path)
}

/// Default download URL for the pinned CEF artifact (overridable via env).
pub fn default_cef_download_url() -> Result<String> {
    if let Ok(url) = env::var(DOWNLOAD_URL_ENV) {
        if url.trim().is_empty() {
            bail!("{DOWNLOAD_URL_ENV} is set but empty");
        }
        return Ok(url);
    }

    let (os, arch, ext) = cef_artifact_triple()?;
    // Hosted next to other Zeta optional runtimes; override with ZETA_CEF_DOWNLOAD_URL
    // when mirroring to a private blob store.
    Ok(format!(
        "https://zed-cef-releases.nyc3.digitaloceanspaces.com/cef_{}_{}_{}.{}",
        sanitize_version(MANAGED_CEF_VERSION),
        os,
        arch,
        ext
    ))
}

fn cef_artifact_triple() -> Result<(&'static str, &'static str, &'static str)> {
    let os = match env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        other => bail!("unsupported OS for managed CEF: {other}"),
    };
    let arch = match env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => bail!("unsupported arch for managed CEF: {other}"),
    };
    let ext = match os {
        "windows" => "zip",
        _ => "tar.bz2",
    };
    Ok((os, arch, ext))
}

/// Ensure the pinned CEF runtime is present, downloading when missing.
///
/// Used by the explicit **Install Browser Runtime** action.
pub async fn install_if_needed(http: Arc<dyn HttpClient>) -> Result<PathBuf> {
    if let Some(path) = probe_managed_libcef_path() {
        log::info!("Managed CEF already installed at {}", path.display());
        return Ok(path);
    }

    download_and_extract(http).await
}

/// Component auto-update path: refresh only if the user previously installed CEF.
///
/// Fresh installs never auto-download; that keeps component updates off the
/// full-application auto-update path and avoids surprise large downloads.
pub async fn refresh_if_installed(http: Arc<dyn HttpClient>) -> Result<Option<PathBuf>> {
    let cef_root = paths::data_dir().join("cef");
    if !cef_root.exists() {
        return Ok(None);
    }
    if let Some(path) = probe_managed_libcef_path() {
        return Ok(Some(path));
    }
    Ok(Some(download_and_extract(http).await?))
}

async fn download_and_extract(http: Arc<dyn HttpClient>) -> Result<PathBuf> {
    let url = default_cef_download_url()?;
    log::info!("Downloading managed CEF from {url}");

    let root = managed_cef_root();
    let parent = root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| paths::data_dir().join("cef"));
    fs::create_dir_all(&parent)
        .await
        .with_context(|| format!("creating {}", parent.display()))?;

    if root.exists() {
        fs::remove_dir_all(&root)
            .await
            .with_context(|| format!("removing stale {}", root.display()))?;
    }
    fs::create_dir_all(&root)
        .await
        .with_context(|| format!("creating {}", root.display()))?;

    let mut response = http
        .get(&url, AsyncBody::default(), true)
        .await
        .with_context(|| format!("downloading CEF from {url}"))?;
    if !response.status().is_success() {
        bail!(
            "CEF download failed with HTTP {} from {url}",
            response.status()
        );
    }

    let archive_path = parent.join(format!(
        "cef-{}-download.{}",
        sanitize_version(MANAGED_CEF_VERSION),
        cef_artifact_triple()?.2
    ));
    {
        let mut file = fs::File::create(&archive_path)
            .await
            .with_context(|| format!("creating {}", archive_path.display()))?;
        futures::io::copy(response.body_mut(), &mut file)
            .await
            .context("writing CEF archive")?;
    }

    extract_archive(&archive_path, &root).await?;
    let _ = fs::remove_file(&archive_path).await;

    let lib_path = root.join(managed_libcef_relative_path());
    if !lib_path.is_file() {
        bail!(
            "CEF archive extracted to {} but {} is missing",
            root.display(),
            lib_path.display()
        );
    }

    log::info!("Installed managed CEF at {}", lib_path.display());
    Ok(lib_path)
}

async fn extract_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)
        .await
        .with_context(|| format!("opening {}", archive_path.display()))?;
    let extension = archive_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();

    match extension {
        "bz2" => {
            let decoder = BzDecoder::new(BufReader::new(file));
            let archive = async_tar::Archive::new(decoder);
            archive
                .unpack(destination)
                .await
                .context("extracting CEF tar.bz2")?;
        }
        "gz" => {
            let decoder = GzipDecoder::new(BufReader::new(file));
            let archive = async_tar::Archive::new(decoder);
            archive
                .unpack(destination)
                .await
                .context("extracting CEF tar.gz")?;
        }
        "zip" => {
            // Keep the managed installer dependency-light: shell out on Windows.
            let status = smol::process::Command::new("tar")
                .args(["-xf"])
                .arg(archive_path)
                .arg("-C")
                .arg(destination)
                .status()
                .await
                .context("running tar to extract CEF zip")?;
            if !status.success() {
                return Err(anyhow!("tar failed extracting {}", archive_path.display()));
            }
        }
        other => bail!("unsupported CEF archive extension: {other}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_plus_and_spaces() {
        assert_eq!(
            sanitize_version("131.3.5+g437feba+chromium-131"),
            "131.3.5_g437feba_chromium-131"
        );
    }

    #[test]
    fn managed_root_includes_version() {
        let root = managed_cef_root();
        assert!(
            root.ends_with(sanitize_version(MANAGED_CEF_VERSION)),
            "{}",
            root.display()
        );
    }
}
