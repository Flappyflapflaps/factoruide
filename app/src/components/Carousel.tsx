import { useEffect, useState } from "react";
import { useT } from "../i18n";

export interface Slide {
  src: string;
  /** What the picture shows: its caption and its alternative text. */
  caption: string;
}

/** Seconds a picture stays before the next one (not while the pointer or the focus is on it). */
const STEP_SECONDS = 7;

/** Pictures one at a time: arrows, a dot per picture, the next one by itself now and then. */
export function Carousel({ slides }: { slides: Slide[] }) {
  const t = useT();
  const [index, setIndex] = useState(0);
  const [held, setHeld] = useState(false);
  const go = (i: number) => setIndex((i + slides.length) % slides.length);

  useEffect(() => {
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (held || still || slides.length < 2) return;
    const timer = window.setTimeout(() => go(index + 1), STEP_SECONDS * 1000);
    return () => window.clearTimeout(timer);
  });

  const slide = slides[index];
  return (
    <section
      className="carousel"
      aria-roledescription={t("carrousel")}
      aria-label={t("Captures d'écran")}
      onMouseEnter={() => setHeld(true)}
      onMouseLeave={() => setHeld(false)}
      onFocus={() => setHeld(true)}
      onBlur={() => setHeld(false)}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") go(index - 1);
        if (e.key === "ArrowRight") go(index + 1);
      }}
    >
      <div className="carousel-frame">
        {slides.map((s, i) => (
          <img key={s.src} src={s.src} alt={s.caption} className={i === index ? "shown" : ""} aria-hidden={i !== index} loading={i === 0 ? "eager" : "lazy"} />
        ))}
        <button className="carousel-arrow prev" onClick={() => go(index - 1)} aria-label={t("Image précédente")}>
          ‹
        </button>
        <button className="carousel-arrow next" onClick={() => go(index + 1)} aria-label={t("Image suivante")}>
          ›
        </button>
      </div>
      <p className="carousel-caption" aria-live="polite">
        {slide.caption}
      </p>
      <div className="carousel-dots">
        {slides.map((s, i) => (
          <button key={s.src} className={i === index ? "active" : ""} onClick={() => go(i)} aria-label={t("Image {n} sur {count}", { n: i + 1, count: slides.length })} aria-current={i === index} />
        ))}
      </div>
    </section>
  );
}
