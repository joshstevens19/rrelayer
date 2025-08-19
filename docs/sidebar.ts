import type { Sidebar } from 'vocs';

export const sidebar = {
  '/docs/': [
    {
      text: 'Introduction',
      items: [
        { text: 'What is rrelayer?', link: '/docs/what-is-rrelayer' },
        { text: 'Why rrelayer?', link: '/docs/why-rrelayer' },
      ],
    },
    {
      text: 'Gasless transactions',
      items: [
        {
          text: 'Introduction',
          link: '/docs/gasless-transactions/introduction',
        },
        {
          text: 'EIP-2771',
          link: '/docs/gasless-transactions/eip-2771',
        },
        {
          text: 'Custom',
          link: '/docs/gasless-transactions/custom',
        },
      ],
    },
    {
      text: 'Server',
      items: [
        {
          text: 'Introduction',
          link: '/docs/server/introduction',
        },
        {
          text: 'Setup',
          link: '/docs/server/setup',
        },
        {
          text: 'API',
          items: [
            {
              text: 'Introduction',
              link: '/docs/server/api/introduction',
            },
            {
              text: 'Network',
              collapsed: true,
              items: [
                {
                  text: 'Get all networks',
                  link: '/docs/server/api/network/get-networks',
                },
                {
                  text: 'Enabled networks',
                  link: '/docs/server/api/network/enabled-networks',
                },
                {
                  text: 'Enable network',
                  link: '/docs/server/api/network/enable-network',
                },
                {
                  text: 'Disabled networks',
                  link: '/docs/server/api/network/disabled-networks',
                },
                {
                  text: 'Disable network',
                  link: '/docs/server/api/network/disable-network',
                },
              ],
            },
            {
              text: 'Relayer',
              collapsed: true,
              items: [
                {
                  text: 'Create relayer',
                  link: '',
                },
                {
                  text: 'Get relayers',
                  link: '',
                },
                {
                  text: 'Get relayer',
                  link: '',
                },
                {
                  text: 'Delete relayer',
                  link: '',
                },
                {
                  text: 'Pause relayer',
                  link: '',
                },
                {
                  text: 'Unpause relayer',
                  link: '',
                },
                {
                  text: 'Update max gas price',
                  link: '',
                },
                {
                  text: 'Update EIP-1559 status',
                  link: '',
                },
                {
                  text: 'Create relayer API key',
                  link: '',
                },
                {
                  text: 'Get relayer API keys',
                  link: '',
                },
                {
                  text: 'Delete relayer API key',
                  link: '',
                },
                {
                  text: 'Get relayer allowlisted',
                  link: '',
                },
                {
                  text: 'Add relayer allowlist',
                  link: '',
                },
                {
                  text: 'Delete relayer allowlist',
                  link: '',
                },
                {
                  text: 'Sign text',
                  link: '',
                },
                {
                  text: 'Sign typed data',
                  link: '',
                },
              ],
            },
            {
              text: 'Transactions',
              collapsed: true,
              items: [
                {
                  text: 'Get transaction',
                  link: '',
                },
                {
                  text: 'Get transaction status',
                  link: '',
                },
                {
                  text: 'Send transaction',
                  link: '',
                },
                {
                  text: 'Replace transaction',
                  link: '',
                },
                {
                  text: 'Cancel transaction',
                  link: '',
                },
                {
                  text: 'Get transactions',
                  link: '',
                },
                {
                  text: 'Get transaction pending count',
                  link: '',
                },
                {
                  text: 'Get transactions inmempool count',
                  link: '',
                },
              ],
            },
            {
              text: 'Gas',
              collapsed: true,
              items: [
                {
                  text: 'Get gas price',
                  link: '',
                },
              ],
            },
          ],
        },
        {
          text: 'Extending the server',
          items: [
            {
              text: 'Custom networks',
              link: '/docs/server/api/extending-server/custom-networks',
            },
            {
              text: 'Custom gas estimators',
              link: '/docs/server/api/extending-server/custom-gas-estimators',
            },
          ],
        },
        {
          text: 'Deploying',
          items: [
            {
              text: 'Railway',
              link: '',
            },
            {
              text: 'AWS',
              link: '',
            },
            {
              text: 'GCP',
              link: '',
            },
          ],
        },
      ],
    },
    {
      text: 'Dashboard',
      items: [
        {
          text: 'Introduction',
          link: '/docs/dashboard/introduction',
        },
        {
          text: 'Setup',
          link: '/docs/dashboard/setup',
        },
        {
          text: 'Deploying',
          items: [
            {
              text: 'Railway',
              link: '',
            },
            {
              text: 'AWS',
              link: '',
            },
            {
              text: 'GCP',
              link: '',
            },
          ],
        },
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
              link: '/docs/integration/typescript/getting-started',
            },
            {
              text: 'Viem',
              link: '/docs/integration/typescript/viem',
            },
            {
              text: 'Ethers',
              link: '/docs/integration/typescript/ethers',
            },
            {
              text: 'SDK',
              items: [
                {
                  text: 'RRelayerrClient',
                  items: [
                    {
                      text: 'Setup',
                      link: '/docs/integration/typescript/sdk/rrelayer-client/setup',
                    },
                    {
                      text: 'Gas',
                      link: '/docs/integration/typescript/sdk/rrelayer-client/gas',
                    },
                    {
                      text: 'Network',
                      link: '/docs/integration/typescript/sdk/rrelayer-client/network',
                    },
                    {
                      text: 'Relayer',
                      link: '/docs/integration/typescript/sdk/rrelayer-client/relayer',
                    },
                    {
                      text: 'Relayer client',
                      link: '/docs/integration/typescript/sdk/rrelayer-client/relayer-client',
                    },
                  ],
                },
                {
                  text: 'RRelayerrRelayerClient',
                  items: [
                    {
                      text: 'Setup',
                      link: '/docs/integration/typescript/sdk/rrelayer-relayer-client/setup',
                    },
                    {
                      text: 'Base methods',
                      link: '/docs/integration/typescript/sdk/rrelayer-relayer-client/base',
                    },
                    {
                      text: 'API keys',
                      link: '/docs/integration/typescript/sdk/rrelayer-relayer-client/api-keys',
                    },
                    {
                      text: 'Sign',
                      link: '/docs/integration/typescript/sdk/rrelayer-relayer-client/sign',
                    },
                    {
                      text: 'Transactions',
                      link: '/docs/integration/typescript/sdk/rrelayer-relayer-client/transactions',
                    },
                  ],
                },
              ],
            },
          ],
        },
        {
          text: 'HTTP requests',
          link: '/docs/integration/http-requests',
        },
        {
          text: 'Go',
          items: [
            {
              text: 'Getting started',
              link: '/docs/integration/go/getting-started',
            },
            {
              text: 'Coming soon...',
            },
          ],
        },
        {
          text: 'Rust',
          items: [
            {
              text: 'Getting started',
              link: '/docs/integration/rust/getting-started',
            },
            {
              text: 'Coming soon...',
            },
          ],
        },
        {
          text: 'Python',
          items: [
            {
              text: 'Getting started',
              link: '/docs/integration/python/getting-started',
            },
            {
              text: 'Coming soon...',
            },
          ],
        },
      ],
    },
  ],
} as const satisfies Sidebar;
