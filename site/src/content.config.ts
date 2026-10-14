import { docsSchema } from '@casoon/pages-theme/content';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';

// Sources: ../docs (docs/ in the project repository). The living project docs at the
// root of docs/ (German, for development) stay off the site; only index.md and the
// group folders are published.
export const collections = {
  docs: defineCollection({
    loader: glob({ pattern: ['index.md', '*/**/*.{md,mdx}'], base: '../docs' }),
    schema: docsSchema,
  }),
};
