import {
  isSurfaceEnabled,
  useFeatureGate,
  type FeatureFlags,
} from "../../features";

export interface GlossaryEntry {
  term: string;
  definition: string;
  /** Set when the term describes a surface that is compiled out by default. */
  feature?: keyof FeatureFlags;
}

/**
 * Help glossary. Entries tagged with a feature render only when that
 * feature is compiled in. Untagged entries are always shown.
 */
export default function GlossaryList({
  entries,
}: {
  entries: readonly GlossaryEntry[];
}) {
  const gate = useFeatureGate();
  const visible = entries.filter((entry) => isSurfaceEnabled(gate, entry.feature));
  return (
    <div className="space-y-2">
      {visible.map(({ term, definition }) => (
        <div key={term}>
          <span className="font-semibold text-gold-400">{term}</span>
          <span className="text-purple-300"> — {definition}</span>
        </div>
      ))}
    </div>
  );
}
