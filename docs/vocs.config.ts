import { defineConfig } from 'vocs';
import pkg from './package.json';
import { sidebar } from './sidebar';

export default defineConfig({
  title: 'rrelayer',
  sidebar,
  description:
    'Build reliable Ethereum apps & libraries with lightweight, composable, & type-safe modules from viem.',
  topNav: [
    { text: 'Docs', link: '/docs/what-is-rrelayer', match: '/docs' },
    {
      text: pkg.version,
      items: [
        {
          text: 'Changelog',
          link: 'https://github.com/joshstevens19/rrelayer/blob/master/CHANGELOG.md',
        },
        {
          text: 'Contributing',
          link: 'https://github.com/joshstevens19/rrelayer/blob/master/.github/CONTRIBUTING.md',
        },
      ],
    },
  ],
});
