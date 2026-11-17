# Documentation

This is the documentation for rrelayer, powered by [Vocs](https://vocs.dev).

## Installing

```bash
npm i
```

## Running

```bash
npm run dev
```

## Deployment

The documentation deploys to GitHub Pages from `.github/workflows/pages.yml`.
The workflow builds the Vocs site with Node.js 20 and publishes
`documentation/rrelayer/docs/dist`.

### GitHub Pages setup

1. Open the repository on GitHub.
2. Go to Settings > Pages.
3. Under Build and deployment, set Source to GitHub Actions.
4. Under Custom domain, set `rrelayer.xyz`.
5. Enable Enforce HTTPS after GitHub finishes provisioning the certificate.

### DNS records

For the apex domain, add these `A` records:

```text
@  A  185.199.108.153
@  A  185.199.109.153
@  A  185.199.110.153
@  A  185.199.111.153
```

Optionally add these `AAAA` records for IPv6:

```text
@  AAAA  2606:50c0:8000::153
@  AAAA  2606:50c0:8001::153
@  AAAA  2606:50c0:8002::153
@  AAAA  2606:50c0:8003::153
```

For `www`, add:

```text
www  CNAME  joshstevens19.github.io
```

If this repository should serve `rindexer.xyz` instead, change
`documentation/rrelayer/docs/public/CNAME` and the GitHub Pages custom domain to
`rindexer.xyz`, then use the same DNS records with `rindexer.xyz` as the zone.

After DNS is live and GitHub Pages serves the custom domain, remove the Vercel
project and delete any old Vercel DNS records, usually `A 76.76.21.21` and
`CNAME cname.vercel-dns.com`.

If you no longer have access to the Vercel project, remove Vercel by changing
the domain at the registrar/DNS provider instead:

1. If the domain uses Vercel nameservers, change the nameservers to your DNS
   provider or registrar defaults.
2. Add the GitHub Pages DNS records above in the active DNS zone.
3. Wait for DNS propagation, then verify the domain in GitHub Pages settings.

Vercel cannot keep serving the domain once the authoritative DNS no longer
points at Vercel.

### GoDaddy from Vercel nameservers

If GoDaddy shows `ns1.vercel-dns.com` and `ns2.vercel-dns.com` under
Nameservers, click Change Nameservers and switch back to GoDaddy/default
nameservers. Once GoDaddy is authoritative again, open DNS Records and add the
GitHub Pages records above.

Before switching, check whether the domain had any email or verification records
in Vercel. Public DNS currently shows no MX or TXT records for `rrelayer.xyz`,
but any private or missing records must be recreated in GoDaddy before relying
on the domain for email or third-party verification.
