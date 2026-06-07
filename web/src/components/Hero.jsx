export default function Hero() {
  return (
    <section id="top" className="bg-grain border-b border-line/70">
      <div className="mx-auto max-w-6xl px-6 py-16 sm:py-20">
        <p className="text-xs font-semibold uppercase tracking-[0.3em] text-coral">
          YouTube · TikTok · Instagram · Twitter / X · Spotify
        </p>
        <h1 className="mt-3 max-w-2xl font-display text-5xl italic leading-[1.05] text-ink sm:text-6xl">
          Paste a link, watch it land in your downloads folder.
        </h1>
        <p className="mt-5 max-w-xl text-base leading-relaxed text-ink-soft">
          vdl fetches a quick preview so you know exactly what you're about to grab,
          then streams the download with live progress — right here in the browser.
        </p>
      </div>
    </section>
  );
}
