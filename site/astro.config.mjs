// @ts-check
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

// Project page: https://casoon.github.io/opi/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/opi',
  base: '/opi/',
  integrations: [
    casoonPages({
      name: 'opi',
      description:
        'Operations Interface — a project control center for your terminal: scripts, health, security, updates and clean for npm, pnpm, bun and Cargo projects.',
      tagline: 'Project control center for the terminal',
      repo: 'casoon/opi',
      version: '0.11.0',
      license: 'MIT',
      packages: [
        { label: 'crates.io', href: 'https://crates.io/crates/opi' },
        { label: 'GitHub releases', href: 'https://github.com/casoon/opi/releases' },
      ],
      mdxComponents: './src/mdx-components.ts',
      docsGroups: {
        // The theme puts pages at the root of docs/ in the first group, so the first
        // group is theirs: Overview and the pages about the codebase itself.
        '': 'The project',
        'getting-started': 'Getting started',
        guides: 'Guides',
        reference: 'Reference',
      },
    }),
  ],
});
