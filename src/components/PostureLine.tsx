import { ShieldAlert } from "lucide-react";
import { useTor } from "../context/useTor";

/**
 * The node's Tor state, in the header, whenever a send would not leave
 * hidden (`docs/design/GUI_SYNC_LANE.md` GS-3). The words are core's
 * rendering, verbatim; the wallet rewords nothing. A node that withholds
 * its state is said to. A node that cannot be read at all has its own
 * notice, and nothing is claimed here.
 */
export default function PostureLine() {
  const { read } = useTor();
  if (!read || read.sends === "hidden") return null;
  return (
    <div
      role="status"
      data-testid="posture-line"
      className="flex items-start gap-2 border-b border-amber-500/30 bg-amber-500/10 px-6 py-2 text-xs text-amber-100"
    >
      <ShieldAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
      <span>{read.text}</span>
    </div>
  );
}
