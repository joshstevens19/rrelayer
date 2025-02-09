use std::path::Path;
use dotenv::{dotenv, from_path};

pub fn load_env_from_project_path(project_path: &Path) {
    if from_path(project_path.join(".env")).is_err() {
        dotenv().ok();
    }
}
