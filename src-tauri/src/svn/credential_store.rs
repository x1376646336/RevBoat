//! Passwords are stored only in the operating system credential store.
//! Neither the app settings nor SVN's plaintext auth cache receive secrets.
use super::executor::{Credentials, SvnError};

fn service(server: &str) -> String {
    format!("OrcaSVN SVN credentials: {server}")
}

fn account(username: &str) -> String {
    if username.is_empty() {
        "default".into()
    } else {
        format!("user:{username}")
    }
}

fn decode(bytes: &[u8], server: &str, username: &str) -> Option<Credentials> {
    let saved: Credentials = serde_json::from_slice(bytes).ok()?;
    if saved.server != server
        || (!username.is_empty() && saved.username != username)
        || saved.username.trim().is_empty()
        || saved.password.is_empty()
        || saved.username.contains(['\r', '\n'])
        || saved.password.contains(['\r', '\n'])
    {
        return None;
    }
    Some(saved)
}

fn save_with(
    credentials: &Credentials,
    mut write: impl FnMut(&str, &str, &[u8]) -> Result<(), SvnError>,
) -> Result<(), SvnError> {
    let data = serde_json::to_vec(credentials).map_err(|_| {
        SvnError::CommandFailed("Cannot encode credentials for secure storage".into())
    })?;
    let service = service(&credentials.server);
    write(&service, &account(&credentials.username), &data)?;
    // URLs without an explicit username use the last successfully authenticated account.
    write(&service, &account(""), &data)
}

pub(super) fn save(credentials: &Credentials) -> Result<bool, SvnError> {
    #[cfg(target_os = "macos")]
    {
        save_with(credentials, |service, account, bytes| {
            security_framework::passwords::set_generic_password(service, account, bytes).map_err(
                |_| SvnError::CommandFailed("Cannot save SVN credentials to macOS Keychain".into()),
            )
        })?;
        Ok(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = credentials;
        Ok(false)
    }
}

pub(super) fn load(server: &str, username: &str) -> Option<Credentials> {
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::{generic_password, PasswordOptions};
        let bytes = generic_password(PasswordOptions::new_generic_password(
            &service(server),
            &account(username),
        ))
        .ok()?;
        decode(&bytes, server, username)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (server, username);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn persisted_credentials_survive_a_new_session_and_accounts_stay_separate() {
        let mut vault = HashMap::new();
        for (username, password) in [("alice", "old"), ("bob", "other"), ("alice", "changed")] {
            save_with(
                &Credentials {
                    server: "svn://example:3691".into(),
                    username: username.into(),
                    password: password.into(),
                },
                |s, a, bytes| {
                    vault.insert((s.to_owned(), a.to_owned()), bytes.to_vec());
                    Ok(())
                },
            )
            .unwrap();
        }
        // No session cache is involved in retrieval; this models a fresh process.
        for (username, expected) in [("", "changed"), ("alice", "changed"), ("bob", "other")] {
            let data = &vault[&(service("svn://example:3691"), account(username))];
            let saved = decode(data, "svn://example:3691", username).unwrap();
            assert_eq!(saved.password, expected);
            assert!(decode(data, "svn://other:3691", username).is_none());
            assert!(decode(data, "svn://example", username).is_none());
            assert!(decode(data, "https://example:3691", username).is_none());
            assert!(decode(data, "svn://example:3691", "unrelated").is_none());
        }
        assert!(decode(b"corrupted", "svn://example", "").is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Creates and removes an isolated synthetic Keychain item; run explicitly"]
    fn macos_keychain_survives_process_restart() {
        use security_framework::os::macos::keychain::SecKeychain;
        use security_framework::passwords::delete_generic_password;
        let _no_prompts = SecKeychain::disable_user_interaction().unwrap();
        const CHILD: &str = "ORCASVN_KEYCHAIN_TEST_SERVER";
        if let Ok(server) = std::env::var(CHILD) {
            for username in ["", "demo"] {
                let saved =
                    load(&server, username).expect("fresh process must retrieve saved credentials");
                assert_eq!(saved.username, "demo");
                assert_eq!(saved.password, "synthetic fixture only");
            }
            return;
        }
        let server = format!(
            "svn://orcasvn-keychain-test-{}-{}.invalid",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for username in ["", "demo"] {
                    let _ = delete_generic_password(&service(&self.0), &account(username));
                }
            }
        }
        let _cleanup = Cleanup(server.clone());
        assert!(save(&Credentials {
            server: server.clone(),
            username: "demo".into(),
            password: "synthetic fixture only".into()
        })
        .unwrap());
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "svn::credential_store::tests::macos_keychain_survives_process_restart",
            ])
            .env(CHILD, &server)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "fresh-process Keychain lookup failed: {}",
            String::from_utf8_lossy(&child.stderr)
        );
    }

    #[test]
    fn vault_write_failure_is_reported_without_secret_details() {
        let credentials = Credentials {
            server: "svn://example".into(),
            username: "alice".into(),
            password: "test secret".into(),
        };
        let mut calls = 0;
        let result = save_with(&credentials, |_, _, _| {
            calls += 1;
            Err(SvnError::CommandFailed("Keychain unavailable".into()))
        });
        assert_eq!(calls, 1);
        assert!(!result
            .unwrap_err()
            .to_string()
            .contains(&credentials.password));
    }
}
