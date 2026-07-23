use chrono::{SecondsFormat, TimeZone, Utc};
use std::{cmp::Ordering, collections::HashSet, env, process::Command};

fn command_output(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("could not execute command `{program} {}`: {e}", args.join(" ")))?;

    let mut value = String::from_utf8(output.stdout)
        .map_err(|e| format!("could not parse output as utf-8: {e}"))?;
    // https://stackoverflow.com/a/55041833
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    };

    Ok(value)
}

fn set_env_if_needed(env: &str, value: &Result<String, String>) {
    if env::var(env).is_ok() {
        return;
    }

    match value {
        Ok(v) => println!("cargo:rustc-env={env}={v}"),
        Err(e) => {
            // debug
            if cfg!(debug_assertions) {
                println!("cargo::warning=could not set env {env}: {e}")
            }
            // release
            else {
                println!("carg::error=env {env} not set for release: {e}");
            }
        }
    }
}

fn branch_name_from_ref(branch_ref: &str) -> Option<&str> {
    // local branch
    if let Some(branch_name) = branch_ref.strip_prefix("refs/heads/") {
        Some(branch_name)
    }
    // remote branche
    else if let Some(remote_ref) = branch_ref.strip_prefix("refs/remotes/") {
        let branch_name = &remote_ref[(remote_ref.find('/')? + 1)..];
        if branch_name != "HEAD" {
            Some(branch_name)
        } else {
            None
        }
    } else {
        None
    }
}

fn epoch_to_iso_timestamp(epoch: i64) -> String {
    Utc.timestamp_opt(epoch, 0).unwrap().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // re-run if files change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // re-run if env change
    println!("cargo:rerun-if-env-changed=BUILD_COMMIT_HASH");
    println!("cargo:rerun-if-env-changed=BUILD_COMMIT_TIMESTAMP_ISO");
    println!("cargo:rerun-if-env-changed=BUILD_COMMIT_LABELS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let commit_hash_result = command_output("git", &["log", "-1", "--format=%H"]);
    set_env_if_needed("BUILD_COMMIT_HASH", &commit_hash_result);

    set_env_if_needed(
        "BUILD_COMMIT_TIMESTAMP_ISO",
        &command_output("git", &["log", "-1", "--format=%ct"])
            .map(|epoch| epoch_to_iso_timestamp(epoch.parse().unwrap())),
    );

    // TODO: get commit labels
    let mut commit_label_set = HashSet::new();
    if let Ok(commit_hash) = &commit_hash_result {
        let branch_refs = command_output(
            "git",
            &["branch", "--points-at", commit_hash, "--format=%(refname)", "--all"],
        )?;
        for branch_ref in branch_refs.lines() {
            let Some(branch_name) = branch_name_from_ref(branch_ref) else {
                continue;
            };
            if !branch_name.is_empty() {
                commit_label_set.insert(("branch", branch_name.to_string()));
            }
        }

        let tags = command_output(
            "git",
            &["tag", "--points-at", commit_hash, "--format=%(refname:short)"],
        )?;
        for tag in tags.lines() {
            if !tag.is_empty() {
                commit_label_set.insert(("tag", tag.to_string()));
            }
        }
    }

    let mut commit_labels = commit_label_set.into_iter().collect::<Vec<_>>();
    commit_labels.sort_unstable_by(|a, b| {
        if a.0 != b.0 {
            a.1.cmp(&b.1)
        }
        // tags before branches
        else if a.0 == "tag" {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });

    set_env_if_needed(
        "BUILD_COMMIT_LABELS",
        &Ok(commit_labels
            .into_iter()
            .map(|(label_type, value)| match label_type {
                "tag" => value.to_string(),
                _ => format!("{label_type}:{value}"),
            })
            .collect::<Vec<_>>()
            .join(" ")),
    );

    // https://reproducible-builds.org/docs/source-date-epoch/#rust
    let build_timestamp_iso = epoch_to_iso_timestamp(
        option_env!("SOURCE_DATE_EPOCH")
            .map(|epoch| epoch.parse().unwrap())
            .unwrap_or_else(|| Utc::now().timestamp()),
    );
    println!("cargo:rustc-env=BUILD_TIMESTAMP_ISO={build_timestamp_iso}");

    Ok(())
}
