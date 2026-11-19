import { Children, type ReactNode } from 'react';

/**
 * A query's code block with its schema a hover away: `<SchemaQuery>` around two children, the
 * schema (a fence, or a group of dialect tabs) and then the query. The query shows as usual, and a
 * "View schema" link under its bottom right opens the schema in a popover, on hover or on focus.
 * The schema is in the page's HTML either way.
 */
export function SchemaQuery({ children }: { children: ReactNode }) {
  const [schema, query] = Children.toArray(children);
  return (
    <div className="schema-query">
      {query}
      <div className="schema-query-foot">
        <span className="schema-query-link" tabIndex={0}>
          View schema
          <span className="schema-query-pop" role="dialog" aria-label="Schema">
            <span className="schema-query-card not-prose">{schema}</span>
          </span>
        </span>
      </div>
    </div>
  );
}
