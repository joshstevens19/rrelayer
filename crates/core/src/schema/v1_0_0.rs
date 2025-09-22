use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.0.
///
/// All tables include appropriate constraints, indexes, and foreign key
/// relationships. The schema uses PostgreSQL-specific features like enums
/// and TIMESTAMPTZ for proper timezone handling.
///
/// # Arguments
/// * `client` - PostgreSQL client with schema creation permissions
///
/// # Returns
/// * `Ok(())` - If schema creation succeeds
/// * `Err(PostgresError)` - If any schema operation fails
pub async fn apply_v1_0_0_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        CREATE SCHEMA IF NOT EXISTS public;
        CREATE SCHEMA IF NOT EXISTS network;
        CREATE SCHEMA IF NOT EXISTS relayer;
        CREATE SCHEMA IF NOT EXISTS signing;
        CREATE SCHEMA IF NOT EXISTS rate_limit;
        CREATE SCHEMA IF NOT EXISTS webhook;

        CREATE TABLE IF NOT EXISTS network.record (
            chain_id BIGINT PRIMARY KEY NOT NULL,
            name VARCHAR(50) NOT NULL,
            disabled BOOLEAN DEFAULT FALSE NOT NULL,
            updated_on TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
        );

        CREATE TABLE IF NOT EXISTS network.node (
            chain_id BIGINT NOT NULL,
            provider_url VARCHAR(200) NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            PRIMARY KEY (chain_id, provider_url),
            CONSTRAINT fk_network_node_chain_id
                FOREIGN KEY (chain_id)
                    REFERENCES network.record (chain_id)
        );

        CREATE TABLE IF NOT EXISTS relayer.record (
            id UUID PRIMARY KEY NOT NULL,
            name VARCHAR(50) NOT NULL,
            chain_id BIGINT NOT NULL,
            address BYTEA NULL,
            wallet_index INT NOT NULL,
            max_gas_price_cap NUMERIC(80) NULL,
            paused BOOLEAN DEFAULT FALSE NOT NULL,
            allowlisted_addresses_only BOOLEAN DEFAULT FALSE NOT NULL,
            eip_1559_enabled BOOLEAN DEFAULT TRUE NOT NULL,
            deleted BOOLEAN DEFAULT FALSE NOT NULL,
            updated_on TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            CONSTRAINT fk_relayer_record_chain_id
                FOREIGN KEY (chain_id)
                    REFERENCES network.record (chain_id)
        );

        CREATE TABLE IF NOT EXISTS relayer.allowlisted_address (
            address BYTEA NOT NULL,
            relayer_id UUID NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            PRIMARY KEY (address, relayer_id),
            CONSTRAINT fk_relayer_allowlisted_address_relayer_id
                FOREIGN KEY (relayer_id)
                    REFERENCES relayer.record (id)
        );

        DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'speed' AND typtype = 'e') THEN
                CREATE TYPE relayer.speed AS ENUM ('SUPER', 'FAST', 'MEDIUM', 'SLOW');
            END IF;
        END;
        $$;

        DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'tx_status' AND typtype = 'e') THEN
                CREATE TYPE relayer.tx_status AS ENUM ('PENDING', 'INMEMPOOL', 'MINED', 'CONFIRMED', 'FAILED', 'EXPIRED');
            END IF;
        END;
        $$;

        CREATE TABLE IF NOT EXISTS relayer.transaction (
            id UUID PRIMARY KEY NOT NULL,
            relayer_id UUID NOT NULL,
            "to" BYTEA NOT NULL,
            "from" BYTEA NOT NULL,
            nonce BIGINT NOT NULL,
            data BYTEA NULL,
            value NUMERIC(80) NOT NULL,
            chain_id BIGINT NOT NULL,
            gas_price NUMERIC NULL,
            sent_max_priority_fee_per_gas NUMERIC(80) NULL,
            sent_max_fee_per_gas NUMERIC(80) NULL,
            gas_limit NUMERIC(80) NULL,
            block_hash BYTEA NULL,
            block_number BIGINT NULL,
            hash BYTEA NULL,
            speed relayer.speed NOT NULL,
            status relayer.tx_status NOT NULL,
            blobs BYTEA[] NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            expired_at TIMESTAMPTZ NULL,
            queued_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            mined_at TIMESTAMPTZ NULL,
            failed_at TIMESTAMPTZ NULL,
            failed_reason TEXT NULL,
            sent_at TIMESTAMPTZ NULL,
            confirmed_at TIMESTAMPTZ NULL,
            external_id VARCHAR(255) NULL,
            CONSTRAINT fk_relayer_transaction_relayer_id
               FOREIGN KEY (relayer_id)
                   REFERENCES relayer.record (id)
        );

        CREATE TABLE IF NOT EXISTS relayer.transaction_audit_log (
            history_id SERIAL PRIMARY KEY NOT NULL,
            id UUID NOT NULL,
            relayer_id UUID NOT NULL,
            "to" BYTEA NOT NULL,
            "from" BYTEA NOT NULL,
            nonce BIGINT NOT NULL,
            data BYTEA NULL,
            value NUMERIC(80) NULL,
            chain_id BIGINT NOT NULL,
            gas_price NUMERIC NULL,
            sent_max_priority_fee_per_gas NUMERIC(80) NULL,
            sent_max_fee_per_gas NUMERIC(80) NULL,
            gas_limit NUMERIC(80) NULL,
            block_hash BYTEA NULL,
            block_number BIGINT NULL,
            hash BYTEA NULL,
            speed relayer.speed NOT NULL,
            status relayer.tx_status NOT NULL,
            blobs BYTEA[] NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            expired_at TIMESTAMPTZ NULL,
            queued_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            mined_at TIMESTAMPTZ NULL,
            failed_at TIMESTAMPTZ NULL,
            failed_reason TEXT NULL,
            sent_at TIMESTAMPTZ NULL,
            confirmed_at TIMESTAMPTZ NULL,
            external_id VARCHAR(255) NULL,
            CONSTRAINT fk_relayer_transaction_audit_log_relayer_id
               FOREIGN KEY (relayer_id)
                   REFERENCES relayer.record (id)
        );

        CREATE INDEX IF NOT EXISTS idx_transaction_relayer_status_nonce 
        ON relayer.transaction(relayer_id, status, nonce ASC);

        CREATE INDEX IF NOT EXISTS idx_transaction_relayer_id 
        ON relayer.transaction(relayer_id);

        CREATE INDEX IF NOT EXISTS idx_transaction_expires_at 
        ON relayer.transaction(expires_at);

        CREATE INDEX IF NOT EXISTS idx_transaction_status 
        ON relayer.transaction(status);

        CREATE INDEX IF NOT EXISTS idx_relayer_chain_deleted 
        ON relayer.record(chain_id, deleted);

        CREATE INDEX IF NOT EXISTS idx_relayer_address_chain 
        ON relayer.record(address, chain_id, deleted) WHERE address IS NOT NULL;

        CREATE INDEX IF NOT EXISTS idx_relayer_chain_wallet_index 
        ON relayer.record(chain_id, wallet_index);

        CREATE INDEX IF NOT EXISTS idx_relayer_deleted 
        ON relayer.record(deleted);

        CREATE INDEX IF NOT EXISTS idx_network_disabled 
        ON network.record(disabled);

        CREATE INDEX IF NOT EXISTS idx_allowlist_relayer_created 
        ON relayer.allowlisted_address(relayer_id, created_at DESC);

        CREATE TABLE IF NOT EXISTS rate_limit.transaction_metadata (
            id SERIAL PRIMARY KEY NOT NULL,
            transaction_hash VARCHAR(66),
            relayer_id UUID,
            end_user_address VARCHAR(42),
            detection_method VARCHAR(20),
            transaction_type VARCHAR(20),
            gas_used BIGINT,
            rate_limits_applied JSONB,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );

        CREATE INDEX IF NOT EXISTS idx_transaction_metadata_user 
        ON rate_limit.transaction_metadata(end_user_address, created_at);

        CREATE INDEX IF NOT EXISTS idx_transaction_metadata_relayer 
        ON rate_limit.transaction_metadata(relayer_id, created_at);

        CREATE TABLE IF NOT EXISTS signing.text_history (
            id SERIAL PRIMARY KEY NOT NULL,
            relayer_id UUID NOT NULL,
            message TEXT NOT NULL,
            signature BYTEA NOT NULL,
            chain_id BIGINT NOT NULL,
            signed_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            CONSTRAINT fk_signing_text_relayer_id
                FOREIGN KEY (relayer_id) 
                REFERENCES relayer.record (id)
        );

        CREATE TABLE IF NOT EXISTS signing.typed_data_history (
            id SERIAL PRIMARY KEY NOT NULL,
            relayer_id UUID NOT NULL,
            domain_data JSONB NOT NULL,
            message_data JSONB NOT NULL,
            primary_type VARCHAR(100) NOT NULL,
            signature BYTEA NOT NULL,
            chain_id BIGINT NOT NULL,
            signed_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            CONSTRAINT fk_signing_typed_data_relayer_id
                FOREIGN KEY (relayer_id)
                REFERENCES relayer.record (id)
        );

        CREATE INDEX IF NOT EXISTS idx_signing_text_relayer_time 
        ON signing.text_history(relayer_id, signed_at DESC);

        CREATE INDEX IF NOT EXISTS idx_signing_typed_data_relayer_time 
        ON signing.typed_data_history(relayer_id, signed_at DESC);

        DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON t.typnamespace = n.oid WHERE t.typname = 'status' AND n.nspname = 'webhook' AND t.typtype = 'e') THEN
                CREATE TYPE webhook.status AS ENUM ('PENDING', 'DELIVERED', 'FAILED', 'ABANDONED');
            END IF;
        END;
        $$;

        DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON t.typnamespace = n.oid WHERE t.typname = 'event_type' AND n.nspname = 'webhook' AND t.typtype = 'e') THEN
                CREATE TYPE webhook.event_type AS ENUM (
                    'TRANSACTION_QUEUED', 'TRANSACTION_SENT', 'TRANSACTION_MINED', 
                    'TRANSACTION_CONFIRMED', 'TRANSACTION_FAILED', 'TRANSACTION_EXPIRED',
                    'TRANSACTION_CANCELLED', 'TRANSACTION_REPLACED', 'TEXT_SIGNED', 'TYPED_DATA_SIGNED'
                );
            END IF;
        END;
        $$;

        CREATE TABLE IF NOT EXISTS webhook.delivery_history (
            id UUID PRIMARY KEY NOT NULL,
            webhook_endpoint VARCHAR(500) NOT NULL,
            event_type webhook.event_type NOT NULL,
            status webhook.status NOT NULL,
            transaction_id UUID NULL,
            relayer_id UUID NULL,
            chain_id BIGINT NULL,
            attempts INTEGER DEFAULT 1 NOT NULL,
            max_retries INTEGER NOT NULL,
            payload JSONB NOT NULL,
            headers JSONB NULL,
            http_status_code INTEGER NULL,
            response_body TEXT NULL,
            error_message TEXT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            first_attempt_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            last_attempt_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            delivered_at TIMESTAMPTZ NULL,
            abandoned_at TIMESTAMPTZ NULL,
            total_duration_ms BIGINT NULL,
            CONSTRAINT fk_webhook_delivery_relayer_id
                FOREIGN KEY (relayer_id)
                    REFERENCES relayer.record (id),
            CONSTRAINT fk_webhook_delivery_transaction_id
                FOREIGN KEY (transaction_id)
                    REFERENCES relayer.transaction (id),
            CONSTRAINT fk_webhook_delivery_chain_id
                FOREIGN KEY (chain_id)
                    REFERENCES network.record (chain_id)
        );

        CREATE INDEX IF NOT EXISTS idx_webhook_delivery_endpoint_status 
        ON webhook.delivery_history(webhook_endpoint, status, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_webhook_delivery_relayer_time 
        ON webhook.delivery_history(relayer_id, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_webhook_delivery_transaction 
        ON webhook.delivery_history(transaction_id, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_webhook_delivery_event_type 
        ON webhook.delivery_history(event_type, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_webhook_delivery_status_time 
        ON webhook.delivery_history(status, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_webhook_delivery_cleanup 
        ON webhook.delivery_history(created_at);

        CREATE OR REPLACE FUNCTION cleanup_old_webhook_deliveries()
        RETURNS void AS $$
        BEGIN
            DELETE FROM webhook.delivery_history
            WHERE created_at < NOW() - INTERVAL '30 days';
        END;
        $$ LANGUAGE plpgsql;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
