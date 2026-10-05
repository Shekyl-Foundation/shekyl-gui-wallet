import PaymentProof from "../components/proofs/PaymentProof";
import ReserveProof from "../components/proofs/ReserveProof";

/** Prove and check a payment or a reserve. Each panel keeps its own proof. */
export default function Proofs() {
  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Proofs</h1>
      <p className="text-xs text-purple-300">
        A payment proof shows that a transaction paid an address. A reserve proof
        shows that an address holds funds. Anyone who receives the proof string
        can check it.
      </p>
      <PaymentProof />
      <ReserveProof />
    </div>
  );
}
