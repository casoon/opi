// @ts-check
import { basename } from 'node:path';
import { satteri } from '@astrojs/markdown-satteri';
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

/**
 * The living docs at the root of docs/ link each other as `decisions.md`, which is what
 * GitHub resolves. On the site each page is a directory, so the link becomes `../decisions/`.
 * @type {(ctx: { fileURL: URL | undefined }) => any}
 */
const siblingMarkdownLinks = ({ fileURL }) => {
  const prefix = fileURL && basename(fileURL.pathname) === 'index.md' ? '' : '../';
  return {
    name: 'sibling-markdown-links',
    /** @param {any} node @param {any} ctx */
    link(node, ctx) {
      const match = /^([\w-]+)\.md(#.*)?$/.exec(node.url);
      if (match) ctx.setProperty(node, 'url', `${prefix}${match[1]}/${match[2] ?? ''}`);
    },
  };
};

// Project page: https://casoon.github.io/opi/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/opi',
  base: '/opi/',
  markdown: { processor: satteri({ mdastPlugins: [siblingMarkdownLinks] }) },
  integrations: [
    casoonPages({
      name: 'opi',
      description:
        'Operations Interface — a project control center for your terminal: scripts, health, security, updates and clean for npm, pnpm, bun and Cargo projects.',
      repo: 'casoon/opi',
      version: '0.11.0',
      license: 'MIT',
      packages: [
        { label: 'crates.io', href: 'https://crates.io/crates/opi' },
        { label: 'GitHub releases', href: 'https://github.com/casoon/opi/releases' },
      ],
      // Root pages (the overview and the living project docs) join the first group.
      docsGroups: {
        project: 'Project',
        'getting-started': 'Getting started',
        guides: 'Guides',
        reference: 'Reference',
      },
    }),
  ],
});
