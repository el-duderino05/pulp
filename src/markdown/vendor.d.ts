// SPDX-License-Identifier: MPL-2.0

declare module 'markdown-it-sub' {
  import type { MarkdownIt } from 'markdown-it';
  type PluginSimple = (md: MarkdownIt) => void;
  const plugin: PluginSimple;
  export default plugin;
}

declare module 'markdown-it-sup' {
  import type { MarkdownIt } from 'markdown-it';
  type PluginSimple = (md: MarkdownIt) => void;
  const plugin: PluginSimple;
  export default plugin;
}
