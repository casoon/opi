import { docsSchema } from '@casoon/pages-theme/content';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';

// Sources: ../docs (docs/ in the project repository). Everything there is published:
// the group folders, and the pages describing the codebase itself that sit at the root
// of docs/. `*/**/*` matches nothing at that root, so those needed `*.md` of their own —
// without it they carried page frontmatter that nothing read, and the site linked them
// as raw markdown on GitHub instead.
export const collections = {
  docs: defineCollection({
    loader: glob({ pattern: ['*.md', '*/**/*.{md,mdx}'], base: '../docs' }),
    schema: docsSchema,
  }),
};
