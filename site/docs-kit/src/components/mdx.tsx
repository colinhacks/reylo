import defaultMdxComponents from 'fumadocs-ui/mdx';
import { CodeBlock, Pre } from 'fumadocs-ui/components/codeblock';
import { Tab, Tabs } from 'fumadocs-ui/components/tabs';
import * as Twoslash from 'fumadocs-twoslash/ui';
import type { MDXComponents } from 'mdx/types';
import type { ComponentProps } from 'react';
import { Accordion, Accordions } from './accordion';
import { Callout } from './callout';
import { CodeCopyButton } from './code-copy';
import { CodeTabs } from './code-tabs';
import { Details } from './details';
import { Heading } from './heading';

/**
 * The MDX components every page gets: Fumadocs' defaults; headings with the link icon in the left
 * margin and the copy-as-Markdown button (./heading.tsx); accordions whose closed items are in the
 * HTML (./accordion.tsx); Fumadocs' tabs, every panel rendered; the quiet `<Details>`; the callout
 * with its header bar (./callout.tsx); code-tab groups that put a shared file name at the end of the
 * strip (./code-tabs.tsx); and Twoslash's popups, opening fast. A site spreads its own components
 * over these (its variant tabs, its agent card with its title, its figures).
 *
 * No content of the page is left out of its HTML for being folded away, in a closed accordion, an
 * inactive tab or a closed `<Details>`. Radix renders only the active
 * tab's panel, so a reader of the HTML (a coding agent, a search engine) saw one of Reylo's ten
 * Setup tabs; every `<Tab>` is now rendered (`forceMount`) and an inactive one hidden by
 * `[role=tabpanel][data-state=inactive]` in style.css.
 */
export function docsMdxComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    h2: (props) => <Heading as="h2" {...props} />,
    h3: (props) => <Heading as="h3" {...props} />,
    h4: (props) => <Heading as="h4" {...props} />,
    Accordion,
    Accordions,
    Callout,
    Details,
    Tab: (props: ComponentProps<typeof Tab>) => <Tab forceMount {...props} />,
    Tabs,
    CodeBlockTabs: CodeTabs,
    // Fumadocs' code block, with docs-kit's copy button (./code-copy.tsx) in place of its own: the
    // page's one copy icon, with a tooltip.
    pre: ({ ref: _ref, allowCopy, ...props }: ComponentProps<typeof CodeBlock>) => (
      <CodeBlock
        {...props}
        allowCopy={false}
        Actions={({ className }) =>
          String(allowCopy) === 'false' ? null : (
            <div className={['empty:hidden', className].filter(Boolean).join(' ')}>
              <CodeCopyButton />
            </div>
          )
        }
      >
        <Pre>{props.children}</Pre>
      </CodeBlock>
    ),
    // The hover popups of `ts twoslash` blocks.
    ...Twoslash,
    // Fumadocs waits 300ms before it opens a popup and as long before it closes one. 60ms opens it
    // as soon as the pointer settles on a name, and still leaves time to move into the popup.
    Popup: (props: ComponentProps<typeof Twoslash.Popup>) => <Twoslash.Popup {...props} delay={60} />,
    ...components,
  } satisfies MDXComponents;
}
