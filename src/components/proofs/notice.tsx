/** A proof panel's one outcome. Ready and fault cannot both be showing. */
export type ProofNotice = { kind: "ready" | "fault"; text: string };

export function ProofNoticeLine({ notice }: { notice: ProofNotice | null }) {
  if (!notice) return null;
  const tone = notice.kind === "ready" ? "text-emerald-200" : "text-red-300";
  return <p className={`text-xs ${tone}`}>{notice.text}</p>;
}
