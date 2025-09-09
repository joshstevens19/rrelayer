use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.0.
///
/// Creates the complete database schema including:
///
/// **Schemas and Types:**
/// - `authentication` schema with `user_role` enum
/// - `network` schema for blockchain networks
/// - `relayer` schema with `speed` and `tx_status` enums
///
/// **Authentication Tables:**
/// - `authentication.user_access` - User permissions and roles
///
/// **Network Tables:**
/// - `network.record` - Blockchain network configurations
/// - `network.node` - RPC provider endpoints per network
///
/// **Relayer Tables:**
/// - `relayer.record` - Relayer configurations and settings
/// - `relayer.api_key` - API keys for relayer access
/// - `relayer.allowlisted_address` - Address allowlists per relayer
/// - `relayer.transaction` - Transaction records and status
/// - `relayer.transaction_audit_log` - Complete transaction history
///
/// **Rate Limiting Tables:**
/// - `rate_limit_rules` - Per-user rate limiting rules and overrides
/// - `rate_limit_usage` - Time-windowed usage tracking
/// - `transaction_rate_limit_metadata` - Transaction metadata for analytics
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
        CREATE SCHEMA IF NOT EXISTS authentication;
        CREATE SCHEMA IF NOT EXISTS network;
        CREATE SCHEMA IF NOT EXISTS relayer;

        DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'user_role' AND typtype = 'e') THEN
                CREATE TYPE authentication.user_role AS ENUM ('ADMIN', 'READONLY', 'MANAGER', 'INTEGRATOR');
            END IF;
        END;
        $$;

        CREATE TABLE IF NOT EXISTS authentication.user_access (
            address BYTEA PRIMARY KEY NOT NULL,
            role authentication.user_role NOT NULL,
            updated_on TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
        );

        -- Network Schema
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
            PRIMARY KEY (chain_id, provider_url)
        );
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_network_node_chain_id') THEN
                ALTER TABLE network.node DROP CONSTRAINT fk_network_node_chain_id;
            END IF;
            ALTER TABLE network.node ADD CONSTRAINT fk_network_node_chain_id
                FOREIGN KEY (chain_id) REFERENCES network.record (chain_id);
        END;
        $$;

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
            UNIQUE (chain_id, wallet_index)
        );
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_relayer_record_chain_id') THEN
                ALTER TABLE relayer.record DROP CONSTRAINT fk_relayer_record_chain_id;
            END IF;
            ALTER TABLE relayer.record ADD CONSTRAINT fk_relayer_record_chain_id
                FOREIGN KEY (chain_id) REFERENCES network.record (chain_id);
        END;
        $$;

        CREATE TABLE IF NOT EXISTS relayer.api_key (
            api_key CHAR(32) PRIMARY KEY NOT NULL,
            relayer_id UUID NOT NULL,
            deleted BOOLEAN DEFAULT FALSE NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
        );
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_relayer_api_key_relayer_id') THEN
                ALTER TABLE relayer.api_key DROP CONSTRAINT fk_relayer_api_key_relayer_id;
            END IF;
            ALTER TABLE relayer.api_key ADD CONSTRAINT fk_relayer_api_key_relayer_id
                FOREIGN KEY (relayer_id) REFERENCES relayer.record (id);
        END;
        $$;

        CREATE TABLE IF NOT EXISTS relayer.allowlisted_address (
            address BYTEA NOT NULL,
            relayer_id UUID NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            PRIMARY KEY (address, relayer_id)
        );
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_relayer_allowlisted_address_relayer_id') THEN
                ALTER TABLE relayer.allowlisted_address DROP CONSTRAINT fk_relayer_allowlisted_address_relayer_id;
            END IF;
            ALTER TABLE relayer.allowlisted_address ADD CONSTRAINT fk_relayer_allowlisted_address_relayer_id
                FOREIGN KEY (relayer_id) REFERENCES relayer.record (id);
        END;
        $$;

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
            api_key CHAR(32) NOT NULL,
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
            external_id VARCHAR(255) NULL
        );
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_relayer_transaction_relayer_id') THEN
                ALTER TABLE relayer.transaction DROP CONSTRAINT fk_relayer_transaction_relayer_id;
            END IF;
            ALTER TABLE relayer.transaction ADD CONSTRAINT fk_relayer_transaction_relayer_id
                FOREIGN KEY (relayer_id) REFERENCES relayer.record (id);
        END;
        $$;

        CREATE TABLE IF NOT EXISTS relayer.transaction_audit_log (
            history_id SERIAL PRIMARY KEY NOT NULL,
            id UUID NOT NULL,
            relayer_id UUID NOT NULL,
            api_key CHAR(32) NOT NULL,
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
            external_id VARCHAR(255) NULL
        );
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_relayer_transaction_audit_log_relayer_id') THEN
                ALTER TABLE relayer.transaction_audit_log DROP CONSTRAINT fk_relayer_transaction_audit_log_relayer_id;
            END IF;
            ALTER TABLE relayer.transaction_audit_log ADD CONSTRAINT fk_relayer_transaction_audit_log_relayer_id
                FOREIGN KEY (relayer_id) REFERENCES relayer.record (id);
        END;
        $$;

        -- Rate Limiting Tables
        
        -- Store rate limit rules (from config + runtime overrides)
        CREATE TABLE IF NOT EXISTS rate_limit_rules (
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

        -- Track rate limit usage in time windows
        CREATE TABLE IF NOT EXISTS rate_limit_usage (
            id SERIAL PRIMARY KEY,
            user_identifier VARCHAR(255) NOT NULL,
            rule_type VARCHAR(50) NOT NULL,
            window_start TIMESTAMPTZ NOT NULL,
            usage_count BIGINT DEFAULT 0,
            last_request_at TIMESTAMPTZ DEFAULT NOW(),
            created_at TIMESTAMPTZ DEFAULT NOW(),
            UNIQUE(user_identifier, rule_type, window_start)
        );

        -- Index for fast lookups during rate limit checks
        CREATE INDEX IF NOT EXISTS idx_rate_limit_usage_lookup 
        ON rate_limit_usage(user_identifier, rule_type, window_start);

        -- Index for cleanup of old usage records
        CREATE INDEX IF NOT EXISTS idx_rate_limit_usage_cleanup 
        ON rate_limit_usage(window_start);

        -- Store transaction metadata for analytics and user tracking
        CREATE TABLE IF NOT EXISTS transaction_rate_limit_metadata (
            id SERIAL PRIMARY KEY,
            transaction_hash VARCHAR(66),
            relayer_id UUID,
            end_user_address VARCHAR(42), -- The actual end user if determinable
            detection_method VARCHAR(20), -- 'header', 'eip2771', 'fallback'
            transaction_type VARCHAR(20), -- 'direct', 'gasless', 'automated'
            gas_used BIGINT,
            rate_limits_applied JSONB, -- Which rate limits were checked
            created_at TIMESTAMPTZ DEFAULT NOW()
        );

        -- Index for querying transaction metadata
        CREATE INDEX IF NOT EXISTS idx_transaction_metadata_user 
        ON transaction_rate_limit_metadata(end_user_address, created_at);

        CREATE INDEX IF NOT EXISTS idx_transaction_metadata_relayer 
        ON transaction_rate_limit_metadata(relayer_id, created_at);

        -- Function to clean up old rate limit usage records
        CREATE OR REPLACE FUNCTION cleanup_old_rate_limit_usage()
        RETURNS void AS $$
        BEGIN
            -- Delete usage records older than 24 hours
            DELETE FROM rate_limit_usage 
            WHERE window_start < NOW() - INTERVAL '24 hours';
        END;
        $$ LANGUAGE plpgsql;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
