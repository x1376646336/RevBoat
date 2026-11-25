use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Credentials {
    pub(super) server: String,
    pub(super) username: String,
    pub(super) password: String,
}
static CREDENTIALS: OnceLock<RwLock<Vec<Credentials>>> = OnceLock::new();

fn credential_server(url: &tauri::Url) -> Option<String> {
    let host = url.host_str().filter(|host| !host.is_empty())?;
    match url.scheme() {
        "http" | "https" => Some(url.origin().ascii_serialization()),
        // svn:// has an opaque URL origin. Never use that origin ("null") as a
        // credential key: it would share passwords across unrelated servers.
        "svn" => {
            let host = host.to_ascii_lowercase();
            Some(match url.port() {
                None | Some(3690) => format!("svn://{host}"),
                Some(port) => format!("svn://{host}:{port}"),
            })
        }
        _ => None,
    }
}

pub fn configure_credentials(
    server: &str,
    username: String,
    password: String,
) -> Result<(), SvnError> {
    let credentials = validated_credentials(server, username, password)?;
    let server = credentials.server;
    let username = credentials.username;
    let password = credentials.password;
    let mut entries = CREDENTIALS
        .get_or_init(Default::default)
        .write()
        .map_err(|_| SvnError::CommandFailed("Credential lock unavailable".into()))?;
    entries.retain(|entry| entry.server != server || entry.username != username);
    entries.push(Credentials {
        server,
        username,
        password,
    });
    Ok(())
}

fn validated_credentials(
    server: &str,
    username: String,
    password: String,
) -> Result<Credentials, SvnError> {
    let url = tauri::Url::parse(server)
        .map_err(|_| SvnError::InvalidArguments("Invalid server URL".into()))?;
    let server = credential_server(&url)
        .ok_or_else(|| SvnError::InvalidArguments("Invalid server URL".into()))?;
    if username.trim().is_empty()
        || password.is_empty()
        || username.contains(['\r', '\n'])
        || password.contains(['\r', '\n'])
    {
        return Err(SvnError::InvalidArguments("Invalid SVN credentials".into()));
    }
    Ok(Credentials {
        server,
        username,
        password,
    })
}

fn url_username(url: &tauri::Url) -> String {
    super::operations::percent_decode(url.username()).unwrap_or_else(|| url.username().to_owned())
}

#[derive(serde::Serialize)]
pub struct SvnAccountInfo {
    pub server: String,
    pub username: String,
    pub explicit_username: bool,
    pub has_credentials: bool,
}

pub fn account_info(target: &str) -> Result<SvnAccountInfo, SvnError> {
    let url = tauri::Url::parse(target)
        .map_err(|_| SvnError::InvalidArguments("Invalid repository URL".into()))?;
    let server = credential_server(&url).ok_or_else(|| {
        SvnError::InvalidArguments(
            "This repository protocol does not use password authentication".into(),
        )
    })?;
    let username = url_username(&url);
    let entries = CREDENTIALS
        .get_or_init(Default::default)
        .read()
        .map_err(|_| SvnError::CommandFailed("Credential lock unavailable".into()))?
        .clone();
    let credentials = credentials_for(&entries, &[target.into()])
        .or_else(|| super::credential_store::load(&server, &username));
    Ok(SvnAccountInfo {
        server,
        explicit_username: !username.is_empty(),
        has_credentials: credentials.is_some(),
        username: credentials.map(|c| c.username).unwrap_or(username),
    })
}

