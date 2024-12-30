CREATE TABLE "user_role" (
   "name" varchar(50) PRIMARY KEY not null
);

INSERT INTO "user_role" ("name") VALUES ('ADMIN'), ('READONLY'), ('MANAGER'), ('INTEGRATOR');

CREATE TABLE "user" (
  "address" char(42) PRIMARY KEY not null,
  "role" varchar(50) null,
  "updated_on" timestamp DEFAULT NOW() not null,
  "created_at" timestamp DEFAULT NOW() not null
);

ALTER TABLE "user" ADD CONSTRAINT "fk_user__role" FOREIGN KEY ("role") REFERENCES "user_role" ("name");

CREATE TABLE "network" (
  "chain_id" BIGINT PRIMARY KEY not null,
  "name" varchar(50) null,
  "disabled" boolean DEFAULT FALSE not null,
  "updated_on" timestamp DEFAULT NOW() not null,
  "created_at" timestamp DEFAULT NOW() not null
);

-- can have many providers per network to split the traffic between nodes
CREATE TABLE "network_nodes" (
  "chain_id" BIGINT not null,
  "provider_url" varchar(200) not null,
  "created_at" timestamp DEFAULT NOW() not null,
  PRIMARY KEY ("chain_id", "provider_url")
);

ALTER TABLE "network_nodes" ADD CONSTRAINT "fk_network_nodes_chain_id" FOREIGN KEY ("chain_id") REFERENCES "network" ("chain_id");

CREATE TABLE "relayer_speed" (
   "name" varchar(50) PRIMARY KEY not null
);

INSERT INTO "relayer_speed" ("name") VALUES ('SUPER'), ('FAST'), ('MEDIUM'), ('SLOW');

CREATE TABLE "relayer" (
  "id" uuid PRIMARY KEY not null,
  "name" varchar(50) not null,
  "chain_id" BIGINT not null,
  -- this is lazy loaded in after creation as it wont know it
  "address" char(42) null,
  "wallet_index" int not null,
  "max_gas_price_cap" varchar(100) null,
  "paused" boolean DEFAULT false not null,
  "allowlisted_addresses_only" boolean DEFAULT false not null,
  "eip_1559_enabled" boolean DEFAULT false not null,
  "deleted" boolean DEFAULT FALSE not null,
  "updated_on" timestamp DEFAULT NOW() not null,
  "created_at" timestamp DEFAULT NOW() not null,
  UNIQUE ("chain_id", "wallet_index")
);

ALTER TABLE "relayer" ADD CONSTRAINT "fk_relayer__chain_id" FOREIGN KEY ("chain_id") REFERENCES "network" ("chain_id");

CREATE or REPLACE VIEW  relayer_view AS
	SELECT 
      "id",
      "name",
      "chain_id",
      "address",
      "wallet_index",
      "max_gas_price_cap", 
      "paused",
      "allowlisted_addresses_only",
      "eip_1559_enabled",
      "created_at",
      "deleted"
  FROM "relayer";

CREATE TABLE "relayer_audit_log" (
  "history_id" SERIAL PRIMARY KEY NOT NULL,
  "id" uuid not null,
  "name" varchar(50) not null,
  "chain_id" BIGINT not null,
  "address" char(42) not null,
  "wallet_index" int not null,
  "max_gas_price_cap" varchar(50) null,
  "paused" boolean DEFAULT false not null,
  "allowlisted_addresses_only" boolean DEFAULT false not null,
  "eip_1559_enabled" boolean DEFAULT false not null,
  "deleted" boolean DEFAULT FALSE not null,
  "created_at" timestamp DEFAULT NOW() not null
);

CREATE TABLE "relayer_api_key" (
  "api_key" char(32) PRIMARY KEY not null,
  "relayer_id" uuid not null,
  "deleted" boolean DEFAULT FALSE not null,
  "created_at" timestamp DEFAULT NOW() not null
);

ALTER TABLE "relayer_api_key" ADD CONSTRAINT "fk_relayer_api_key__relayer_id" FOREIGN KEY ("relayer_id") REFERENCES "relayer" ("id");

CREATE TABLE "relayer_allowlisted_address" (
  "address" char(42) not null,
  "relayer_id" uuid not null,
  "created_at" timestamp DEFAULT NOW() not null,
   PRIMARY KEY ("address", "relayer_id")
);

ALTER TABLE "relayer_allowlisted_address" ADD CONSTRAINT "fk_relayer_allowlisted_address__relayer_id" FOREIGN KEY ("relayer_id") REFERENCES "relayer" ("id");

