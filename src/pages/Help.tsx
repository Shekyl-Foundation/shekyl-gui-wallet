import { useState } from "react";
import {
  BookOpen,
  Pickaxe,
  Coins,
  ShieldCheck,
  HelpCircle,
  ChevronDown,
  ChevronRight,
} from "lucide-react";

interface SectionProps {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  id: string;
  children: React.ReactNode;
  open: string | null;
  onToggle: (id: string) => void;
}

function Section({ icon: Icon, title, id, children, open, onToggle }: SectionProps) {
  const isOpen = open === id;
  return (
    <div className="card">
      <button
        onClick={() => onToggle(id)}
        className="flex w-full items-center gap-3 text-left"
      >
        <Icon className="h-5 w-5 shrink-0 text-gold-400" />
        <span className="flex-1 text-sm font-semibold text-white">{title}</span>
        {isOpen ? (
          <ChevronDown className="h-4 w-4 text-purple-400" />
        ) : (
          <ChevronRight className="h-4 w-4 text-purple-400" />
        )}
      </button>
      {isOpen && (
        <div className="mt-4 space-y-3 border-t border-purple-700/50 pt-4 text-xs leading-relaxed text-purple-200">
          {children}
        </div>
      )}
    </div>
  );
}

export default function Help() {
  const [open, setOpen] = useState<string | null>("getting-started");

  function toggle(id: string) {
    setOpen((prev) => (prev === id ? null : id));
  }

  return (
    <div className="mx-auto max-w-2xl space-y-4">
      <div className="flex items-center gap-3">
        <HelpCircle className="h-6 w-6 text-gold-400" />
        <h1 className="text-xl font-bold text-white">Help Center</h1>
      </div>

      <Section
        icon={BookOpen}
        title="Getting Started"
        id="getting-started"
        open={open}
        onToggle={toggle}
      >
        <div className="space-y-3">
          <div>
            <h4 className="font-semibold text-white">What is Shekyl?</h4>
            <p>
              Shekyl (SKL) is a privacy-focused cryptocurrency with
              post-quantum security. It uses ring signatures and stealth
              addresses to keep your transactions private, and hybrid
              cryptographic signatures to protect against future quantum
              computing threats.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Connecting to the Daemon</h4>
            <p>
              The wallet connects to a <code>shekyld</code> daemon to read
              blockchain data. By default it looks for a local daemon at{" "}
              <code>127.0.0.1:11029</code>. You can change this in{" "}
              <strong>Settings &gt; Daemon Connection</strong>.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Creating a Wallet</h4>
            <p>
              On first launch, click <strong>Create New Wallet</strong> on the
              Welcome screen. Choose a name and password, then carefully save
              your 25-word mnemonic seed. You'll need to confirm 4 random words
              from your seed before proceeding. This seed is the only way to
              recover your funds if you lose access. Your wallet is
              automatically protected by hybrid PQC signatures.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Importing a Wallet</h4>
            <p>
              If you already have a Shekyl wallet, click{" "}
              <strong>Import Existing Wallet</strong> on the Welcome screen.
              You can restore from a 25-word seed phrase or from your private
              keys (spend key + view key). Set a restore height to speed up
              blockchain scanning, or leave it at 0 to scan everything.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Sending and Receiving</h4>
            <p>
              To receive SKL, go to the <strong>Receive</strong> page and share
              your address. To send, use the <strong>Send</strong> page with
              the recipient's address and the amount. Transactions use ring
              signatures to hide the true sender among a group of decoys.
            </p>
          </div>
        </div>
      </Section>

      <Section
        icon={Pickaxe}
        title="Mining Guide"
        id="mining"
        open={open}
        onToggle={toggle}
      >
        <div className="space-y-3">
          <div>
            <h4 className="font-semibold text-white">What is Mining?</h4>
            <p>
              Mining is the process of using your computer's CPU to solve
              cryptographic puzzles that secure the Shekyl network. Miners who
              find valid solutions earn block rewards in SKL. Shekyl uses the{" "}
              <strong>RandomX</strong> proof-of-work algorithm, which is
              optimized for general-purpose CPUs rather than specialized
              hardware.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">How to Mine</h4>
            <p>
              Go to the <strong>Mining</strong> page, enter your SKL address,
              choose how many CPU threads to use, and click{" "}
              <strong>Start Mining</strong>. More threads means more hash rate
              but higher CPU usage. The <strong>Background Mining</strong>{" "}
              option reduces CPU priority so mining won't interfere with normal
              use.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Block Rewards</h4>
            <p>
              When you mine a block, you receive the block reward plus
              transaction fees. However, mined coins are{" "}
              <strong>locked for 60 blocks</strong> (~2 hours) before they
              become spendable. This lock period is a consensus rule that
              prevents double-spending during chain reorganizations.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Requirements</h4>
            <p>
              Mining requires an <strong>unrestricted daemon</strong> (do not
              run shekyld with <code>--restricted-rpc</code>). Solo mining on
              mainnet with a single CPU is unlikely to find blocks frequently,
              but it contributes to network decentralization and security.
            </p>
          </div>
        </div>
      </Section>

      <Section
        icon={Coins}
        title="Staking Guide"
        id="staking"
        open={open}
        onToggle={toggle}
      >
        <div className="space-y-3">
          <div>
            <h4 className="font-semibold text-white">
              What is Staking in Shekyl?
            </h4>
            <p>
              Shekyl uses a <strong>claim-based staking model</strong>, not
              delegation. You lock SKL for a chosen duration and earn a share
              of the emission pool when you claim your rewards. This is
              different from delegated proof-of-stake -- you retain full
              custody of your funds.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Tiers and Lock Durations</h4>
            <p>
              There are three staking tiers with increasing lock periods and
              yield multipliers. <strong>Tier 0</strong> has a short lock with
              1x multiplier, <strong>Tier 1</strong> has a medium lock with
              1.5x, and <strong>Tier 2</strong> has the longest lock with 2x.
              Longer locks earn proportionally more rewards.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Privacy Benefit</h4>
            <p>
              Staked funds <strong>commingle in the accrual pool</strong>.
              When you claim rewards, they draw from pooled funds, providing{" "}
              <strong>plausible deniability</strong> on the source of yield.
              Staking is both yield generation and privacy participation.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">Estimated APY</h4>
            <p>
              The APY shown on the Staking page is an estimate based on
              current network conditions -- emission rate, total staked amount,
              and your chosen tier's multiplier. Actual returns depend on
              network activity over your lock period.
            </p>
          </div>
        </div>
      </Section>

      <Section
        icon={ShieldCheck}
        title="Post-Quantum Cryptography"
        id="pqc"
        open={open}
        onToggle={toggle}
      >
        <div className="space-y-3">
          <div>
            <h4 className="font-semibold text-white">Why PQC Matters</h4>
            <p>
              Future quantum computers could break the elliptic-curve
              cryptography (like Ed25519) that most cryptocurrencies rely on.
              A sufficiently powerful quantum computer could derive your
              private key from your public key, potentially stealing funds.
              Shekyl addresses this threat proactively.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">
              How Shekyl Protects You Today
            </h4>
            <p>
              Every spend transaction uses a{" "}
              <strong>hybrid signature</strong>: both a classical{" "}
              <strong>Ed25519</strong> signature and a post-quantum{" "}
              <strong>ML-DSA-65</strong> (FIPS 204, formerly Dilithium)
              signature. Both signatures must verify for a transaction to be
              valid. This means even if one scheme is broken, the other still
              protects your funds.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">
              What "Hybrid" Means
            </h4>
            <p>
              The hybrid approach is a belt-and-suspenders strategy. The
              classical Ed25519 component provides proven, battle-tested
              security. The ML-DSA-65 component provides quantum resistance.
              Neither alone is a single point of failure -- an attacker would
              need to break <em>both</em> schemes simultaneously.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">V4 Roadmap</h4>
            <p>
              Transaction version V4 will introduce{" "}
              <strong>lattice-based ring signatures</strong> for quantum-
              resistant input privacy, and{" "}
              <strong>KEM stealth addresses</strong> (X25519 + ML-KEM-768)
              for quantum-resistant one-time address generation. This will
              provide full post-quantum privacy for both who sends and who
              receives.
            </p>
          </div>
          <div>
            <h4 className="font-semibold text-white">
              Coinbase Transactions
            </h4>
            <p>
              Mining (coinbase) transactions do not require PQC signatures --
              they are generated by the consensus protocol itself and don't
              involve spending existing outputs. Only user-initiated spend
              transactions carry hybrid PQC authentication.
            </p>
          </div>
        </div>
      </Section>

      <Section
        icon={HelpCircle}
        title="Glossary"
        id="glossary"
        open={open}
        onToggle={toggle}
      >
        <div className="space-y-2">
          {GLOSSARY.map(({ term, definition }) => (
            <div key={term}>
              <span className="font-semibold text-gold-400">{term}</span>
              <span className="text-purple-300"> — {definition}</span>
            </div>
          ))}
        </div>
      </Section>
    </div>
  );
}

