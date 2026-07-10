use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use zed_extension_api as zed;
use zed_extension_api::{DownloadedFileType, LanguageServerInstallationStatus};

pub(crate) const VISUALFORCE_LSP_ID: &str = "visualforce-language-server";
const VISUALFORCE_LSP_RELEASE: &str = "v67.4.0";
const VISUALFORCE_LSP_BUNDLE_SHA256: &str =
    "37f6808e5e4bd360f7c7f219fd2d71cc8d7ce22688b271c1a4ae5020bd85bb3f";
const VISUALFORCE_LSP_DOWNLOAD_URL: &str = "https://github.com/forcedotcom/salesforcedx-vscode/releases/download/v67.4.0/salesforcedx-vscode-visualforce-67.4.0.vsix";
const VISUALFORCE_LSP_CACHE_REL_PATH: &str = "lsp/visualforce-language-server/v67.4.0";
const VISUALFORCE_LSP_SERVER_REL_PATH: &str = "extension/dist/visualforceServer.js";

#[derive(Debug, PartialEq, Eq)]
enum BundleVerification {
    Missing,
    Valid,
    Invalid { actual: String },
}

pub(crate) fn language_server_command(
    language_server_id: &zed::LanguageServerId,
    worktree: &zed::Worktree,
) -> zed::Result<zed::Command> {
    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::CheckingForUpdate,
    );

    let node = zed::node_binary_path().map_err(|err| {
        fail_installation(
            language_server_id,
            format!("Could not resolve Zed's Node.js runtime for Visualforce: {err}"),
        )
    })?;
    let work_dir = std::env::current_dir().map_err(|err| {
        fail_installation(
            language_server_id,
            format!("Could not get the extension work directory for Visualforce: {err}"),
        )
    })?;

    let server_path = ensure_verified_bundle_with(
        &work_dir,
        VISUALFORCE_LSP_BUNDLE_SHA256,
        |url, destination| {
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Downloading,
            );
            zed::download_file(url, destination, DownloadedFileType::Zip).map_err(|err| {
                format!(
                    "Failed to download and extract Visualforce Language Server {VISUALFORCE_LSP_RELEASE} from {url}: {err}"
                )
            })
        },
    )
    .map_err(|err| fail_installation(language_server_id, err))?;

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::None,
    );

    Ok(build_command(
        node,
        server_path.to_string_lossy().into_owned(),
        worktree.shell_env(),
    ))
}

pub(crate) fn initialization_options() -> zed::serde_json::Value {
    zed::serde_json::json!({
        "embeddedLanguages": {
            "css": true,
            "javascript": true
        }
    })
}

fn fail_installation(language_server_id: &zed::LanguageServerId, message: String) -> String {
    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::Failed(message.clone()),
    );
    message
}

fn build_command(node: String, server_path: String, env: zed::EnvVars) -> zed::Command {
    zed::Command {
        command: node,
        args: vec![server_path, "--stdio".to_string()],
        env,
    }
}

fn bundle_path(work_dir: &Path) -> PathBuf {
    work_dir
        .join(VISUALFORCE_LSP_CACHE_REL_PATH)
        .join(VISUALFORCE_LSP_SERVER_REL_PATH)
}

fn ensure_verified_bundle_with<F>(
    work_dir: &Path,
    expected_sha256: &str,
    mut download: F,
) -> Result<PathBuf, String>
where
    F: FnMut(&str, &str) -> Result<(), String>,
{
    let server_path = bundle_path(work_dir);
    if verify_bundle(&server_path, expected_sha256)? == BundleVerification::Valid {
        return Ok(server_path);
    }

    let version_dir = work_dir.join(VISUALFORCE_LSP_CACHE_REL_PATH);
    remove_visualforce_version_cache(&version_dir)?;
    if let Some(parent) = version_dir.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Could not create Visualforce Language Server cache parent {}: {err}",
                parent.display()
            )
        })?;
    }

    download(VISUALFORCE_LSP_DOWNLOAD_URL, VISUALFORCE_LSP_CACHE_REL_PATH)?;

    match verify_bundle(&server_path, expected_sha256)? {
        BundleVerification::Valid => Ok(server_path),
        BundleVerification::Invalid { actual } => Err(format!(
            "Visualforce Language Server integrity verification failed for {}: expected {expected_sha256}; actual {actual}. Remove {} and retry.",
            server_path.display(),
            version_dir.display()
        )),
        BundleVerification::Missing => Err(format!(
            "Visualforce Language Server integrity verification failed for {}: expected {expected_sha256}; actual missing. The VSIX did not contain {}. Remove {} and retry.",
            server_path.display(),
            VISUALFORCE_LSP_SERVER_REL_PATH,
            version_dir.display()
        )),
    }
}

fn remove_visualforce_version_cache(version_dir: &Path) -> Result<(), String> {
    if !version_dir.exists() {
        return Ok(());
    }

    if version_dir.is_dir() {
        fs::remove_dir_all(version_dir)
    } else {
        fs::remove_file(version_dir)
    }
    .map_err(|err| {
        format!(
            "Could not clear Visualforce Language Server version cache {}: {err}",
            version_dir.display()
        )
    })
}

