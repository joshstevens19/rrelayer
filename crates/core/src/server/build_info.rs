pub const BUILD_COMMIT_HASH: &str = match option_env!("BUILD_COMMIT_HASH") {
    Some(v) => v,
    None => "0000000000000000000000000000000000000000",
};

pub const BUILD_COMMIT_TIMESTAMP_ISO: &str = match option_env!("BUILD_COMMIT_TIMESTAMP_ISO") {
    Some(v) => v,
    None => "1970-01-01T00:00:00.000Z",
};

pub const BUILD_COMMIT_LABELS: &str = match option_env!("BUILD_COMMIT_LABELS") {
    Some(v) => v,
    None => "",
};

pub const BUILD_TIMESTAMP_ISO: &str = env!("BUILD_TIMESTAMP_ISO");