CREATE TABLE "relayer_transaction_status" (
   "status" varchar(50) PRIMARY KEY not null
);

INSERT INTO "relayer_transaction_status" ("status") VALUES ('PENDING'), ('INMEMPOOL'), ('MINED'), ('CONFIRMED'), ('FAILED'), ('EXPIRED');

CREATE TABLE "relayer_transaction" (
  "id" uuid PRIMARY KEY not null,
  "relayer_id" uuid not null,
  "api_key" char(32) not null,
  "to" char(42) not null,
  "from" char(42) not null,
  "nonce" varchar(1000) not null,
  "data" varchar(3000) null,
  "value" varchar(66) null,
  "chain_id" BIGINT not null,
  "gas_price" varchar(66) null,
  "sent_max_priority_fee_per_gas" varchar(100) null,
  "sent_max_fee_per_gas" varchar(100) null,
  "gas_limit" varchar(66) null,
  "block_hash" char(66) null,
  "block_number" int null,
  "hash" char(66) null,
  "speed" varchar(30) not null,
  "status" varchar(50) not null,
  "blobs" TEXT[] null, -- TODO! FIX TYPES
  "expires_at" timestamp not null,
  "expired_at" timestamp null,
  "queued_at" timestamp DEFAULT NOW() not null,
  "mined_at" timestamp null,
  "failed_at" timestamp null,
  "failed_reason" varchar(2000) null,
  "sent_at" timestamp null,
  "confirmed_at" timestamp null
);

ALTER TABLE "relayer_transaction" ADD CONSTRAINT "fk_relayer_transaction__relayer_id" FOREIGN KEY ("relayer_id") REFERENCES "relayer" ("id");
ALTER TABLE "relayer_transaction" ADD CONSTRAINT "fk_relayer_transaction__speed" FOREIGN KEY ("speed") REFERENCES "relayer_speed" ("name");
ALTER TABLE "relayer_transaction" ADD CONSTRAINT "fk_relayer_transaction__status" FOREIGN KEY ("status") REFERENCES "relayer_transaction_status" ("status");

CREATE or REPLACE VIEW relayer_transaction_view AS
	SELECT 
      "id",
     "relayer_id",
      "api_key",
      "to",
      "from",
      "value", 
      "data",
      "nonce",
      "chain_id",
      "status",
      "sent_max_priority_fee_per_gas",
      "sent_max_fee_per_gas",
      "gas_limit",
      "block_hash",
      "block_number",
      "hash",
      "speed",
      "expires_at",
      "expired_at",
      "queued_at",
      "mined_at",
      "failed_at", 
      "sent_at",
      "confirmed_at"
  FROM "relayer_transaction";

CREATE TABLE "relayer_transaction_audit_log" (
  "history_id" SERIAL PRIMARY KEY not null,
  "id" uuid not null,
  "relayer_id" uuid not null,
  "api_key" char(32) not null,
  "to" char(42) not null,
  "from" char(42) not null,
  "nonce" varchar(1000) not null,
  "data" varchar(3000) null,
  "value" varchar(66) null,
  "chain_id" BIGINT not null,
  "gas_price" varchar(66) null,
  "sent_max_priority_fee_per_gas" varchar(100) null,
  "sent_max_fee_per_gas" varchar(100) null,
  "gas_limit" varchar(66) null,
  "block_hash" char(66) null,
  "block_number" int null,
  "hash" char(66) null,
  "speed" varchar(30) not null,
  "status" varchar(50) not null,
  "expires_at" timestamp not null,
  "expired_at" timestamp null,
  "queued_at" timestamp DEFAULT NOW() not null,
  "mined_at" timestamp null,
  "failed_at" timestamp null,
  "failed_reason" varchar(2000) null,
  "sent_at" timestamp null,
  "confirmed_at" timestamp null
);

ALTER TABLE "relayer_transaction_audit_log" ADD CONSTRAINT "fk_relayer_transaction_audit_log__relayer_id" FOREIGN KEY ("relayer_id") REFERENCES "relayer" ("id");
ALTER TABLE "relayer_transaction_audit_log" ADD CONSTRAINT "fk_relayer_transaction_audit_log__speed" FOREIGN KEY ("speed") REFERENCES "relayer_speed" ("name");
ALTER TABLE "relayer_transaction_audit_log" ADD CONSTRAINT "fk_relayer_transaction_audit_log__status" FOREIGN KEY ("status") REFERENCES "relayer_transaction_status" ("status");
