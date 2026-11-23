import { url } from '@casoon/pages-theme/url';
import type { UseCase } from '@casoon/pages-theme/use-cases';

export const lead =
  'Five jobs opi gets used for. Each one names the command and where to read on.';

export const useCases: UseCase[] = [
  {
    slug: 'no-ci',
    eyebrow: 'Git · Pre-push',
    title: 'Keep a red state from leaving a repository without CI',
    body: 'A private repository with no pipeline has nothing between a broken build and the remote. The pre-push hook runs every check and the build first, and refuses the push when one fails.',
    tags: ['Git hooks', 'No CI'],
    steps: [
      'Pick which checks the push runs; the choice is saved in `.opi/checks.json` to share in Git.',
      'Install `opi --check push` as the pre-push hook, in `.husky` when the repository has one.',
      'Push as usual: a clean tree, not behind upstream, the checks and the build, or no push.',
    ],
    code: {
      title: 'Terminal',
      label: 'sh',
      lang: 'sh',
      code: `opi --checks push
opi --hooks
git push`,
      marks: [
        { line: 1, n: 1 },
        { line: 2, n: 2 },
        { line: 3, n: 3 },
      ],
    },
    links: [
      { label: 'Workflows', href: url('docs/guides/workflows/') },
      { label: 'Hooks and CI', href: url('docs/guides/integrations/') },
    ],
  },
  {
    slug: 'agents',
    eyebrow: 'Coding agents',
    title: 'Let a coding agent check its own changes',
    body: 'An agent that writes code should not be the one that judges it. A hook that runs opi after each edit hands the agent the findings of the tools the project already uses, the same ones you would see.',
    tags: ['Agents', 'JSON'],
    specs: [
      { term: 'Command', value: '`opi --check commit --json`' },
      { term: 'Verdict', value: '`ready`, `gates` and `checks`, schema 1' },
      { term: 'Exit code', value: 'Non-zero when not ready' },
    ],
    links: [
      { label: 'Hooks and CI', href: url('docs/guides/integrations/#anything-else') },
      { label: 'JSON output', href: url('docs/reference/cli/#json-output') },
    ],
  },
  {
    slug: 'monorepo',
    eyebrow: 'Workspaces',
    title: 'Find the right script in a monorepo',
    body: 'Forty-odd scripts across a workspace is a list nobody reads. opi groups them into tabs, searches all of them as you type and runs a member’s script by its path.',
    tags: ['npm', 'pnpm', 'Cargo'],
    specs: [
      { term: 'Tabs', value: 'Past 15 entries or 5 groups' },
      { term: 'Search', value: '`/` across every group' },
      { term: 'Member', value: '`opi blog/dev`' },
    ],
    links: [{ label: 'Scripts', href: url('docs/guides/scripts/') }],
  },
  {
    slug: 'release',
    eyebrow: 'Release',
    title: 'Check a release before you tag it',
    body: 'A release is the push you cannot take back. The release check runs every detected check and the build on a clean tree, and asks what only a release has to: is this version already tagged?',
    tags: ['Release', 'SBOM'],
    specs: [
      { term: 'Command', value: '`opi --check release`' },
      { term: 'Gates', value: 'Clean working tree, `v<version>` not tagged yet' },
      { term: 'Then', value: '`opi --sbom` writes an SBOM to `.opi/sbom/`' },
    ],
    links: [{ label: 'Workflows', href: url('docs/guides/workflows/') }],
  },
  {
    slug: 'many-repositories',
    eyebrow: 'Many repositories',
    title: 'Check every repository in one folder',
    body: 'Without a terminal opi changes nothing and asks nothing, so a short shell loop can walk a folder of checkouts and leave one report per repository.',
    tags: ['Shell', 'Reports'],
    specs: [
      { term: 'Runs', value: 'Health and Security, per repository' },
      { term: 'Output', value: 'A report each, a line per repository' },
      { term: 'Platforms', value: 'Plain `sh` on macOS and Linux' },
    ],
    links: [
      { label: 'The script', href: url('docs/guides/integrations/#across-many-repositories') },
    ],
  },
];

export const tradeoffs = {
  eyebrow: 'The limits',
  title: 'What opi does not do',
  lead: 'Better read here than found later.',
  items: [
    'It brings no rules of its own apart from the bundled secret scan: a check exists only where the project already has the tool.',
    'It installs nothing. Dependencies have to be in place, or the checks that need them report that they could not run.',
    'It runs on macOS and Linux, not on Windows.',
    'Yarn Berry has no outdated command, so Updates lists nothing there; Yarn classic is covered.',
  ],
};