async fn validate_account_with(
    executable: PathBuf,
    target: &str,
    username: String,
    password: String,
) -> Result<Credentials, SvnError> {
    let mut url = tauri::Url::parse(target)
        .map_err(|_| SvnError::InvalidArguments("Invalid repository URL".into()))?;
    let pinned = url_username(&url);
    if !pinned.is_empty() && pinned != username {
        return Err(SvnError::InvalidArguments("Repository URL specifies a different account; use a working copy checked out with the new account".into()));
    }
    let candidate = validated_credentials(target, username, password)?;
    let _ = url.set_password(None);
    // An isolated config prevents an existing SVN auth cache from making an
    // invalid candidate appear successful. The probe is read-only.
    struct Config(PathBuf);
    impl Drop for Config {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let config = Config(std::env::temp_dir().join(format!(
            "orcasvn-account-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )));
    std::fs::create_dir(&config.0).map_err(|e| SvnError::CommandFailed(e.to_string()))?;
    let config_path = config.0.to_string_lossy();
    let target = format!("{}@", url.as_str());
    execute_svn_with_credentials(
        executable,
        &[
            "info",
            "-r",
            "HEAD",
            "--xml",
            "--config-dir",
            &config_path,
            "--",
            &target,
        ],
        None,
        Some(candidate.clone()),
    )
    .await?;
    Ok(candidate)
}

pub async fn change_account(
    target: &str,
    username: String,
    password: String,
) -> Result<bool, SvnError> {
    let candidate =
        validate_account_with(current_svn_executable()?, target, username, password).await?;
    // A failed verification leaves both the session and saved account untouched.
    configure_credentials(
        &candidate.server,
        candidate.username.clone(),
        candidate.password.clone(),
    )?;
    tokio::task::spawn_blocking(move || super::credential_store::save(&candidate).unwrap_or(false))
        .await
        .map_err(|_| SvnError::CommandFailed("Credential storage unavailable".into()))
}

fn credentials_for(entries: &[Credentials], urls: &[String]) -> Option<Credentials> {
    if urls.is_empty() {
        return None;
    }
    entries
        .iter()
        .rev()
        .find(|entry| {
            urls.iter().all(|value| {
                tauri::Url::parse(value).is_ok_and(|url| {
                    credential_server(&url).as_deref() == Some(entry.server.as_str())
                        && (url.username().is_empty() || url_username(&url) == entry.username)
                })
            })
        })
        .cloned()
}

// Called only after the GUI retry has authenticated successfully.
pub fn save_credentials(server: &str, username: &str) -> Result<bool, SvnError> {
    let url = tauri::Url::parse(server)
        .map_err(|_| SvnError::InvalidArguments("Invalid server URL".into()))?;
    let server = credential_server(&url)
        .ok_or_else(|| SvnError::InvalidArguments("Invalid server URL".into()))?;
    let saved = CREDENTIALS
        .get_or_init(Default::default)
        .read()
        .map_err(|_| SvnError::CommandFailed("Credential lock unavailable".into()))?
        .iter()
        .find(|entry| entry.server == server && entry.username == username)
        .cloned();
    match saved {
        Some(credentials) => super::credential_store::save(&credentials),
        None => Err(SvnError::InvalidArguments(
            "No authenticated session credentials".into(),
        )),
    }
}

fn saved_credentials_for(
    urls: &[String],
    load: impl FnOnce(&str, &str) -> Option<Credentials>,
) -> Option<Credentials> {
    let parsed = urls
        .iter()
        .map(|url| tauri::Url::parse(url).ok())
        .collect::<Option<Vec<_>>>()?;
    let server = credential_server(parsed.first()?)?;
    if parsed
        .iter()
        .any(|url| credential_server(url).as_deref() != Some(server.as_str()))
    {
        return None;
    }
    let usernames: Vec<_> = parsed
        .iter()
        .map(url_username)
        .filter(|name| !name.is_empty())
        .collect();
    let username = usernames.first().map(String::as_str).unwrap_or("");
    if usernames.iter().any(|name| name != username) {
        return None;
    }
    let saved = load(&server, username)?;
    credentials_for(&[saved], urls)
}

pub const SVN_TIMEOUT: Duration = Duration::from_secs(120);
static SVN_EXECUTABLE: OnceLock<RwLock<PathBuf>> = OnceLock::new();

#[derive(Error, Debug)]
pub enum SvnError {
    #[error("SVN 命令执行失败：{0}")]
    CommandFailed(String),
    #[error("SVN 未安装或不在 PATH 中")]
    SvnNotFound,
    #[error("解析输出失败：{0}")]
    ParseError(String),
    #[error("无效的 SVN 参数：{0}")]
    InvalidArguments(String),
    #[error("SVN 操作超时")]
    Timeout,
}

pub async fn execute_svn(args: &[&str], path: Option<&str>) -> Result<String, SvnError> {
    let executable = current_svn_executable()?;
    let credentials = CREDENTIALS
        .get_or_init(Default::default)
        .read()
        .map_err(|_| SvnError::CommandFailed("Credential lock unavailable".into()))?
        .clone();
    execute_svn_resolved(executable, args, path, credentials, true).await
}

// Look only at command targets, never messages or other option values.
fn explicit_urls<'a>(args: &'a [&'a str]) -> Vec<&'a str> {
    let targets = if let Some(separator) = args.iter().position(|arg| *arg == "--") {
        &args[separator + 1..]
    } else if matches!(args.first(), Some(&"checkout" | &"switch" | &"merge")) {
        &args[1..]
    } else {
        &[]
    };
    targets
        .iter()
        .copied()
        .filter(|arg| arg.contains("://"))
        .collect()
}

#[cfg(test)]
async fn execute_svn_with_session_credentials(
    executable: PathBuf,
    args: &[&str],
    path: Option<&str>,
    credentials: Vec<Credentials>,
) -> Result<String, SvnError> {
    execute_svn_resolved(executable, args, path, credentials, false).await
}

async fn execute_svn_resolved(
    executable: PathBuf,
    args: &[&str],
    path: Option<&str>,
    credentials: Vec<Credentials>,
    load_saved: bool,
) -> Result<String, SvnError> {
    if credentials.is_empty() && !load_saved {
        return execute_svn_inner(executable, args, path).await;
    }
    let mut urls: Vec<String> = explicit_urls(args).into_iter().map(str::to_owned).collect();
    let options = &args[..args
        .iter()
        .position(|arg| *arg == "--")
        .unwrap_or(args.len())];
    let local_info = args.first() == Some(&"info")
        && urls.is_empty()
        && !options
            .iter()
            .any(|arg| arg.starts_with("-r") || arg.starts_with("--revision"));
    let local_status = args.first() == Some(&"status")
        && !options
            .iter()
            .any(|arg| matches!(*arg, "-u" | "--show-updates"));
    let local_diff = matches!(args, ["diff", "--", target] if !target.contains("://"));
    let local_only = local_info
        || local_status
        || local_diff
        || matches!(
            args.first(),
            Some(&"add" | &"revert" | &"resolve" | &"cleanup" | &"auth")
        );
    if local_only {
        return execute_svn_inner(executable, args, path).await;
    }
    if urls.is_empty() {
        if let Some(path) = path {
            // Local WC metadata identifies the server without sending credentials.
            let url = execute_svn_inner(
                executable.clone(),
                &["info", "--show-item", "url", "--", "."],
                Some(path),
            )
            .await?;
            urls.push(url.trim().to_string());
        }
    }
    let mut credentials = credentials_for(&credentials, &urls);
    if credentials.is_none() && load_saved {
        // Never block the UI/runtime worker while macOS handles Keychain access.
        credentials = tokio::task::spawn_blocking(move || {
            saved_credentials_for(&urls, super::credential_store::load)
        })
        .await
        .map_err(|_| SvnError::CommandFailed("Credential lookup unavailable".into()))?;
        if let Some(saved) = &credentials {
            configure_credentials(
                &saved.server,
                saved.username.clone(),
                saved.password.clone(),
            )?;
        }
    }
    execute_svn_with_credentials(executable, args, path, credentials).await
}

pub async fn configure_svn_executable(executable: Option<&str>) -> Result<String, SvnError> {
    let executable = normalize_svn_executable(executable);

    let version = execute_svn_inner(executable.clone(), &["--version", "--quiet"], None).await?;

    let mut configured = svn_executable_lock()
        .write()
        .map_err(|_| SvnError::CommandFailed("SVN 可执行文件配置锁已损坏".to_string()))?;
    *configured = executable;
    Ok(version.trim().to_string())
}

fn normalize_svn_executable(executable: Option<&str>) -> PathBuf {
    executable
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_svn_executable)
}

fn default_svn_executable() -> PathBuf {
    #[cfg(target_os = "macos")]
    if let Some(bundled) = std::env::current_exe()
        .ok()
        .and_then(|executable| {
            Some(
                executable
                    .parent()?
                    .parent()?
                    .join("Resources/svn-runtime/svn"),
            )
        })
        .filter(|path| path.is_file())
    {
        return bundled;
    }
    PathBuf::from("svn")
}

fn svn_executable_lock() -> &'static RwLock<PathBuf> {
    SVN_EXECUTABLE.get_or_init(|| RwLock::new(default_svn_executable()))
}

pub(super) fn current_svn_executable() -> Result<PathBuf, SvnError> {
    let executable = svn_executable_lock()
        .read()
        .map(|path| path.clone())
        .map_err(|_| SvnError::CommandFailed("SVN 可执行文件配置锁已损坏".to_string()))?;
    #[cfg(windows)]
    if executable == std::path::Path::new("svn") {
        return super::windows_runtime::executable();
    }
    Ok(executable)
}

fn build_command_args(args: &[String]) -> Vec<String> {
    let mut command_args = vec!["--non-interactive".to_string()];
    command_args.extend(args.iter().cloned());
    command_args
}

async fn execute_svn_inner(
    executable: PathBuf,
    args: &[&str],
    path: Option<&str>,
) -> Result<String, SvnError> {
    execute_svn_with_credentials(executable, args, path, None).await
}

async fn execute_svn_with_credentials(
    executable: PathBuf,
    args: &[&str],
    path: Option<&str>,
    credentials: Option<Credentials>,
) -> Result<String, SvnError> {
    #[cfg(windows)]
    let executable = {
        let bundled = super::windows_runtime::executable()?;
        if executable == std::path::Path::new("svn") || executable == bundled {
            bundled
        } else {
            match super::windows_encoding::validate_arguments(&executable, args).await {
                Ok(()) => executable,
                Err(SvnError::InvalidArguments(_)) => bundled,
                Err(error) => return Err(error),
            }
        }
    };
    let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut cmd = Command::new(&executable);
    if let Some(credentials) = &credentials {
        cmd.args([
            "--username",
            &credentials.username,
            "--password-from-stdin",
            "--no-auth-cache",
        ]);
        cmd.stdin(Stdio::piped());
    }
    cmd.args(build_command_args(&args_vec));
    // SVN converts paths and messages using the process locale. Desktop launches
    // may inherit no locale, an ASCII locale, or a non-UTF-8 locale even though
    // Rust passes UTF-8 arguments. Override LC_ALL (which takes precedence over
    // LANG and LC_CTYPE) for the child only, keeping its output UTF-8 as well.
    #[cfg(unix)]
    {
        // macOS ships en_US.UTF-8; C.UTF-8 is available on Linux without
        // installing or generating an additional language locale.
        #[cfg(target_os = "macos")]
        const SVN_LOCALE: &str = "en_US.UTF-8";
        #[cfg(not(target_os = "macos"))]
        const SVN_LOCALE: &str = "C.UTF-8";
        cmd.env("LC_ALL", SVN_LOCALE);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Dropping the output future on timeout kills the child rather than leaving
    // an SVN process running. Tokio drains stdout and stderr concurrently.
    cmd.kill_on_drop(true);
    if let Some(path) = path {
        cmd.current_dir(path);
    }

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = collect_command_output_with_password(
        &mut cmd,
        SVN_TIMEOUT,
        credentials.as_ref().map(|entry| entry.password.as_str()),
    )
    .await?;
    if output.trim().is_empty() && always_prints_output(&args_vec) {
        return Err(SvnError::CommandFailed(format!(
            "SVN 命令成功退出但没有任何输出：svn {}\n{} 的输出未能送达 RevBoat，可能是透明加密或数据防泄漏软件的进程策略所致。请联系管理员，将 RevBoat 及其启动的 SVN 程序加入策略。",
            args_vec.join(" "),
            executable.display()
        )));
    }
    Ok(output)
}

/// Commands whose successful output is never empty. With some transparent
/// encryption agents, `svn.exe` started by an unlisted parent exits with 0 without
/// delivering any stdout. The XML parsers accept empty input, so this would
/// otherwise look like a clean working copy. Report it rather than work around it:
/// the agent's policy is the owner's to change.
fn always_prints_output(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--xml" || arg == "--version")
}

#[cfg(any(windows, test))]
pub(super) async fn collect_command_output(
    cmd: &mut Command,
    deadline: Duration,
) -> Result<String, SvnError> {
    collect_command_output_with_password(cmd, deadline, None).await
}

async fn collect_command_output_with_password(
    cmd: &mut Command,
    deadline: Duration,
    password: Option<&str>,
) -> Result<String, SvnError> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if password.is_none() {
        cmd.stdin(Stdio::null());
    }
    let output = timeout(deadline, async {
        use tokio::io::AsyncWriteExt;
        let mut child = cmd.spawn()?;
        if let Some(password) = password {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(password.as_bytes()).await?;
                stdin.write_all(b"\n").await?;
            }
        }
        child.wait_with_output().await
    })
    .await
    .map_err(|_| SvnError::Timeout)?
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SvnError::SvnNotFound
        } else {
            SvnError::CommandFailed(error.to_string())
        }
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stdout.trim_start().starts_with("<?xml") && !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim(), stderr.trim())
                .trim()
                .to_string()
        };
        Err(SvnError::CommandFailed(message))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn real_svnserve_authentication_supports_checkout_update_and_log() {
        for executable in ["svn", "svnadmin", "svnserve"] {
            if std::process::Command::new(executable)
                .arg("--version")
                .output()
                .is_err()
            {
                assert!(
                    std::env::var_os("ORCASVN_REQUIRE_AUTH_INTEGRATION").is_none(),
                    "Required tool missing: {executable}"
                );
                eprintln!("Skipping real authentication fixture: {executable} unavailable");
                return;
            }
        }
        let root = std::env::temp_dir().join(format!(
            "orcasvn-real-auth-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let repo = root.join("repo");
        assert!(std::process::Command::new("svnadmin")
            .arg("create")
            .arg(&repo)
            .status()
            .unwrap()
            .success());
        std::fs::write(repo.join("conf/svnserve.conf"), "[general]\nanon-access = none\nauth-access = write\npassword-db = passwd\nrealm = OrcaSVN test fixture\n").unwrap();
        std::fs::write(
            repo.join("conf/passwd"),
            "[users]\ndemo = fictional secret\nother = second secret\n",
        )
        .unwrap();
        let config = root.join("client-config");
        std::fs::create_dir(&config).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let server = std::process::Command::new("svnserve")
            .args([
                "--daemon",
                "--foreground",
                "--listen-host",
                "127.0.0.1",
                "--listen-port",
                &port.to_string(),
                "--root",
            ])
            .arg(&root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        struct Fixture {
            root: PathBuf,
            server: std::process::Child,
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = self.server.kill();
                let _ = self.server.wait();
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
        let _fixture = Fixture {
            root: root.clone(),
            server,
        };
        let mut ready = false;
        for _ in 0..100 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(ready, "svnserve did not start");
        let url = format!("svn://127.0.0.1:{port}/repo");
        let credentials = vec![Credentials {
            server: format!("svn://127.0.0.1:{port}"),
            username: "demo".into(),
            password: "fictional secret".into(),
        }];
        let config = config.to_str().unwrap();
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let info_args = ["info", "-r", "HEAD", "--xml", "--config-dir", config, "--", &url];
            let error = execute_svn_with_session_credentials(PathBuf::from("svn"), &info_args, None, Vec::new()).await.unwrap_err().to_string();
            assert!(error.contains("E170001") || error.contains("E215004"), "{error}");
            let info = execute_svn_with_session_credentials(PathBuf::from("svn"), &info_args, None, credentials.clone()).await.unwrap();
            assert!(info.contains("<info>"));
            let wc = root.join("working-copy");
            let wc = wc.to_str().unwrap();
            execute_svn_with_session_credentials(PathBuf::from("svn"), &["checkout", "--config-dir", config, "--", &url, wc], None, credentials.clone()).await.unwrap();
            std::fs::write(Path::new(wc).join("demo.txt"), "fixture\n").unwrap();
            execute_svn_inner(PathBuf::from("svn"), &["add", "--", "demo.txt"], Some(wc)).await.unwrap();
            execute_svn_with_session_credentials(PathBuf::from("svn"), &["commit", "-m", "fixture", "--config-dir", config, "--", "demo.txt"], Some(wc), credentials.clone()).await.unwrap();
            execute_svn_with_session_credentials(PathBuf::from("svn"), &["update", "--config-dir", config, "--", "."], Some(wc), credentials.clone()).await.unwrap();
            let log = execute_svn_with_session_credentials(PathBuf::from("svn"), &["log", "--xml", "--config-dir", config], Some(wc), credentials.clone()).await.unwrap();
            assert!(log.contains("fixture"));
            let mut wrong = credentials.clone();
            wrong[0].password = "wrong password".into();
            assert!(execute_svn_with_session_credentials(PathBuf::from("svn"), &info_args, None, wrong).await.is_err());
            // Session credentials must not seed SVN's on-disk authentication cache.
            assert!(execute_svn_with_session_credentials(PathBuf::from("svn"), &info_args, None, Vec::new()).await.is_err());
            configure_credentials(&credentials[0].server, "demo".into(), "fictional secret".into()).unwrap();
            assert!(validate_account_with(PathBuf::from("svn"), &url, "other".into(), "bad password".into()).await.is_err());
            let snapshot = CREDENTIALS.get().unwrap().read().unwrap().clone();
            assert_eq!(credentials_for(&snapshot, std::slice::from_ref(&url)).unwrap().username, "demo");
            let candidate = validate_account_with(PathBuf::from("svn"), &url, "other".into(), "second secret".into()).await.unwrap();
            configure_credentials(&candidate.server, candidate.username.clone(), candidate.password.clone()).unwrap();
            let changed = CREDENTIALS.get().unwrap().read().unwrap().clone();
            assert_eq!(credentials_for(&changed, std::slice::from_ref(&url)).unwrap().username, "other");
            let output = execute_svn_with_session_credentials(PathBuf::from("svn"), &info_args, None, changed).await.unwrap();
            assert!(output.contains("<info>"));
            let pinned = url.replacen("svn://", "svn://demo@", 1);
            assert!(validate_account_with(PathBuf::from("svn"), &pinned, "other".into(), "second secret".into()).await.is_err());
            eprintln!("Real svnserve authentication and account replacement verified; wrong password preserves the previous account");
        });
    }

    #[test]
    fn restored_credentials_are_scoped_before_accessing_keychain() {
        let urls = vec!["svn://svn.example:3690/repo".into()];
        let restored = saved_credentials_for(&urls, |server, username| {
            assert_eq!(server, "svn://svn.example");
            assert_eq!(username, "");
            Some(Credentials {
                server: server.into(),
                username: "demo".into(),
                password: "saved password".into(),
            })
        })
        .unwrap();
        assert_eq!(restored.password, "saved password");
        for urls in [
            vec![],
            vec!["file:///repo".into()],
            vec!["svn://svn.example/repo".into(), "svn://other/repo".into()],
            vec![
                "svn://alice@svn.example/repo".into(),
                "svn://bob@svn.example/repo".into(),
            ],
        ] {
            assert!(saved_credentials_for(&urls, |_, _| panic!(
                "Must not access Keychain for ambiguous or local targets"
            ))
            .is_none());
        }
        assert!(saved_credentials_for(
            &["svn://alice@svn.example/repo".into()],
            |server, username| {
                assert_eq!(username, "alice");
                Some(Credentials {
                    server: server.into(),
                    username: "bob".into(),
                    password: "other account".into(),
                })
            }
        )
        .is_none());
    }

    #[test]
    fn session_credentials_do_not_cross_servers_ports_or_explicit_accounts() {
        let entries = vec![super::Credentials {
            server: "https://auth-test.invalid:8443".into(),
            username: "qi".into(),
            password: "secret".into(),
        }];
        assert!(super::credentials_for(
            &entries,
            &["https://auth-test.invalid:8443/svn/repo".into()]
        )
        .is_some());
        for url in [
            "https://other.invalid:8443/svn/repo",
            "https://auth-test.invalid/svn/repo",
            "http://auth-test.invalid:8443/svn/repo",
            "https://other@auth-test.invalid:8443/svn/repo",
        ] {
            assert!(super::credentials_for(&entries, &[url.into()]).is_none());
        }
        assert!(super::credentials_for(&entries, &[]).is_none());
        assert!(super::configure_credentials(
            "svn+ssh://auth-test.invalid",
            "qi".into(),
            "secret".into()
        )
        .is_err());
    }

    #[test]
    fn native_svn_credentials_normalize_default_ports_and_isolate_servers() {
        for (url, expected) in [
            ("svn://SVN.example/repo", "svn://svn.example"),
            ("svn://demo@svn.example:3690/repo", "svn://svn.example"),
            ("svn://svn.example:3691/repo", "svn://svn.example:3691"),
            ("svn://[::1]:3690/repo", "svn://[::1]"),
            ("svn://[::1]:3691/repo", "svn://[::1]:3691"),
        ] {
            assert_eq!(
                credential_server(&tauri::Url::parse(url).unwrap()).as_deref(),
                Some(expected)
            );
        }
        assert!(configure_credentials(
            "svn://native-auth-test.invalid:3690",
            "demo".into(),
            "secret".into()
        )
        .is_ok());
        let entries = vec![Credentials {
            server: "svn://svn.example".into(),
            username: "demo".into(),
            password: "secret".into(),
        }];
        for url in [
            "svn://svn.example/repo",
            "svn://demo@SVN.example:3690/other",
        ] {
            assert!(credentials_for(&entries, &[url.into()]).is_some());
        }
        for url in [
            "svn://other.example/repo",
            "svn://svn.example:3691/repo",
            "https://svn.example/repo",
            "svn+ssh://svn.example/repo",
            "svn://other@svn.example/repo",
        ] {
            assert!(credentials_for(&entries, &[url.into()]).is_none());
        }
        assert!(credentials_for(
            &entries,
            &[
                "svn://svn.example/repo".into(),
                "svn://other.example/repo".into()
            ]
        )
        .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn native_svn_credentials_reach_checkout_and_existing_working_copies() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "stonesvn-native-auth-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let executable = root.join("svn");
        std::fs::write(
            &executable,
            r#"#!/bin/sh
for arg in "$@"; do
  if [ "$arg" = --show-item ]; then echo svn://native.example:3690/repo; exit 0; fi
done
printf '%s\n' "$@"
for arg in "$@"; do
  if [ "$arg" = --password-from-stdin ]; then
    IFS= read -r password
    [ "$password" = 'native secret' ] || exit 1
    echo authenticated
    exit 0
  fi
done
exit 1
"#,
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let credentials = vec![Credentials {
            server: "svn://native.example".into(),
            username: "demo".into(),
            password: "native secret".into(),
        }];
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                for args in [
                    vec![
                        "info",
                        "-r",
                        "HEAD",
                        "--xml",
                        "--",
                        "svn://native.example:3690/repo",
                    ],
                    vec!["checkout", "--", "svn://native.example/repo", "wc"],
                    vec!["update", "--", "."],
                    vec!["log", "--xml"],
                ] {
                    let output = execute_svn_with_session_credentials(
                        executable.clone(),
                        &args,
                        root.to_str(),
                        credentials.clone(),
                    )
                    .await
                    .unwrap();
                    assert!(output.contains("authenticated"));
                    assert!(output.contains("--no-auth-cache"));
                    assert!(!output.contains("native secret"));
                    assert!(!output.contains("--trust-server-cert-failures"));
                }
            });
        std::fs::remove_dir_all(root).unwrap();
    }

    use super::*;
    use std::path::Path;

    #[cfg(windows)]
    #[test]
    fn windows_unicode_arguments_work_with_bundled_and_legacy_clients() {
        for executable in ["svn", "svn.exe"] {
            let directory = std::env::temp_dir().join(format!(
                "orcasvn-windows-encoding-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&directory).unwrap();
            struct Cleanup(PathBuf);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
            let _cleanup = Cleanup(directory.clone());
            let repository = directory.join("repo");
            let admin = super::super::windows_runtime::executable()
                .unwrap()
                .with_file_name("svnadmin.exe");
            let created = std::process::Command::new(admin)
                .arg("create")
                .arg(&repository)
                .output()
                .unwrap();
            assert!(created.status.success(), "{created:?}");
            let url = format!(
                "file:///{}",
                repository.to_string_lossy().replace('\\', "/")
            );
            let wc = directory.join("工作副本 🐋");
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let run = |args: &[&str], cwd| {
                runtime
                    .block_on(execute_svn_inner(PathBuf::from(executable), args, cwd))
                    .unwrap()
            };
            run(&["checkout", &url, wc.to_str().unwrap()], None);
            let wc_path = wc.to_str().unwrap();
            for name in [
                "A.txt",
                "Ａ.txt",
                "中文 空格.txt",
                "日本語.txt",
                "emoji-🐋.txt",
            ] {
                std::fs::write(wc.join(name), format!("original {name}\n")).unwrap();
                run(&["add", "--", name], Some(wc_path));
                run(&["commit", "-m", "中文提交 🐋"], Some(wc_path));
                let info = run(&["info", "--xml", "--", name], Some(wc_path));
                assert!(info.contains(name), "wrong target: {info}");
                std::fs::write(wc.join(name), "changed\n").unwrap();
                let diff = run(&["diff", "--", name], Some(wc_path));
                assert!(diff.contains("+changed"), "{diff}");
                run(&["revert", "--", name], Some(wc_path));
                assert_eq!(
                    std::fs::read_to_string(wc.join(name)).unwrap(),
                    format!("original {name}\n")
                );
            }
            let log = run(&["log", "--xml", "-v", &url], None);
            assert!(log.contains("中文提交 🐋"));
            assert!(log.contains("emoji-🐋.txt"));
        }
    }

    #[test]
    fn empty_output_is_only_an_error_for_commands_that_always_print() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert!(always_prints_output(&args(&["status", "--xml"])));
        assert!(always_prints_output(&args(&["info", "--xml"])));
        assert!(always_prints_output(&args(&["--version", "--quiet"])));
        assert!(!always_prints_output(&args(&["diff"])));
        assert!(!always_prints_output(&args(&[
            "info",
            "--show-item",
            "revision"
        ])));
    }

    // A stand-in for the affected Windows behaviour: exit 0 with no stdout.
    #[cfg(unix)]
    #[test]
    fn silent_svn_is_reported_instead_of_parsed_as_a_clean_working_copy() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::env::temp_dir().join(format!("orcasvn-silent-svn-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let fake = directory.join("svn");
        std::fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let status = runtime.block_on(execute_svn_inner(fake.clone(), &["status", "--xml"], None));
        assert!(status.unwrap_err().to_string().contains("没有任何输出"));
        let diff = runtime.block_on(execute_svn_inner(fake, &["diff"], None));
        assert_eq!(diff.unwrap(), "");
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn non_ascii_paths_work_without_a_utf8_parent_locale() {
        const CHILD_MARKER: &str = "ORCASVN_ENCODING_TEST_CHILD";
        if std::env::var_os(CHILD_MARKER).is_none() {
            for executable in ["svn", "svnadmin"] {
                if std::process::Command::new(executable)
                    .arg("--version")
                    .output()
                    .is_err()
                {
                    eprintln!("Skipping encoding regression test: {executable} is unavailable");
                    return;
                }
            }
            // Isolate parent locale changes from other tests running in parallel.
            for locale in [None, Some("C"), Some("invalid_ORCASVN_locale")] {
                let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                child.args([
                    "--exact",
                    "svn::executor::tests::non_ascii_paths_work_without_a_utf8_parent_locale",
                    "--nocapture",
                ]);
                child.env(CHILD_MARKER, "1");
                for variable in ["LC_ALL", "LC_CTYPE", "LANG"] {
                    match locale {
                        Some(value) => {
                            child.env(variable, value);
                        }
                        None => {
                            child.env_remove(variable);
                        }
                    }
                }
                let output = child.output().unwrap();
                assert!(
                    output.status.success(),
                    "encoding regression failed with parent locale {locale:?}:\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }

        let directory = std::env::temp_dir().join(format!(
            "orcasvn-encoding-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let repository = directory.join("repository");
        let created = std::process::Command::new("svnadmin")
            .arg("create")
            .arg(&repository)
            .output()
            .unwrap();
        assert!(created.status.success());
        let url = format!("file://{}", repository.to_str().unwrap());
        let wc = directory.join("工作副本");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let run = |args: &[&str], path: Option<&str>| {
            runtime
                .block_on(execute_svn_inner(PathBuf::from("svn"), args, path))
                .unwrap()
        };
        run(&["checkout", &url, wc.to_str().unwrap()], None);
        std::fs::create_dir(wc.join("子目录")).unwrap();
        std::fs::write(wc.join("子目录/文件.xlsx"), "中文内容").unwrap();
        run(&["add", "子目录"], Some(wc.to_str().unwrap()));
        run(
            &["commit", "-m", "提交中文文件", "子目录/文件.xlsx", "子目录"],
            Some(wc.to_str().unwrap()),
        );
        let log = run(&["log", "--xml", "-v", &url], None);
        assert!(log.contains("提交中文文件"));
        assert!(log.contains("/子目录/文件.xlsx"));
        let second_wc = directory.join("第二副本");
        run(
            &[
                "checkout",
                &format!("{url}/子目录"),
                second_wc.to_str().unwrap(),
            ],
            None,
        );
        assert_eq!(
            std::fs::read_to_string(second_wc.join("文件.xlsx")).unwrap(),
            "中文内容"
        );
    }

    #[test]
    fn captures_svn_stdout() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");

        let result = runtime.block_on(execute_svn(&["--version", "--quiet"], None));

        match result {
            Ok(output) => assert!(!output.trim().is_empty()),
            Err(SvnError::SvnNotFound) => {}
            Err(err) => panic!("unexpected svn executor error: {err}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn drains_large_stdout_and_stderr_without_blocking() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "i=0; while [ $i -lt 10000 ]; do echo output; echo error >&2; i=$((i+1)); done",
        ]);
        command.kill_on_drop(true);
        let output = runtime
            .block_on(collect_command_output(
                &mut command,
                Duration::from_secs(10),
            ))
            .unwrap();
        assert_eq!(output.lines().count(), 10000);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_the_child() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "echo $$; exec sleep 10"]);
        command.kill_on_drop(true);
        // Spawn explicitly so the test can check that the timed-out PID exits.
        command.stdout(Stdio::piped());
        runtime.block_on(async {
            let child = command.spawn().unwrap();
            let pid = child.id().unwrap();
            assert!(timeout(Duration::from_millis(30), child.wait_with_output())
                .await
                .is_err());
            for _ in 0..100 {
                let alive = std::process::Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .output()
                    .unwrap()
                    .status
                    .success();
                if !alive {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("timed-out child is still alive");
        });
    }

    #[test]
    fn empty_executable_uses_default_command() {
        assert_eq!(normalize_svn_executable(Some("   ")), Path::new("svn"));
        assert_eq!(normalize_svn_executable(None), Path::new("svn"));
    }

    #[test]
    fn configured_executable_is_trimmed() {
        assert_eq!(
            normalize_svn_executable(Some("  C:\\Tools\\svn.exe  ")),
            Path::new("C:\\Tools\\svn.exe")
        );
    }

    #[test]
    fn global_options_precede_target_separator() {
        let args = vec![
            "diff".to_string(),
            "-c".to_string(),
            "12".to_string(),
            "--".to_string(),
            "https://example.test/repo/file@12".to_string(),
        ];

        assert_eq!(
            build_command_args(&args),
            [
                "--non-interactive",
                "diff",
                "-c",
                "12",
                "--",
                "https://example.test/repo/file@12"
            ]
        );
    }
}
