import { Users } from "lucide-react";

import CollapsibleSection from "../CollapsibleSection";
import { isSurfaceEnabled, useFeatureGate } from "../../features";

interface MultisigHelpSectionProps {
  /** The id of the section the Help page currently has open, if any. */
  open: string | null;
  onToggle: (id: string) => void;
}

const ID = "multisig";

/**
 * Help's multisig section. Renders nothing unless `get_feature_flags`
 * reports the cargo feature. The page only composes this panel.
 */
export default function MultisigHelpSection({ open, onToggle }: MultisigHelpSectionProps) {
  const gate = useFeatureGate();
  if (!isSurfaceEnabled(gate, "multisig")) return null;
  return (
    <CollapsibleSection
      icon={Users}
      title="Multisig Wallets"
      open={open === ID}
      onToggle={() => onToggle(ID)}
    >
        <div className="space-y-3">
          <div>
            <h4 className="font-semibold text-white">What is Multisig?</h4>
            <p>
              Multisig (multi-signature) lets a group of participants share
              control of a wallet. An <strong>M-of-N</strong> multisig requires
              at least <em>M</em> out of <em>N</em> total signers to approve
              each transaction. For example, a 2-of-3 multisig needs any two of
              the three key holders to sign before funds can move.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Why Use Multisig?</h4>
            <p>
              Multisig improves security and enables shared custody. Common use
              cases include organizational treasuries where no single person can
              spend alone, family vaults with redundant recovery keys, and
              escrow arrangements where a neutral third party co-signs. Even a
              2-of-2 setup protects you if one device is compromised.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">
              PQC Hybrid Multisig
            </h4>
            <p>
              Shekyl's multisig uses the same{" "}
              <strong>Ed25519 + ML-DSA-65 hybrid</strong> scheme as single-signer
              transactions, extended to multiple participants. Each signer holds
              their own hybrid key pair. The group's combined public key and all
              collected signatures are encoded on-chain so the network can verify
              that the required threshold was met -- with both classical and
              post-quantum security.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Creating a Group</h4>
            <p>
              Go to the <strong>Multisig</strong> page and open the{" "}
              <strong>Setup Group</strong> tab. Enter the total number of
              participants (N, up to 7), the required signatures (M), and each
              participant's hybrid public key (shared out-of-band as hex). Click{" "}
              <strong>Create Multisig Group</strong>. The wallet will compute a{" "}
              <strong>Group ID</strong> -- a unique fingerprint all participants
              should verify matches.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Signing Flow</h4>
            <p>
              A coordinator creates a <strong>signing request</strong> containing
              the transaction details and shares it with other signers (as JSON).
              Each signer pastes the request into the{" "}
              <strong>Sign Request</strong> tab, reviews it, and clicks{" "}
              <strong>Sign</strong>. The resulting signature response is sent
              back to the coordinator. Once M responses are collected, the
              coordinator assembles the final transaction and broadcasts it.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">
              Limits and Security Notes
            </h4>
            <p>
              The maximum group size is <strong>7 participants</strong> (a
              consensus limit that bounds on-chain signature size). Multisig
              outputs are protected against{" "}
              <strong>scheme downgrade attacks</strong> -- an attacker cannot
              bypass the multisig requirement by submitting a single-signer
              transaction. Always verify the Group ID matches across all
              participants before funding the multisig address.
            </p>
          </div>
        </div>
    </CollapsibleSection>
  );
}