fn verify_bundle(path: &Path, expected_sha256: &str) -> Result<BundleVerification, String> {
    if !path.is_file() {
        return Ok(BundleVerification::Missing);
    }

    let actual = sha256_file(path)?;
    if actual == expected_sha256 {
        Ok(BundleVerification::Valid)
    } else {
        Ok(BundleVerification::Invalid { actual })
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|err| {
        format!(
            "Could not open {} for SHA-256 verification: {err}",
            path.display()
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| {
            format!(
                "Could not read {} for SHA-256 verification: {err}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{
        build_command, bundle_path, ensure_verified_bundle_with, initialization_options,
        verify_bundle, BundleVerification, VISUALFORCE_LSP_CACHE_REL_PATH,
        VISUALFORCE_LSP_DOWNLOAD_URL, VISUALFORCE_LSP_ID, VISUALFORCE_LSP_SERVER_REL_PATH,
    };
    use std::cell::Cell;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn selects_versioned_visualforce_bundle_path() {
        let root = temp_test_root("path");

        assert_eq!(
            bundle_path(&root),
            root.join(VISUALFORCE_LSP_CACHE_REL_PATH)
                .join(VISUALFORCE_LSP_SERVER_REL_PATH)
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn verifies_known_sha256_and_reports_mismatch() {
        let root = temp_test_root("hash");
        let path = root.join("server.js");
        fs::write(&path, b"abc").unwrap();

        assert_eq!(
            verify_bundle(&path, ABC_SHA256).unwrap(),
            BundleVerification::Valid
        );
        assert_eq!(
            verify_bundle(
                &path,
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
            .unwrap(),
            BundleVerification::Invalid {
                actual: ABC_SHA256.to_string()
            }
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reuses_valid_cached_bundle_without_downloading() {
        let root = temp_test_root("reuse");
        let path = bundle_path(&root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"abc").unwrap();
        let calls = Cell::new(0);

        let resolved = ensure_verified_bundle_with(&root, ABC_SHA256, |_, _| {
            calls.set(calls.get() + 1);
            Err("download must not run".to_string())
        })
        .unwrap();

        assert_eq!(resolved, path);
        assert_eq!(calls.get(), 0);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn replaces_corrupt_version_cache_with_one_download_attempt() {
        let root = temp_test_root("repair");
        let path = bundle_path(&root);
        let version_dir = root.join(VISUALFORCE_LSP_CACHE_REL_PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"corrupt").unwrap();
        fs::write(version_dir.join("stale.txt"), b"remove me").unwrap();
        let calls = Cell::new(0);

        let resolved = ensure_verified_bundle_with(&root, ABC_SHA256, |url, destination| {
            calls.set(calls.get() + 1);
            assert_eq!(url, VISUALFORCE_LSP_DOWNLOAD_URL);
            assert_eq!(destination, VISUALFORCE_LSP_CACHE_REL_PATH);
            let downloaded = root.join(destination).join(VISUALFORCE_LSP_SERVER_REL_PATH);
            fs::create_dir_all(downloaded.parent().unwrap()).unwrap();
            fs::write(downloaded, b"abc").unwrap();
            Ok(())
        })
        .unwrap();

        assert_eq!(resolved, path);
        assert_eq!(calls.get(), 1);
        assert!(!version_dir.join("stale.txt").exists());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reports_expected_and_actual_hash_after_failed_repair() {
        let root = temp_test_root("failed-repair");
        let calls = Cell::new(0);

        let error = ensure_verified_bundle_with(&root, ABC_SHA256, |_, destination| {
            calls.set(calls.get() + 1);
            let downloaded = root.join(destination).join(VISUALFORCE_LSP_SERVER_REL_PATH);
            fs::create_dir_all(downloaded.parent().unwrap()).unwrap();
            fs::write(downloaded, b"wrong bundle").unwrap();
            Ok(())
        })
        .unwrap_err();

        assert_eq!(calls.get(), 1);
        assert!(error.contains(&format!("expected {ABC_SHA256}")));
        assert!(error.contains("actual "));
        assert!(error.contains(&bundle_path(&root).display().to_string()));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn builds_node_stdio_command_with_worktree_environment() {
        let env = vec![("SHELL_VALUE".to_string(), "kept".to_string())];
        let command = build_command(
            "/zed/node".to_string(),
            "/cache/extension/dist/visualforceServer.js".to_string(),
            env.clone(),
        );

        assert_eq!(VISUALFORCE_LSP_ID, "visualforce-language-server");
        assert_eq!(command.command, "/zed/node");
        assert_eq!(
            command.args,
            vec![
                "/cache/extension/dist/visualforceServer.js".to_string(),
                "--stdio".to_string()
            ]
        );
        assert_eq!(command.env, env);
    }

    #[test]
    fn enables_embedded_css_and_javascript() {
        assert_eq!(
            initialization_options(),
            zed_extension_api::serde_json::json!({
                "embeddedLanguages": {
                    "css": true,
                    "javascript": true
                }
            })
        );
    }

    fn temp_test_root(name: &str) -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "zed-salesforce-visualforce-{name}-{}-{now}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
