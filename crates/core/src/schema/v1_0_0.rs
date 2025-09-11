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
            eip_1559_enabled BOOLEAN DEFAULT FALSE NOT NULL,
            deleted BOOLEAN DEFAULT FALSE NOT NULL,
            updated_on TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            UNIQUE (chain_id, wallet_index),
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

        CREATE SCHEMA IF NOT EXISTS rate_limit;

        CREATE TABLE IF NOT EXISTS rate_limit.rules (
            id SERIAL PRIMARY KEY,
            user_identifier VARCHAR(255) NOT NULL, -- Address, relayer_id, or special identifier
            rule_type VARCHAR(50) NOT NULL, -- 'transactions_per_minute', 'gas_per_hour', etc.
            limit_value BIGINT NOT NULL,
            window_duration_seconds INTEGER NOT NULL,
            is_unlimited BOOLEAN DEFAULT FALSE,
            created_at TIMESTAMPTZ DEFAULT NOW(),
            updated_at TIMESTAMPTZ DEFAULT NOW(),
            UNIQUE(user_identifier, rule_type)
        );

        CREATE TABLE IF NOT EXISTS rate_limit.usage (
            id SERIAL PRIMARY KEY,
            user_identifier VARCHAR(255) NOT NULL,
            rule_type VARCHAR(50) NOT NULL,
            window_start TIMESTAMPTZ NOT NULL,
            usage_count BIGINT DEFAULT 0,
            last_request_at TIMESTAMPTZ DEFAULT NOW(),
            created_at TIMESTAMPTZ DEFAULT NOW(),
            UNIQUE(user_identifier, rule_type, window_start)
        );

        CREATE INDEX IF NOT EXISTS idx_rate_limit_usage_lookup 
        ON rate_limit.usage(user_identifier, rule_type, window_start);

        CREATE INDEX IF NOT EXISTS idx_rate_limit_usage_cleanup 
        ON rate_limit.usage(window_start);

        CREATE TABLE IF NOT EXISTS rate_limit.transaction_metadata (
            id SERIAL PRIMARY KEY,
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

        CREATE OR REPLACE FUNCTION cleanup_old_rate_limit_usage()
        RETURNS void AS $$
        BEGIN
            DELETE FROM rate_limit.usage
            WHERE window_start < NOW() - INTERVAL '24 hours';
        END;
        $$ LANGUAGE plpgsql;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
