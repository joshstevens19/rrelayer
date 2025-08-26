/// Generates a Docker Compose configuration for PostgreSQL.
///
/// Returns a static string containing a docker-compose.yml configuration
/// that sets up a PostgreSQL 16 database with persistent storage.
///
/// The configuration includes:
/// - PostgreSQL 16 image
/// - Port mapping 5441:5432 (to avoid conflicts with local PostgreSQL)
/// - Persistent volume for data storage
/// - Environment variable loading from .env file
///
/// # Returns
/// * `&'static str` - Docker Compose YAML configuration as a string
pub fn generate_docker_file() -> &'static str {
    r#"volumes:
  postgres_data:
    driver: local

services:
  postgresql:
    image: postgres:16
    shm_size: 1g
    restart: always
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - 5441:5432
    env_file:
      - ./.env
 "#
}
