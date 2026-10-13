import changelog from "../../../CHANGELOG.md?raw";
import { Markdown } from "../components/Markdown";
import { useT } from "../i18n";

/** The released versions of CHANGELOG.md, newest first (its preamble and the unreleased part left out). */
const RELEASES = changelog
  .split(/^(?=## )/m)
  .filter((part) => part.startsWith("## ") && !/^## Unreleased/i.test(part))
  .join("\n");

/** What changed in each version (the project's changelog, in English). */
export function ChangelogPage() {
  const t = useT();
  return (
    <div className="changelog">
      <div className="page-head">
        <h2>{t("Nouveautés")}</h2>
      </div>
      {/* Reachable by the keyboard, to scroll a long text without links. */}
      <section className="panel" tabIndex={0} aria-label={t("Nouveautés")}>
        <Markdown text={RELEASES} />
      </section>
    </div>
  );
}
