import Multisig from "../../pages/Multisig";
import { isSurfaceEnabled, useFeatureGate } from "../../features";

/**
 * Route element for `/multisig`. The route stays registered so a catch-all
 * cannot paint another page while the flag is still loading. This panel
 * owns the fetch and renders the page only when the compiled feature is on.
 */
export default function MultisigRoute() {
  const gate = useFeatureGate();
  if (!isSurfaceEnabled(gate, "multisig")) return null;
  return <Multisig />;
}
