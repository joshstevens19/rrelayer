use crate::postgres::{PostgresClient, PostgresError};

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
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
