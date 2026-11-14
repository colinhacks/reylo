import { Wordmark } from '@/components/wordmark';

/**
 * The wordmark, a link to the top of the page: docs-kit's chrome shows it at the top of the left
 * column, or above the title below xl. It is the yellow, and by night it glows (app/global.css,
 * `.reylo-brand`).
 */
export function Brand() {
  return (
    <a href="/" className="reylo-brand inline-block">
      <Wordmark className="h-6 w-auto" />
    </a>
  );
}
