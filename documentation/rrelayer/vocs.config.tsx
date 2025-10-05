import { defineConfig } from 'vocs';

export default defineConfig({
  title: '🦀 rrelayer 🦀',
  search: {},
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
        content="A lighting-fast multi chain indexing solution written in Rust"
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
      text: 'Migration',
      items: [
        {
          text: 'Defender > rrelayer',
          link: '/migration/defender-to-rrelayer',
        },
      ],
    },
    {
      text: 'Config',
      link: '/config',
      items: [
        { text: 'Api Config', link: '/config/api-config' },
        {
          text: 'Signing Providers',
          collapsed: false,
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
            {
              text: 'Fireblocks',
              link: '/config/signing-providers/fireblocks',
            },
            {
              text: 'Private Keys',
              link: '/config/signing-providers/private-keys',
            },
            {
              text: 'PKCS#11 - BETA',
              link: '/config/signing-providers/pkcs11',
            },
          ],
        },
        {
          text: 'Networks',
          collapsed: false,
          items: [
            { text: 'Config', link: '/config/networks/config' },
            {
              text: 'Transaction Speed',
              link: '/config/networks/transaction-speed',
            },
            {
              text: 'Automatic Top Up',
              collapsed: false,
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
              collapsed: false,
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
              collapsed: false,
              link: '/config/networks/gas-provider',
              items: [
                {
                  text: 'Fallback',
                  link: '/config/networks/gas-provider#fallback',
                },
                {
                  text: 'Block Native',
                  link: '/config/networks/gas-provider#block-native',
                },
                {
                  text: 'Infura',
                  link: '/config/networks/gas-provider#infura',
                },
                {
                  text: 'Tenderly',
                  link: '/config/networks/gas-provider#tenderly',
                },
                {
                  text: 'Etherscan',
                  link: '/config/networks/gas-provider#etherscan',
                },
                {
                  text: 'Custom',
                  link: '/config/networks/gas-provider#custom',
                },
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
          text: 'SDK',
          items: [
            {
              text: 'Installation',
              collapsed: false,
              items: [
                {
                  text: 'Node',
                  link: '/integration/sdk/installation/node',
                },
                {
                  text: 'Rust',
                  link: '/integration/sdk/installation/rust',
                },
              ],
            },
            {
              text: 'Framework Guides',
              collapsed: false,
              items: [
                {
                  text: 'Typescript',
                  items: [
                    {
                      text: 'Viem',
                      link: '/integration/sdk/framework-guides/typescript/viem',
                    },
                    {
                      text: 'Ethers',
                      link: '/integration/sdk/framework-guides/typescript/ethers',
                    },
                  ],
                },
                {
                  text: 'Rust',
                  items: [
                    {
                      text: 'Alloy',
                      link: '/integration/sdk/framework-guides/rust/alloy',
                    },
                  ],
                },
              ],
            },
            {
              text: 'Create Client / Authentication',
              collapsed: true,
              items: [
                {
                  text: 'Node',
                  collapsed: true,
                  link: '/integration/sdk/create-client-authentication/node',
                  items: [
                    {
                      text: 'Basic Auth',
                      link: '/integration/sdk/create-client-authentication/node#basic-auth',
                    },
                    {
                      text: 'API Keys',
                      link: '/integration/sdk/create-client-authentication/node#api-key-auth',
                    },
                    {
                      text: 'Auth Status Check',
                      link: '/integration/sdk/create-client-authentication/node#auth-status-check',
                    },
                    {
                      text: 'Rate Limiting',
                      link: '/integration/sdk/create-client-authentication/node#rate-limiting',
                    },
                  ],
                },
                {
                  text: 'Rust',
                  collapsed: true,
                  link: '/integration/sdk/create-client-authentication/rust',
                  items: [
                    {
                      text: 'Basic Auth',
                      link: '/integration/sdk/create-client-authentication/rust#basic-auth',
                    },
                    {
                      text: 'API Keys',
                      link: '/integration/sdk/create-client-authentication/rust#api-key-auth',
                    },
                    {
                      text: 'Auth Status Check',
                      link: '/integration/sdk/create-client-authentication/rust#auth-status-check',
                    },
                    {
                      text: 'Rate Limiting',
                      link: '/integration/sdk/create-client-authentication/rust#rate-limiting',
                    },
                  ],
                },
              ],
            },
            {
              text: 'Relayers',
              collapsed: true,
              items: [
                {
                  text: 'Node',
                  collapsed: true,
                  link: '/integration/sdk/relayers/node',
                  items: [
                    {
                      text: 'Create Relayer',
                      link: '/integration/sdk/relayers/node#create-relayer',
                    },
                    {
                      text: 'Get Relayers',
                      link: '/integration/sdk/relayers/node#get-relayers',
                    },
                    {
                      text: 'Get Relayer',
                      link: '/integration/sdk/relayers/node#get-relayer',
                    },
                    {
                      text: 'Clone Relayer',
                      link: '/integration/sdk/relayers/node#clone-relayer',
                    },
                    {
                      text: 'Pause/Unpause',
                      link: '/integration/sdk/relayers/node#pauseunpause-relayer',
                    },
                    {
                      text: 'Update EIP-1559 Status',
                      link: '/integration/sdk/relayers/node#update-eip-1559-status',
                    },
                    {
                      text: 'Update Max Gas Price',
                      link: '/integration/sdk/relayers/node#update-max-gas-price',
                    },
                    {
                      text: 'Remove Max Gas Price',
                      link: '/integration/sdk/relayers/node#remove-max-gas-price',
                    },
                    {
                      text: 'Delete Relayer',
                      link: '/integration/sdk/relayers/node#delete-relayer',
                    },
                    {
                      text: 'Get Allowlist',
                      link: '/integration/sdk/relayers/node#get-allowlist',
                    },
                  ],
                },
                {
                  text: 'Rust',
                  collapsed: true,
                  link: '/integration/sdk/relayers/rust',
                  items: [
                    {
                      text: 'Create Relayer',
                      link: '/integration/sdk/relayers/rust#create-relayer',
                    },
                    {
                      text: 'Get Relayers',
                      link: '/integration/sdk/relayers/rust#get-relayers',
                    },
                    {
                      text: 'Get Relayer',
                      link: '/integration/sdk/relayers/rust#get-relayer',
                    },
                    {
                      text: 'Clone Relayer',
                      link: '/integration/sdk/relayers/rust#clone-relayer',
                    },
                    {
                      text: 'Pause/Unpause',
                      link: '/integration/sdk/relayers/rust#pauseunpause-relayer',
                    },
                    {
                      text: 'Update EIP-1559 Status',
                      link: '/integration/sdk/relayers/rust#update-eip-1559-status',
                    },
                    {
                      text: 'Update Max Gas Price',
                      link: '/integration/sdk/relayers/rust#update-max-gas-price',
                    },
                    {
                      text: 'Remove Max Gas Price',
                      link: '/integration/sdk/relayers/rust#remove-max-gas-price',
                    },
                    {
                      text: 'Delete Relayer',
                      link: '/integration/sdk/relayers/rust#delete-relayer',
                    },
                    {
                      text: 'Get Allowlist',
                      link: '/integration/sdk/relayers/rust#get-allowlist',
                    },
                  ],
                },
              ],
            },
            {
              text: 'Networks',
              collapsed: true,
              items: [
                {
                  text: 'Node',
                  collapsed: true,
                  link: '/integration/sdk/networks/node',
                  items: [
                    {
                      text: 'Get Network',
                      link: '/integration/sdk/networks/node#get-network',
                    },
                    {
                      text: 'Get Networks',
                      link: '/integration/sdk/networks/node#get-networks',
                    },
                    {
                      text: 'Gas Prices',
                      link: '/integration/sdk/networks/node#gas-prices',
                    },
                  ],
                },
                {
                  text: 'Rust',
                  collapsed: true,
                  link: '/integration/sdk/networks/rust',
                  items: [
                    {
                      text: 'Get Network',
                      link: '/integration/sdk/networks/rust#get-network',
                    },
                    {
                      text: 'Get Networks',
                      link: '/integration/sdk/networks/rust#get-networks',
                    },
                    {
                      text: 'Gas Prices',
                      link: '/integration/sdk/networks/rust#gas-prices',
                    },
                  ],
                },
              ],
            },
            {
              text: 'Transactions',
              collapsed: true,
              items: [
                {
                  text: 'Node',
                  collapsed: true,
                  link: '/integration/sdk/transactions/node',
                  items: [
                    {
                      text: 'Send Transaction',
                      link: '/integration/sdk/transactions/node#send-transaction',
                    },
                    {
                      text: 'Get Transaction',
                      link: '/integration/sdk/transactions/node#get-transaction',
                    },
                    {
                      text: 'Transaction Status',
                      link: '/integration/sdk/transactions/node#transaction-status',
                    },
                    {
                      text: 'Replace Transaction',
                      link: '/integration/sdk/transactions/node#replace-transaction',
                    },
                    {
                      text: 'Cancel Transaction',
                      link: '/integration/sdk/transactions/node#cancel-transaction',
                    },
                    {
                      text: 'Transaction Counts',
                      link: '/integration/sdk/transactions/node#transactions-counts',
                    },
                  ],
                },
                {
                  text: 'Rust',
                  collapsed: true,
                  link: '/integration/sdk/transactions/rust',
                  items: [
                    {
                      text: 'Send Transaction',
                      link: '/integration/sdk/transactions/rust#send-transaction',
                    },
                    {
                      text: 'Get Transaction',
                      link: '/integration/sdk/transactions/rust#get-transaction',
                    },
                    {
                      text: 'Transaction Status',
                      link: '/integration/sdk/transactions/rust#transaction-status',
                    },
                    {
                      text: 'Replace Transaction',
                      link: '/integration/sdk/transactions/rust#replace-transaction',
                    },
                    {
                      text: 'Cancel Transaction',
                      link: '/integration/sdk/transactions/rust#cancel-transaction',
                    },
                    {
                      text: 'Transaction Counts',
                      link: '/integration/sdk/transactions/rust#transactions-counts',
                    },
                  ],
                },
              ],
            },
            {
              text: 'Sign',
              collapsed: true,
              items: [
                {
                  text: 'Node',
                  collapsed: true,
                  link: '/integration/sdk/sign/node',
                  items: [
                    {
                      text: 'Sign Text Message',
                      link: '/integration/sdk/sign/node#sign-text-message',
                    },
                    {
                      text: 'Signed Text History',
                      link: '/integration/sdk/sign/node#signed-text-history',
                    },
                    {
                      text: 'Sign Typed Data',
                      link: '/integration/sdk/sign/node#sign-typed-data',
                    },
                    {
                      text: 'Signed Typed Data History',
                      link: '/integration/sdk/sign/node#signed-typed-data-history',
                    },
                  ],
                },
                {
                  text: 'Rust',
                  collapsed: true,
                  link: '/integration/sdk/sign/rust',
                  items: [
                    {
                      text: 'Sign Text Message',
                      link: '/integration/sdk/sign/rust#sign-text-message',
                    },
                    {
                      text: 'Signed Text History',
                      link: '/integration/sdk/sign/rust#signed-text-history',
                    },
                    {
                      text: 'Sign Typed Data',
                      link: '/integration/sdk/sign/rust#sign-typed-data',
                    },
                    {
                      text: 'Signed Typed Data History',
                      link: '/integration/sdk/sign/rust#signed-typed-data-history',
                    },
                  ],
                },
              ],
            },
          ],
        },
        {
          text: 'API',
          items: [
            {
              text: 'Authentication',
              link: '/integration/api/authentication',
              collapsed: true,
              items: [
                {
                  text: 'Basic Auth',
                  link: '/integration/api/authentication#basic-authentication',
                },
                {
                  text: 'API Keys',
                  link: '/integration/api/authentication#api-key-authentication',
                },
                {
                  text: 'Auth Status Check',
                  link: '/integration/api/authentication#status-check',
                },
                {
                  text: 'Rate Limiting',
                  link: '/integration/api/authentication#rate-limiting-headers',
                },
              ],
            },
            {
              text: 'Relayers',
              link: '/integration/api/relayers',
              collapsed: true,
              items: [
                {
                  text: 'Create Relayer',
                  link: '/integration/api/relayers#create-relayer',
                },
                {
                  text: 'Get Relayers',
                  link: '/integration/api/relayers#get-relayers',
                },
                {
                  text: 'Get Relayer',
                  link: '/integration/api/relayers#get-relayer',
                },
                {
                  text: 'Clone Relayer',
                  link: '/integration/api/relayers#clone-relayer',
                },
                {
                  text: 'Pause/Unpause',
                  link: '/integration/api/relayers#pause-relayer',
                },
                {
                  text: 'Gas Settings',
                  link: '/integration/api/relayers#update-max-gas-price',
                },
                {
                  text: 'Delete Relayer',
                  link: '/integration/api/relayers#delete-relayer',
                },
                {
                  text: 'Get Allowlist',
                  link: '/integration/api/allowlist#get-allowlist-addresses',
                },
              ],
            },
            {
              text: 'Networks',
              link: '/integration/api/networks',
              collapsed: true,
              items: [
                {
                  text: 'Get Network',
                  link: '/integration/api/networks#get-networks',
                },
                {
                  text: 'Get Networks',
                  link: '/integration/api/networks#get-networks',
                },
                {
                  text: 'Gas Prices',
                  link: '/integration/api/networks#get-gas-price',
                },
                {
                  text: 'Gas Providers',
                  link: '/integration/api/networks#gas-price-providers',
                },
              ],
            },
            {
              text: 'Transactions',
              link: '/integration/api/transactions',
              collapsed: true,
              items: [
                {
                  text: 'Send Transaction',
                  link: '/integration/api/transactions#send-transaction',
                },
                {
                  text: 'Get Transaction',
                  link: '/integration/api/transactions#get-transaction',
                },
                {
                  text: 'Transaction Status',
                  link: '/integration/api/transactions#get-transaction-status',
                },
                {
                  text: 'Replace Transaction',
                  link: '/integration/api/transactions#replace-transaction',
                },
                {
                  text: 'Cancel Transaction',
                  link: '/integration/api/transactions#cancel-transaction',
                },
                {
                  text: 'Transaction Counts',
                  link: '/integration/api/transactions#get-transaction-counts',
                },
              ],
            },
            {
              text: 'Sign',
              link: '/integration/api/sign',
              collapsed: true,
              items: [
                {
                  text: 'Sign Text Message',
                  link: '/integration/api/sign#sign-text-message',
                },
                {
                  text: 'Signed Text History',
                  link: '/integration/api/sign#get-text-signing-history',
                },
                {
                  text: 'Sign Typed Data',
                  link: '/integration/api/sign#sign-typed-data-eip-712',
                },
                {
                  text: 'Signed Typed Data History',
                  link: '/integration/api/sign#get-typed-data-signing-history',
                },
                {
                  text: 'EIP-712 Types',
                  link: '/integration/api/sign#eip-712-domain-types',
                },
              ],
            },
          ],
        },
        {
          text: 'Gasless Transactions',
          collapsed: false,
          items: [
            {
              text: 'Introduction',
              link: '/integration/gasless-transactions#gasless-transactions',
            },
            {
              text: 'EIP-712',
              link: '/integration/gasless-transactions#eip-712',
            },
            {
              text: 'ERC-2771',
              link: '/integration/gasless-transactions#erc-2771',
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
    {
      text: 'Coming soon..',
      items: [
        {
          text: 'Triggers - Send transactions based on events emitted',
          link: '',
        },
        {
          text: 'Cron - Send transactions every n number of hours/days/weeks',
          link: '',
        },
        { text: 'Safe multisig support', link: '' },
      ],
    },
    { text: 'Changelog', link: '/changelog' },
    { text: 'Shoutout', link: '/shoutout' },
  ],
});
