import { ansiToHtml } from '@casoon/pages-theme/ansi';
import type { ShowcaseExample } from '@casoon/pages-theme/showcase';

// Real output of opi 0.8.0, run on its own repository by site/captures/capture.sh.
// The .ansi files were captured in a terminal and keep their colours; the .txt files
// were piped, which is how opi prints without a terminal.
const files = import.meta.glob<string>('../captures/*.{ansi,txt}', {
  query: '?raw',
  import: 'default',
  eager: true,
});

export function capture(file: string): string {
  const found = files[`../captures/${file}`];
  if (found === undefined) throw new Error(`Unknown capture: ${file}`);
  return found;
}

const catalogue = [
  {
    slug: 'script-list',
    title: 'Script list',
    file: 'list.txt',
    command: 'opi | cat',
    tags: ['scripts', 'cargo', 'piped'],
    description:
      'A Rust-only project: its commands are a fixed set, grouped like npm scripts. Piped, the list prints every group as plain text.',
  },
  {
    slug: 'health',
    title: 'Health',
    file: 'health.ansi',
    command: 'opi --health',
    tags: ['health', 'cargo'],
    description:
      'Every detected check runs concurrently and prints as it finishes, with the tool that answered.',
  },
  {
    slug: 'before-commit',
    title: 'Before commit',
    file: 'commit.ansi',
    command: 'opi --check commit',
    tags: ['workflow', 'cargo'],
    description: 'The commit workflow: the fast checks, without tests and dead-code analysis.',
  },
  {
    slug: 'security',
    title: 'Security',
    file: 'security.ansi',
    command: 'opi --security',
    tags: ['security', 'cargo audit'],
    description:
      'No secret scanner in this project, and opi says so instead of reporting a clean scan. The crates are audited with cargo audit.',
  },
  {
    slug: 'updates',
    title: 'Updates',
    file: 'updates.txt',
    command: 'opi --updates | cat',
    tags: ['updates', 'cargo outdated', 'piped'],
    description:
      'Outdated dependencies sorted by semver jump, one section per ecosystem. Without a terminal there is no offer to apply them.',
  },
  {
    slug: 'clean',
    title: 'Clean',
    file: 'clean.txt',
    command: 'opi --clean | cat',
    tags: ['clean', 'piped'],
    description:
      'Removable artefacts with their size. Nothing is removed without someone choosing it, so a pipe gets the inventory only.',
  },
];

export const examples: ShowcaseExample[] = catalogue.map(({ file, command, ...meta }) => {
  const source = capture(file);
  return {
    ...meta,
    file: `site/captures/${file}`,
    input: { code: `$ ${command}`, lang: 'sh' },
    output: { html: ansiToHtml(source), kind: 'terminal' },
  };
});
