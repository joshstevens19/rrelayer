//! Real PostgreSQL fault injection for the admission persistence boundary.
//! DATABASE_URL must permit creating a disposable database; none of its schemas
//! are changed. Tests skip when no database was requested.

use super::shield_admission;
use crate::{
    postgres::PostgresClient,
    schema::apply_schema,
    transaction::{
        db::SaveTransactionError,
        types::{test_fixtures::test_transaction, TransactionId, TransactionStatus},
    },
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_postgres::Config;

async fn client(config: Config) -> PostgresClient {
    let tls = postgres_native_tls::MakeTlsConnector::new(
        native_tls::TlsConnector::builder().build().unwrap(),
    );
    let manager = bb8_postgres::PostgresConnectionManager::new(config, tls);
    PostgresClient { pool: bb8::Pool::builder().build(manager).await.unwrap() }
}

/// Drop exactly one COMMIT acknowledgement after PostgreSQL has committed. This
/// exercises a genuine ambiguous driver error, rather than pretending a failed
/// INSERT might have committed.
async fn commit_ack_proxy(
    config: &Config,
) -> (Config, Arc<AtomicBool>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let host = match &config.get_hosts()[0] {
        tokio_postgres::config::Host::Tcp(host) => host.clone(),
        _ => panic!("admission regression requires a TCP DATABASE_URL"),
    };
    let upstream_port = config.get_ports().first().copied().unwrap_or(5432);
    let armed = Arc::new(AtomicBool::new(false));
    let armed_task = armed.clone();
    let task = tokio::spawn(async move {
        while let Ok((downstream, _)) = listener.accept().await {
            let upstream =
                tokio::net::TcpStream::connect((host.as_str(), upstream_port)).await.unwrap();
            let armed = armed_task.clone();
            tokio::spawn(async move {
                let (mut client_read, mut client_write) = downstream.into_split();
                let (mut server_read, mut server_write) = upstream.into_split();
                tokio::select! {
                    _ = tokio::io::copy(&mut client_read, &mut server_write) => {}
                    _ = async {
                        // Parse backend frames so split TCP packets cannot hide
                        // the COMMIT CommandComplete message from the injection.
                        loop {
                            let kind = server_read.read_u8().await?;
                            let length = server_read.read_u32().await?;
                            let mut body = vec![0; length as usize - 4];
                            server_read.read_exact(&mut body).await?;
                            if kind == b'C' && body == b"COMMIT\0"
                                && armed.swap(false, Ordering::SeqCst)
                            {
                                return Ok::<(), std::io::Error>(());
                            }
                            client_write.write_u8(kind).await?;
                            client_write.write_u32(length).await?;
                            client_write.write_all(&body).await?;
                        }
                    } => {}
                }
            });
        }
    });
    let mut proxy = Config::new();
    proxy.host("127.0.0.1").port(port).ssl_mode(tokio_postgres::config::SslMode::Disable);
    if let Some(user) = config.get_user() {
        proxy.user(user);
    }
    if let Some(password) = config.get_password() {
        proxy.password(password);
    }
    if let Some(dbname) = config.get_dbname() {
        proxy.dbname(dbname);
    }
    (proxy, armed, task)
}

#[tokio::test]
async fn admission_database_failures_cancellation_and_commit_settlement() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping admission database regression");
        return;
    };
    let mut config: Config = url.parse().unwrap();
    config.ssl_mode(tokio_postgres::config::SslMode::Disable);
    let admin = client(config.clone()).await;
    let name = format!("rrelayer_admission_{}", uuid::Uuid::new_v4().simple());
    admin.batch_execute(&format!("CREATE DATABASE {name}")).await.unwrap();
    config.dbname(&name);
    let mut db = client(config.clone()).await;
    apply_schema(&db).await.unwrap();

    let transaction = test_transaction(0, TransactionStatus::PENDING);
    db.execute(
        "INSERT INTO relayer.record (id, name, chain_id, wallet_index, address) VALUES ($1, 'admission-test', $2, 0, $3)",
        &[&transaction.relayer_id, &transaction.chain_id, &transaction.from],
    ).await.unwrap();

    // A rejected INSERT cannot reserve a nonce. The identical transaction can
    // then be saved at nonce zero; the primary and audit writes stay atomic.
    db.batch_execute(
        "ALTER TABLE relayer.transaction ADD CONSTRAINT reject_admission CHECK (nonce < 0)",
    )
    .await
    .unwrap();
    assert!(matches!(
        db.save_transaction_for_admission(&transaction.relayer_id, &transaction, None).await,
        Err(SaveTransactionError::BeforeCommit(_))
    ));
    assert!(db.settle_transaction_admission(&transaction.id).await.unwrap().is_none());
    db.batch_execute("ALTER TABLE relayer.transaction DROP CONSTRAINT reject_admission")
        .await
        .unwrap();
    db.save_transaction_for_admission(&transaction.relayer_id, &transaction, None).await.unwrap();
    // Duplicate IDs fail before COMMIT: they must never adopt the existing row
    // a second time as though the duplicate were an ambiguous save.
    assert!(matches!(
        db.save_transaction_for_admission(&transaction.relayer_id, &transaction, None).await,
        Err(SaveTransactionError::BeforeCommit(_))
    ));

    // A COMMIT rejection proves the absent outcome through the same settlement
    // path used for transport failures at that boundary.
    db.batch_execute("CREATE FUNCTION reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected commit rejection'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON relayer.transaction DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_commit()").await.unwrap();
    let mut rejected = transaction.clone();
    rejected.id = TransactionId::new();
    assert!(matches!(
        db.save_transaction_for_admission(&rejected.relayer_id, &rejected, None).await,
        Err(SaveTransactionError::CommitOutcomeUnknown(_))
    ));
    assert!(db.settle_transaction_admission(&rejected.id).await.unwrap().is_none());
    db.batch_execute(
        "DROP TRIGGER reject_commit ON relayer.transaction; DROP FUNCTION reject_commit()",
    )
    .await
    .unwrap();

    let (proxy_config, armed, proxy_task) = commit_ack_proxy(&config).await;
    let mut proxy_db = client(proxy_config).await;
    let mut lost_ack = transaction.clone();
    lost_ack.id = TransactionId::new();
    armed.store(true, Ordering::SeqCst);
    assert!(matches!(
        proxy_db.save_transaction_for_admission(&lost_ack.relayer_id, &lost_ack, None).await,
        Err(SaveTransactionError::CommitOutcomeUnknown(_))
    ));
    assert!(!armed.load(Ordering::SeqCst), "COMMIT acknowledgement was actually dropped");
    let adopted = db.settle_transaction_admission(&lost_ack.id).await.unwrap().unwrap();
    assert_eq!(adopted.id, lost_ack.id);
    assert_eq!(adopted.nonce, lost_ack.nonce);

    // Do not return an absent result while the original transaction still owns
    // the fence: its COMMIT may yet make the row visible.
    let mut conn = db.pool.get().await.unwrap();
    let original = conn.transaction().await.unwrap();
    let uncertain_id = TransactionId::new();
    original
        .query_one(
            "SELECT pg_advisory_xact_lock(1919249505, hashtext($1))",
            &[&uncertain_id.to_string()],
        )
        .await
        .unwrap();
    let settling_db = PostgresClient { pool: db.pool.clone() };
    let mut settlement =
        tokio::spawn(async move { settling_db.settle_transaction_admission(&uncertain_id).await });
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), &mut settlement)
        .await
        .is_err());
    original.rollback().await.unwrap();
    assert!(settlement.await.unwrap().unwrap().is_none());
    drop(conn);

    // A replacement process cannot restore its nonce high-water from a snapshot
    // taken before a late COMMIT from the old process finishes.
    let mut conn = db.pool.get().await.unwrap();
    let old_process = conn.transaction().await.unwrap();
    old_process.query_one("SELECT pg_advisory_xact_lock_shared(1919249506, 0)", &[]).await.unwrap();
    old_process
        .execute("UPDATE relayer.transaction SET nonce = 12 WHERE id = $1", &[&transaction.id])
        .await
        .unwrap();
    let startup_db = PostgresClient { pool: db.pool.clone() };
    let transaction_id = transaction.id;
    let mut startup = tokio::spawn(async move {
        startup_db.wait_for_transaction_admissions().await.unwrap();
        startup_db.get_transaction(&transaction_id).await.unwrap().unwrap().nonce
    });
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), &mut startup)
        .await
        .is_err());
    old_process.commit().await.unwrap();
    assert_eq!(startup.await.unwrap().into_inner(), 12);
    drop(conn);

    // Cancellation before a worker obtains the shared startup barrier must
    // prevent a late save after a replacement process has loaded its snapshot.
    let mut conn = db.pool.get().await.unwrap();
    let reloading = conn.transaction().await.unwrap();
    reloading.query_one("SELECT pg_advisory_xact_lock(1919249506, 0)", &[]).await.unwrap();
    let mut late = transaction.clone();
    late.id = TransactionId::new();
    let late_id = late.id;
    let mut late_db = PostgresClient { pool: db.pool.clone() };
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let caller = tokio::spawn(async move {
        shield_admission(move |ready| async move {
            let result =
                late_db.save_transaction_for_admission(&late.relayer_id, &late, Some(ready)).await;
            let _ = done_tx.send(result);
        })
        .await
    });
    for _ in 0..100 {
        let waiting = db.query_one("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND classid = 1919249506 AND NOT granted)", &[]).await.unwrap().get::<_, bool>(0);
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(db.query_one("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND classid = 1919249506 AND NOT granted)", &[]).await.unwrap().get::<_, bool>(0));
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    reloading.commit().await.unwrap();
    assert!(!done_rx.await.unwrap().unwrap(), "late worker must skip persistence");
    assert!(db.settle_transaction_admission(&late_id).await.unwrap().is_none());
    drop(conn);

    // Cancel a real caller after its save is waiting on PostgreSQL. The detached
    // critical section must still persist once, release the queue lock and adopt.
    let mut conn = db.pool.get().await.unwrap();
    let blocker = conn.transaction().await.unwrap();
    blocker.batch_execute("LOCK TABLE relayer.transaction IN ACCESS EXCLUSIVE MODE").await.unwrap();
    let mut cancelled = transaction.clone();
    cancelled.id = TransactionId::new();
    let cancelled_id = cancelled.id;
    let mut cancelled_db = PostgresClient { pool: db.pool.clone() };
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let caller = tokio::spawn(async move {
        shield_admission(move |ready| async move {
            let result = cancelled_db
                .save_transaction_for_admission(&cancelled.relayer_id, &cancelled, Some(ready))
                .await;
            let _ = done_tx.send(result);
        })
        .await
    });
    for _ in 0..100 {
        let waiting = db.query_one("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE relation = 'relayer.transaction'::regclass AND NOT granted)", &[]).await.unwrap().get::<_, bool>(0);
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(db.query_one("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE relation = 'relayer.transaction'::regclass AND NOT granted)", &[]).await.unwrap().get::<_, bool>(0));
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    blocker.commit().await.unwrap();
    done_rx.await.unwrap().unwrap();
    assert!(db.settle_transaction_admission(&cancelled_id).await.unwrap().is_some());
    let rows: i64 = db
        .query_one(
            "SELECT count(*) FROM relayer.transaction_audit_log WHERE id = $1",
            &[&cancelled_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(rows, 1, "caller cancellation must not duplicate durable admission");

    drop(conn);
    drop(db);
    drop(proxy_db);
    proxy_task.abort();
    admin.batch_execute(&format!("DROP DATABASE {name} WITH (FORCE)")).await.unwrap();
}
