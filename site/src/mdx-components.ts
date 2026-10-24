import Diagram from './components/Diagram.astro';

/**
 * Project components available in `docs/` MDX without an import — nothing there can
 * resolve a package or a relative path, so the theme injects these by name.
 */
export const components = { Diagram };