const GLOSSARY = [
  {
    term: "Atomic Unit",
    definition:
      "The smallest indivisible unit of SKL. 1 SKL = 1,000,000,000 atomic units (9 decimal places).",
  },
  {
    term: "Block Height",
    definition:
      "The sequential number of a block in the blockchain, starting from 0 (genesis).",
  },
  {
    term: "Difficulty",
    definition:
      "A measure of how hard it is to mine a block. Adjusts automatically to maintain the 2-minute target block time.",
  },
  {
    term: "Emission Era",
    definition:
      "Named phases of the coin supply schedule (e.g., Foundation, Growth). Each era has different emission characteristics.",
  },
  {
    term: "Hash Rate",
    definition:
      "The number of cryptographic hashes your CPU computes per second when mining. Measured in H/s, KH/s, MH/s, etc.",
  },
  {
    term: "ML-DSA-65",
    definition:
      "A NIST-standardized post-quantum digital signature algorithm (formerly Dilithium). Used alongside Ed25519 in Shekyl's hybrid scheme.",
  },
  {
    term: "RandomX",
    definition:
      "A proof-of-work algorithm optimized for general-purpose CPUs, making mining accessible without specialized hardware.",
  },
  {
    term: "Release Multiplier",
    definition:
      "A dynamic factor that adjusts block emission based on network activity. Higher activity means slightly more coins released per block.",
  },
  {
    term: "Ring Signature",
    definition:
      "A privacy technique where your transaction is signed alongside decoy inputs, making it impossible to determine which input is the real one.",
  },
  {
    term: "Stake Ratio",
    definition:
      "The percentage of circulating supply that is currently staked. Higher ratios indicate strong network participation.",
  },
  {
    term: "Stealth Address",
    definition:
      "A one-time address generated for each transaction, ensuring that only the sender and receiver know the destination.",
  },
];
