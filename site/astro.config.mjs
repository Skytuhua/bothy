// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// The public address of the site; set SITE_URL when building for the real domain.
const site = process.env.SITE_URL ?? 'http://localhost:4321';

export default defineConfig({
  site,
  integrations: [
    starlight({
      title: 'Bothy',
      description:
        "Serverless, local-first sync over the user's own Google Drive. No server, offline-first, end-to-end encrypted, fast between online devices.",
      logo: { src: './src/assets/logo.svg' },
      favicon: '/favicon.svg',
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/bothy-sync/bothy' }],
      editLink: { baseUrl: 'https://github.com/bothy-sync/bothy/edit/main/site/' },
      customCss: ['./src/styles/theme.css'],
      lastUpdated: true,
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'What is Bothy?', slug: 'start/introduction' },
            { label: 'Start in 15 minutes', slug: 'start/quick-start' },
            { label: 'Set up Google sign-in', slug: 'start/google-cloud' },
          ],
        },
        {
          label: 'Concepts',
          items: [
            { label: 'How sync works', slug: 'concepts/how-sync-works' },
            { label: 'Offline edits and conflicts', slug: 'concepts/conflicts' },
            { label: 'Encryption', slug: 'concepts/encryption' },
            { label: 'Direct channels (P2P)', slug: 'concepts/p2p' },
            { label: 'Where the files live', slug: 'concepts/remotes' },
            { label: 'Limits and quota', slug: 'concepts/limits' },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'Flutter apps', slug: 'guides/flutter' },
            { label: 'Python', slug: 'guides/python' },
            { label: 'Robots and ROS 2', slug: 'guides/robots' },
            { label: 'Command line', slug: 'guides/cli' },
            { label: 'Rust library', slug: 'guides/rust' },
            { label: 'C and C++', slug: 'guides/c-abi' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'API documentation', slug: 'reference/api' },
            { label: 'File format', slug: 'reference/format' },
            { label: 'FAQ', slug: 'reference/faq' },
            { label: 'Release notes', slug: 'reference/releases' },
          ],
        },
      ],
    }),
  ],
});
