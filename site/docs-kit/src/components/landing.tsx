'use client';

import { useEffect, useRef } from 'react';

/**
 * The end of a one-page site's landing, a rule across the page. What comes before it is centred on
 * the window with nothing beside it, and the table of contents waits under the rule: it scrolls up
 * with the docs below it and sticks at the top once the rule has gone (style.css, "The landing").
 * The rule sets `--dk-landing-offset` on `<html>`, how far below the window's top its bottom is,
 * on each frame that scrolls or resizes the window, and `data-dk-landing` once it has, before which
 * the table is hidden: so it never shows beside the landing, as it would where it stuck.
 */
export function LandingEnd() {
  const ref = useRef<HTMLHRElement>(null);
  useEffect(() => {
    const root = document.documentElement;
    let frame = 0;
    const update = () => {
      frame = 0;
      const rule = ref.current;
      if (!rule) return;
      root.style.setProperty('--dk-landing-offset', `${Math.max(0, rule.getBoundingClientRect().bottom)}px`);
      root.setAttribute('data-dk-landing', '');
    };
    const schedule = () => {
      if (frame === 0) frame = requestAnimationFrame(update);
    };
    update();
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
      root.style.removeProperty('--dk-landing-offset');
      root.removeAttribute('data-dk-landing');
    };
  }, []);
  return <hr ref={ref} className="dk-landing-end" />;
}
