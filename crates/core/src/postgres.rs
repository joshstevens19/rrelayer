use std::{env, future::Future, time::Duration};

use crate::rrelayer_error;
use bb8::{Pool, RunError};
use bb8_postgres::PostgresConnectionManager;
use bytes::Buf;
use dotenv::dotenv;
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use tokio::{task, time::timeout};
pub use tokio_postgres::types::{ToSql, Type as PgType};
use tokio_postgres::{
    config::SslMode, Config, CopyInSink, Error as PgError, Row, Statement, ToStatement,
};
use tracing::error;

pub fn connection_string() -> Result<String, env::VarError> {
    dotenv().ok();
    let connection = env::var("DATABASE_URL")?;
    Ok(connection)
}

#[derive(thiserror::Error, Debug)]
pub enum PostgresConnectionError {
    #[error("The database connection string is wrong please check your environment: {0}")]
    DatabaseConnectionConfigWrong(#[from] env::VarError),

    #[error("Connection pool error: {0}")]
    ConnectionPoolError(#[from] tokio_postgres::Error),

    #[error("Connection pool runtime error: {0}")]
    ConnectionPoolRuntimeError(#[from] RunError<tokio_postgres::Error>),

    #[error("Can not connect to the database please make sure your connection string is correct")]
    CanNotConnectToDatabase,

    #[error("Could not parse connection string make sure it is correctly formatted")]
    CouldNotParseConnectionString,

    #[error("Could not create tls connector")]
    CouldNotCreateTlsConnector,
}

#[derive(thiserror::Error, Debug)]
pub enum PostgresError {
    #[error("PgError {0}")]
    PgError(#[from] PgError),

    #[error("Connection pool error: {0}")]
    ConnectionPoolError(#[from] RunError<tokio_postgres::Error>),
}

pub struct PostgresClient {
    pub pool: Pool<PostgresConnectionManager<MakeTlsConnector>>,
}

impl PostgresClient {
    /// Creates a new PostgreSQL client with connection pooling.
    pub async fn new() -> Result<Self, PostgresConnectionError> {
        async fn _new(disable_ssl: bool) -> Result<PostgresClient, PostgresConnectionError> {
            let connection_str = connection_string()?;
            let mut config: Config = connection_str
                .parse()
                .map_err(|_| PostgresConnectionError::CouldNotParseConnectionString)?;

            if disable_ssl {
                config.ssl_mode(SslMode::Disable);
            }

            let connector = TlsConnector::builder()
                .build()
                .map_err(|_| PostgresConnectionError::CouldNotCreateTlsConnector)?;
            let tls_connector = MakeTlsConnector::new(connector);

            let (client, connection) =
                match timeout(Duration::from_millis(5000), config.connect(tls_connector.clone()))
                    .await
                {
                    Ok(Ok((client, connection))) => (client, connection),
                    Ok(Err(e)) => {
                        // retry without ssl if ssl has been attempted and failed
                        if !disable_ssl
                            && config.get_ssl_mode() != SslMode::Disable
                            && !connection_str.contains("sslmode=require")
                        {
                            return Box::pin(_new(true)).await;
                        }
                        rrelayer_error!("Error connecting to database: {}", e);
                        return Err(PostgresConnectionError::CanNotConnectToDatabase);
                    }
                    Err(e) => {
                        rrelayer_error!("Timeout connecting to database: {}", e);
                        return Err(PostgresConnectionError::CanNotConnectToDatabase);
                    }
                };

            let connection_handle = task::spawn(connection);

            match client.query_one("SELECT 1", &[]).await {
                Ok(_) => {}
                Err(_) => return Err(PostgresConnectionError::CanNotConnectToDatabase),
            };

            drop(client);
            match connection_handle.await {
                Ok(Ok(())) => (),
                Ok(Err(_)) => return Err(PostgresConnectionError::CanNotConnectToDatabase),
                Err(_) => return Err(PostgresConnectionError::CanNotConnectToDatabase),
            }

            let manager = PostgresConnectionManager::new(config, tls_connector);

            let pool = Pool::builder().build(manager).await?;

            Ok(PostgresClient { pool })
        }

        _new(false).await
    }

    /// Executes multiple SQL statements in batch.
    pub async fn batch_execute(&self, sql: &str) -> Result<(), PostgresError> {
        let conn = self.pool.get().await?;
        conn.batch_execute(sql).await.map_err(PostgresError::PgError)
    }

    /// Executes a single SQL statement.
    pub async fn execute<T>(
        &self,
        query: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, PostgresError>
    where
        T: ?Sized + ToStatement,
    {
        let conn = self.pool.get().await?;
        conn.execute(query, params).await.map_err(PostgresError::PgError)
    }

    /// Prepares a SQL statement with explicit parameter types.
    pub async fn prepare(
        &self,
        query: &str,
        parameter_types: &[PgType],
    ) -> Result<Statement, PostgresError> {
        let conn = self.pool.get().await?;
        conn.prepare_typed(query, parameter_types).await.map_err(PostgresError::PgError)
    }

    pub async fn with_transaction<F, Fut, T, Q>(
        &self,
        query: &Q,
        params: &[&(dyn ToSql + Sync)],
        f: F,
    ) -> Result<T, PostgresError>
    where
        F: FnOnce(u64) -> Fut + Send,
        Fut: Future<Output = Result<T, PostgresError>> + Send,
        Q: ?Sized + ToStatement,
    {
        let mut conn = self.pool.get().await.map_err(PostgresError::ConnectionPoolError)?;
        let transaction = conn.transaction().await.map_err(PostgresError::PgError)?;

        let count = transaction.execute(query, params).await.map_err(PostgresError::PgError)?;

        let result = f(count).await?;

        transaction.commit().await.map_err(PostgresError::PgError)?;

        Ok(result)
    }

    /// Executes a query and returns all matching rows.
    pub async fn query<T>(
        &self,
        query: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, PostgresError>
    where
        T: ?Sized + ToStatement,
    {
        let conn = self.pool.get().await?;
        let rows = conn.query(query, params).await.map_err(PostgresError::PgError)?;
        Ok(rows)
    }

    /// Executes a query that is expected to return exactly one row.
    pub async fn query_one<T>(
        &self,
        query: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, PostgresError>
    where
        T: ?Sized + ToStatement,
    {
        let conn = self.pool.get().await?;
        let row = conn.query_one(query, params).await.map_err(PostgresError::PgError)?;
        Ok(row)
    }

    /// Executes a query that may return zero or one row.
    pub async fn query_one_or_none<T>(
        &self,
        query: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, PostgresError>
    where
        T: ?Sized + ToStatement,
    {
        let conn = self.pool.get().await?;
        let row = conn.query_opt(query, params).await.map_err(PostgresError::PgError)?;
        Ok(row)
    }

    /// Performs a batch insert operation within a transaction.
    pub async fn batch_insert<T>(
        &self,
        query: &T,
        params_list: Vec<Vec<Box<dyn ToSql + Send + Sync>>>,
    ) -> Result<(), PostgresError>
    where
        T: ?Sized + ToStatement,
    {
        let mut conn = self.pool.get().await?;
        let transaction = conn.transaction().await.map_err(PostgresError::PgError)?;

        for params in params_list {
            let params_refs: Vec<&(dyn ToSql + Sync)> =
                params.iter().map(|param| param.as_ref() as &(dyn ToSql + Sync)).collect();
            transaction.execute(query, &params_refs).await.map_err(PostgresError::PgError)?;
        }

        transaction.commit().await.map_err(PostgresError::PgError)?;
        Ok(())
    }

    /// Creates a COPY IN sink for high-performance bulk data loading.
    pub async fn copy_in<T, U>(&self, statement: &T) -> Result<CopyInSink<U>, PostgresError>
    where
        T: ?Sized + ToStatement,
        U: Buf + 'static + Send,
    {
        let conn = self.pool.get().await?;

        conn.copy_in(statement).await.map_err(PostgresError::PgError)
    }
}
