import { defineConfig } from 'vocs';
import pkg from './package.json';
import { sidebar } from './sidebar';

export default defineConfig({
  title: 'rrelayerr',
  sidebar,
  description:
    'Build reliable Ethereum apps & libraries with lightweight, composable, & type-safe modules from viem.',
  topNav: [
    { text: 'Docs', link: '/docs/what-is-rrelayerr', match: '/docs' },
    {
      text: pkg.version,
      items: [
        {
          text: 'Changelog',
          link: 'https://github.com/joshstevens19/rrelayerr/blob/master/CHANGELOG.md',
        },
        {
          text: 'Contributing',
          link: 'https://github.com/joshstevens19/rrelayerr/blob/master/.github/CONTRIBUTING.md',
        },
      ],
    },
  ],
});
