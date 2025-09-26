import { defineConfig } from 'vocs';

export default defineConfig({
  title: '🦀 rrelayer 🦀',
  head: (
    <>
      <meta property="og:type" content="website" />
      <meta
        property="og:title"
        content="rrelayer · A lighting-fast multi chain indexing solution written in Rust"
      />
      <meta property="og:image" content="https://rrelayer.xyz/favicon.png" />
      <meta property="og:url" content="https://rrelayer.xyz" />
      <meta
        property="og:description"
        content="Build scalable, efficient, and secure blockchain indexing solutions for modern decentralized applications."
      />
    </>
  ),

  iconUrl: '/favicon.png',
  ogImageUrl: '/favicon.png',
  description:
    'rrelayer is a lighting-fast multi chain indexing solution written in Rust',

  topNav: [
    { text: 'Docs', link: '/getting-started/installation', match: '/docs' },
    { text: 'Changelog', link: '/changelog', match: '/docs' },
  ],
  socials: [
    {
      icon: 'github',
      link: 'https://github.com/joshstevens19/rrelayer',
    },
  ],
  sidebar: [
    {
      text: 'Introduction',
      items: [
        { text: 'What is rrelayer?', link: '/introduction/what-is-rrelayer' },
        { text: 'Why rrelayer?', link: '/introduction/why-rrelayer' },
      ],
    },
    {
      text: 'Getting started',
      items: [
        { text: 'Installation', link: '/getting-started/installation' },
        { text: 'Create Project', link: '/getting-started/create-new-project' },
        { text: 'CLI', link: '/getting-started/cli' },
      ],
    },
    {
      text: 'Config',
      link: '/config',
      items: [
        { text: 'Api Config', link: '/config/api-config' },
        {
          text: 'Signing Providers',
          items: [
            { text: 'AWS KMS', link: '/config/signing-providers/aws-kms' },
            {
              text: 'AWS Secret Manager',
              link: '/config/signing-providers/aws-secret-manager',
            },
            {
              text: 'GCP Secret Manager',
              link: '/config/signing-providers/gcp-secret-manager',
            },
            {
              text: 'Raw Mnemonic',
              link: '/config/signing-providers/raw-mnemonic',
            },
            { text: 'Privy', link: '/config/signing-providers/privy' },
            { text: 'Turnkey', link: '/config/signing-providers/turnkey' },
          ],
        },
        {
          text: 'Networks',
          items: [
            { text: 'Config', link: '/config/networks/config' },
            {
              text: 'Automatic Top Up',
              link: '/config/networks/automatic-top-up',
              items: [
                {
                  text: 'Via Safe',
                  link: '/config/networks/automatic-top-up#safe',
                },
                {
                  text: 'Native',
                  link: '/config/networks/automatic-top-up#native---optional-you-should-have-this-or-erc20_tokens',
                },
                {
                  text: 'ERC20',
                  link: '/config/networks/automatic-top-up#erc20-tokens---optional-you-should-have-this-or-native',
                },
              ],
            },
            {
              text: 'Permissions',
              link: '/config/networks/permissions',
              items: [
                {
                  text: 'Allowlist',
                  link: '/config/networks/permissions#allowlist---optional',
                },
                {
                  text: 'Disable native transfer',
                  link: '/config/networks/permissions#disable_native_transfer---optional-default-false',
                },
                {
                  text: 'Disable personal sign',
                  link: '/config/networks/permissions#disable_personal_sign---optional-default-false',
                },
                {
                  text: 'Disable typed data sign',
                  link: '/config/networks/permissions#disable_typed_data_sign---optional-default-false',
                },
                {
                  text: 'Disable transactions',
                  link: '/config/networks/permissions#disable_transactions---optional-default-false',
                },
              ],
            },
            { text: 'API Keys', link: '/config/networks/api-keys' },
            {
              text: 'Gas Provider',
              link: '/config/networks/gas-provider',
              items: [
                { text: 'Infura', link: '/config/networks/gas-provider#infura' },
                { text: 'Tenderly', link: '/config/networks/gas-provider#tenderly' },
                { text: 'Custom', link: '/config/networks/gas-provider#custom' },
                { text: 'Fallback', link: '/config/networks/gas-provider#fallback' },
              ],
            },
          ],
        },
        { text: 'Webhooks', link: '/config/webhooks' },
        { text: 'Rate limits', link: '/config/rate-limits' },
      ],
    },
    {
      text: 'Integration',
      items: [
        {
          text: 'Typescript',
          items: [
            {
              text: 'Getting started',
              link: '',
            },
            {
              text: 'Viem',
              link: '',
            },
            {
              text: 'Ethers',
              link: '',
            },
            {
              text: 'Direct SDK',
              link: '',
              items: [
                {
                  text: 'Authentication',
                  link: '',
                },
                {
                  text: 'Relayers',
                  link: '',
                },
                {
                  text: 'Networks',
                  link: '',
                },
                {
                  text: 'Transactions',
                  link: '',
                },
                {
                  text: 'Sign',
                  link: '',
                },
                {
                  text: 'Allowlist',
                  link: '',
                },
              ],
            },
          ],
        },
        {
          text: 'API',
          link: '',
          items: [
            {
              text: 'Authentication',
              link: '',
            },
            {
              text: 'Relayers',
              link: '',
            },
            {
              text: 'Networks',
              link: '',
            },
            {
              text: 'Transactions',
              link: '',
            },
            {
              text: 'Sign',
              link: '',
            },
            {
              text: 'Allowlist',
              link: '',
            },
          ],
        },
      ],
    },
    {
      text: 'Deploying',
      items: [
        { text: 'Railway - coming soon', link: '' },
        { text: 'AWS - coming soon', link: '' },
        { text: 'GCP - coming soon', link: '' },
      ],
    },
    { text: 'Changelog', link: '/docs/changelog' },
  ],
});
