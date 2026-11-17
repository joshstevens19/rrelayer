use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerInfo<'a> {
    /// The iso timestamp of when the server was started
    #[serde(rename = "startedAtTimestampIso")]
    pub started_at_timestamp_iso: String,

    /// The number of seconds the server has been running for
    #[serde(rename = "uptimeSeconds")]
    pub uptime_seconds: i64,

    /// The hash of the commit used to build the server
    #[serde(rename = "commitHash")]
    pub commit_hash: &'a str,

    /// The iso timestamp of the commit used to build the server
    #[serde(rename = "commitTimestampIso")]
    pub commit_timestamp_iso: &'a str,

    /// The labels of the commit used to build the server
    /// sorted list with tags first, then branches
    /// branches are prefixed with `branch:`
    #[serde(rename = "commitLabels")]
    pub commit_labels: Vec<&'a str>,

    /// The iso timestamp when the server was built
    #[serde(rename = "buildTimestampIso")]
    pub build_timestamp_iso: &'a str,
}
