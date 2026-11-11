import { CopyAgentPrompt } from 'docs-kit/components';
import { useEffect, useRef, type ReactNode } from 'react';
import { wordmark } from '@/components/wordmark';
import { githubUrl } from '@/lib/layout.shared';

/**
 * The main page's title (app/routes/page.tsx): the mark over the name, centred, the name in
 * Orbitron, the face of the wordmark (app/global.css, `.hero-title`). The mark is decoration; the
 * heading reads as the name.
 */
export function HeroTitle({ children }: { children: ReactNode }) {
  return (
    <span className="hero-title">
      <svg viewBox="0 0 400 235.47" fill="currentColor" aria-hidden="true" className="hero-mark">
        <defs>
          {/* The bloom: the mark's shape blurred twice, near and far, lit gold and amber (app/global.css, .hero-bloom-*). */}
          <filter id="hero-bloom" x="-40%" y="-60%" width="180%" height="220%" colorInterpolationFilters="sRGB">
            <feGaussianBlur in="SourceAlpha" stdDeviation="8" result="near" />
            <feFlood className="hero-bloom-near" />
            <feComposite in2="near" operator="in" result="nearLight" />
            <feGaussianBlur in="SourceAlpha" stdDeviation="22" result="far" />
            <feFlood className="hero-bloom-far" />
            <feComposite in2="far" operator="in" result="farLight" />
            <feMerge>
              <feMergeNode in="farLight" />
              <feMergeNode in="nearLight" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <path d={wordmark.mark} filter="url(#hero-bloom)" />
      </svg>
      {children}
    </span>
  );
}

/**
 * Under the main page's subheader (content/docs/index.mdx): the licence, npm and GitHub stars
 * badges, then "Copy agent prompt", centred, as at the top of Zod's README.
 */
export function HeroActions({ agentPrompt }: { agentPrompt?: string }) {
  return (
    <div className="hero-actions not-prose">
      <p className="hero-badges">
        <a href="https://opensource.org/licenses/MIT">
          <img src="https://img.shields.io/github/license/colinhacks/reylo" alt="License: MIT" height={20} />
        </a>
        <a href="https://www.npmjs.com/package/reylo">
          <img src="https://img.shields.io/npm/v/reylo" alt="npm version" height={20} />
        </a>
        <a href={githubUrl}>
          <img src="https://img.shields.io/github/stars/colinhacks/reylo" alt="GitHub stars" height={20} />
        </a>
      </p>
      <CopyAgentPrompt text={agentPrompt} />
    </div>
  );
}

// The demo's markup as HTML: React leaves `muted` out of server-rendered HTML, and a browser
// autoplays only a muted video, so a <video> rendered by React would sit still until hydration.
// A phone takes the portrait cut (its media query matches app/global.css's breakpoint). Each cut is
// AV1 for the browsers that decode it and H.264 for the rest: the browser plays the first source
// whose type it can decode and whose media query matches. The desktop cut is 1344px wide, twice the
// column, all a 2x screen draws there.
const demoVideo =
  '<video autoplay loop muted playsinline preload="auto" aria-label="An editor: hovering user in main.ts shows its type; the SQL in get_user.sql gains columns and loses its where clause, and the type follows">' +
  '<source src="/demo/reylo-phone.av1.mp4" type="video/mp4; codecs=av01.0.05M.08" media="(max-width: 767px)">' +
  '<source src="/demo/reylo-phone.mp4" type="video/mp4; codecs=avc1.640032" media="(max-width: 767px)">' +
  '<source src="/demo/reylo.av1.mp4" type="video/mp4; codecs=av01.0.05M.08">' +
  '<source src="/demo/reylo.mp4" type="video/mp4; codecs=avc1.640032">' +
  '</video>';

/**
 * Under the badges and "Copy agent prompt": a looping recording of an editor, the type of a query's
 * result following its SQL as the SQL changes. Its box keeps the video's shape from the first paint
 * (app/global.css, `.hero-demo`), so nothing moves when the video loads; a reader who asks for
 * reduced motion gets its first frame, still.
 */
export function HeroDemo() {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const video = box.current?.querySelector('video');
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
    if (!video) return;
    const apply = () => {
      if (reduce.matches) {
        video.pause();
        video.currentTime = 0;
      } else void video.play().catch(() => {});
    };
    apply();
    reduce.addEventListener('change', apply);
    return () => reduce.removeEventListener('change', apply);
  }, []);
  return <div ref={box} className="hero-demo not-prose" dangerouslySetInnerHTML={{ __html: demoVideo }} />;
}
