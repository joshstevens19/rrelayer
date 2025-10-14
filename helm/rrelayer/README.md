# rrelayer Helm Chart

## Description

A Helm chart for deploying the `rrelayer` service. The chart mounts your `rrelayer.yaml` configuration from a ConfigMap and injects runtime secrets through environment variables or existing Kubernetes secrets.

## Prerequisites

- Kubernetes 1.25+
- Helm 3.0+
- A PostgreSQL instance accessible via `DATABASE_URL`

## Installing the Chart

To install the chart with the release name `my-relayer` from the chart directory:

```bash
helm install my-relayer ./rrelayer
```

Use the `--values` flag to provide custom configuration:

```bash
helm install my-relayer ./rrelayer -f values.yaml
```

## Uninstalling the Chart

To uninstall/delete the `my-relayer` release:

```bash
helm uninstall my-relayer
```

This removes all Kubernetes components associated with the release.

## Configuration

Key configuration options are summarised below. See `values.yaml` for the full set.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of API replicas | `1` |
| `image.repository` | Container image repository | `ghcr.io/joshstevens19/rrelayer` |
| `image.tag` | Image tag | `latest` |
| `service.port` | Service and container port | `3000` |
| `projectPath` | Mount path for `rrelayer.yaml` | `/app/project` |
| `rrelayerConfig` | Raw YAML configuration mounted into the pod | Example config |
| `env` | Inline environment variable values (used when `externalSecret` is empty) | empty strings |
| `externalSecret` | Name of an existing secret containing required keys | `""` |
| `extraEnv` | Additional environment variable objects appended to the pod | `[]` |
| `securityContext` | Pod security context options | see values |

### Required Environment Keys

rrelayer expects the following environment variables to be provided via `env` or `externalSecret`:

- `DATABASE_URL`
- `RAW_DANGEROUS_MNEMONIC` (or replace with a secure signing provider secret)
- `RRELAYER_AUTH_USERNAME`
- `RRELAYER_AUTH_PASSWORD`
- `POSTGRES_PASSWORD` (required when connecting to secured Postgres instances)

Example secret creation:

```bash
kubectl create secret generic rrelayer-secrets \
  --from-literal=DATABASE_URL="postgresql://user:password@hostname:5432/postgres" \
  --from-literal=RAW_DANGEROUS_MNEMONIC="seed phrase goes here" \
  --from-literal=RRELAYER_AUTH_USERNAME="admin" \
  --from-literal=RRELAYER_AUTH_PASSWORD="change-me" \
  --from-literal=POSTGRES_PASSWORD="change-me"
```

Then update `values.yaml`:

```yaml
externalSecret: rrelayer-secrets
env:
  RAW_DANGEROUS_MNEMONIC: ""
  DATABASE_URL: ""
  POSTGRES_PASSWORD: ""
  RRELAYER_AUTH_USERNAME: ""
  RRELAYER_AUTH_PASSWORD: ""
```

### Custom Configuration

Override `rrelayerConfig` to match your project needs. The value is rendered verbatim into `rrelayer.yaml` and supports environment interpolation using `${VARIABLE}` syntax.

```yaml
rrelayerConfig: |
  name: production
  signing_provider:
    aws_kms:
      key_id: ${AWS_KMS_KEY_ID}
  networks:
    - name: base_ethereum
      chain_id: 8453
      provider_urls:
        - ${BASE_RPC_URL}
  api_config:
    host: "0.0.0.0"
    port: 3000
    authentication_username: ${RRELAYER_AUTH_USERNAME}
    authentication_password: ${RRELAYER_AUTH_PASSWORD}
```

Use `extraEnv` to supply any additional environment variables needed by your configuration:

```yaml
extraEnv:
  - name: AWS_REGION
    value: us-east-1
  - name: AWS_KMS_KEY_ID
    valueFrom:
      secretKeyRef:
        name: kms-secrets
        key: key-id
```

## Ingress Examples

Enable ingress by toggling `ingress.enabled` and setting controller-specific annotations:

```yaml
ingress:
  enabled: true
  annotations:
    kubernetes.io/ingress.class: "nginx"
  hosts:
    - host: rrelayer.example.com
      paths:
        - /
```

## Upgrading

Use `helm upgrade` with a modified values file:

```bash
helm upgrade my-relayer ./rrelayer -f values-production.yaml
```

## Notes

- Ensure the referenced `DATABASE_URL` is reachable from the cluster.
- Consider replacing `RAW_DANGEROUS_MNEMONIC` with a hardware or key-management signing provider for production deployments.
- Rotate authentication credentials regularly and store them in a secrets manager.
