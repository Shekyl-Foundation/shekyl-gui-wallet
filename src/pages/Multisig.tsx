import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Users,
  AlertCircle,
  CheckCircle2,
  FileDown,
  FileUp,
  PenTool,
  Copy,
  Settings,
  LayoutDashboard,
  AlertTriangle,
  Clock,
  RefreshCw,
  Wifi,
  WifiOff,
  ShieldAlert,
} from "lucide-react";
import {
  FingerprintBadge,
  ProverView,
  LossAcknowledgment,
  AddressProvenance,
  RelayConfig,
  ViolationAlert,
  SigningDashboard,
  GroupDescriptor,
} from "../components/multisig";

interface MultisigInfo {
  is_multisig: boolean;
  n_total: number;
  m_required: number;
  group_id: string;
  fingerprint?: string;
  spend_auth_version?: number;
  participant_keys?: string[];
}

type Tab = "setup" | "dashboard" | "sign" | "import" | "settings";

export default function Multisig() {
  const [info, setInfo] = useState<MultisigInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<Tab>("setup");

  const [violations, setViolations] = useState<
    { invariantId: string; invariantName: string; reporterIndex: number; intentHash: string; timestamp: number }[]
  >([]);
  const [relays, setRelays] = useState<{ url: string; operatorId: string }[]>([]);
  const [lossAcknowledged, setLossAcknowledged] = useState(false);
  const [relayConnected, setRelayConnected] = useState(true);
  const [counterDivergence, setCounterDivergence] = useState<{
    localCounter: number;
    peerCounter: number;
    peerId: number;
  } | null>(null);
  const [cosignerTimeout, setCosignerTimeout] = useState<{
    participantIndex: number;
    lastSeen: number;
  } | null>(null);
  const [counterProofFailure, setCounterProofFailure] = useState<{
    fromParticipant: number;
    reason: string;
  } | null>(null);

  const [intents, setIntents] = useState<
    {
      intentHash: string;
      state: "Proposed" | "Verified" | "ProverReady" | "Signed" | "Assembled" | "Broadcast" | "Rejected" | "TimedOut";
      proposerIndex: number;
      sigsCollected: number;
      sigsRequired: number;
      expiresAt: number;
      recipients: { address: string; amount: number }[];
      fee: number;
    }[]
  >([]);
  const [provenanceHistory, setProvenanceHistory] = useState<
    { fingerprint: string; timestamp: number; source: string }[]
  >([]);
  const [proverAssignments, setProverAssignments] = useState<
    { outputIndex: number; outputPubkey: string; assignedProver: number; amount: number }[]
  >([]);

  const loadInfo = useCallback(async () => {
    try {
      const result = await invoke<MultisigInfo>("get_multisig_info");
      setInfo(result);
      if (result.is_multisig) setTab("dashboard");
    } catch {
      setInfo(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadInfo();
  }, [loadInfo]);

  const nowSecs = Math.floor(Date.now() / 1000);
  const stuckIntents = intents.filter((i) => i.state === "Signed");
  const fingerprintChanged =
    provenanceHistory.length > 1 &&
    provenanceHistory[0].fingerprint !== provenanceHistory[1]?.fingerprint;
  const hasActiveAlerts =
    violations.length > 0 ||
    !relayConnected ||
    counterDivergence !== null ||
    cosignerTimeout !== null ||
    counterProofFailure !== null ||
    stuckIntents.length > 0 ||
    fingerprintChanged;

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-gold-400 border-t-transparent" />
      </div>
    );
  }

  const tabs = info?.is_multisig
    ? [
        { id: "dashboard" as Tab, label: "Dashboard", icon: LayoutDashboard },
        { id: "sign" as Tab, label: "Sign", icon: PenTool },
        { id: "import" as Tab, label: "File Transport", icon: FileUp },
        { id: "settings" as Tab, label: "Settings", icon: Settings },
      ]
    : [{ id: "setup" as Tab, label: "Setup Group", icon: Users }];

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold text-white">Multisig</h1>
        <div className="flex items-center gap-3">
          {hasActiveAlerts && (
            <span className="flex items-center gap-1 rounded-full bg-red-500/15 px-3 py-1 text-xs font-medium text-red-400">
              <AlertTriangle className="h-3 w-3" />
              Alerts
            </span>
          )}
          {info?.is_multisig && (
            <span className="rounded-full bg-gold-500/15 px-3 py-1 text-xs font-medium text-gold-400">
              {info.m_required}-of-{info.n_total}
            </span>
          )}
        </div>
      </div>

      {info?.is_multisig && info.fingerprint && (
        <FingerprintBadge
          fingerprint={info.fingerprint}
          mRequired={info.m_required}
          nTotal={info.n_total}
          spendAuthVersion={info.spend_auth_version ?? 2}
        />
      )}

      <div className="flex gap-1 rounded-lg bg-purple-900/50 p-1">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
              tab === id
                ? "bg-gold-500/15 text-gold-400"
                : "text-purple-300 hover:text-white"
            }`}
          >
            <Icon className="h-4 w-4" />
            {label}
          </button>
        ))}
      </div>

      {tab === "setup" && (
        <div className="space-y-6">
          <LossAcknowledgment
            nTotal={2}
            onAcknowledge={() => setLossAcknowledged(true)}
            acknowledged={lossAcknowledged}
          />
          {lossAcknowledged && <SetupGroup onCreated={loadInfo} />}
        </div>
      )}

      {tab === "dashboard" && info?.is_multisig && (
        <div className="space-y-6">
          {/* Failure-mode alerts */}
          <FailureAlerts
            violations={violations}
            relayConnected={relayConnected}
            counterDivergence={counterDivergence}
            cosignerTimeout={cosignerTimeout}
            counterProofFailure={counterProofFailure}
            stuckIntents={stuckIntents}
            fingerprintChanged={fingerprintChanged}
            onDismissCounterDivergence={() => setCounterDivergence(null)}
            onDismissCosignerTimeout={() => setCosignerTimeout(null)}
            onDismissCounterProofFailure={() => setCounterProofFailure(null)}
          />

          <SigningDashboard
            intents={intents}
            ourIndex={0}
            nowSecs={nowSecs}
            onSign={(hash) => console.log("sign", hash)}
            onVeto={(hash) => console.log("veto", hash)}
          />

          {proverAssignments.length > 0 && (
            <ProverView
              ourIndex={0}
              nTotal={info.n_total}
              assignments={proverAssignments}
            />
          )}
        </div>
      )}

      {tab === "sign" && <SignRequest />}
      {tab === "import" && <ImportAndSign />}

      {tab === "settings" && info?.is_multisig && (
        <div className="space-y-6">
          <GroupDescriptor
            groupId={info.group_id}
            mRequired={info.m_required}
            nTotal={info.n_total}
          />

          <RelayConfig
            relays={relays}
            onRelaysChange={setRelays}
          />

          {provenanceHistory.length > 0 && (
            <AddressProvenance
              current={provenanceHistory[0]}
              history={provenanceHistory}
              fingerprintChanged={fingerprintChanged}
            />
          )}
        </div>
      )}
    </div>
  );
}

// ─── Failure-mode alerts ──────────────────────────────────────────────────────

interface FailureAlertsProps {
  violations: { invariantId: string; invariantName: string; reporterIndex: number; intentHash: string; timestamp: number }[];
  relayConnected: boolean;
  counterDivergence: { localCounter: number; peerCounter: number; peerId: number } | null;
  cosignerTimeout: { participantIndex: number; lastSeen: number } | null;
  counterProofFailure: { fromParticipant: number; reason: string } | null;
  stuckIntents: { intentHash: string; state: string; expiresAt: number }[];
  fingerprintChanged: boolean;
  onDismissCounterDivergence: () => void;
  onDismissCosignerTimeout: () => void;
  onDismissCounterProofFailure: () => void;
}

function FailureAlerts({
  violations,
  relayConnected,
  counterDivergence,
  cosignerTimeout,
  counterProofFailure,
  stuckIntents,
  fingerprintChanged,
  onDismissCounterDivergence,
  onDismissCosignerTimeout,
  onDismissCounterProofFailure,
}: FailureAlertsProps) {
  const hasAny =
    violations.length > 0 ||
    !relayConnected ||
    counterDivergence !== null ||
    cosignerTimeout !== null ||
    counterProofFailure !== null ||
    stuckIntents.length > 0 ||
    fingerprintChanged;

  if (!hasAny) return null;

  return (
    <div className="space-y-3">
      {violations.length > 0 && (
        <ViolationAlert violations={violations} />
      )}

      {cosignerTimeout && (
        <AlertBanner
          severity="warning"
          icon={<Clock className="h-4 w-4" />}
          title="Co-signer unresponsive"
          onDismiss={onDismissCosignerTimeout}
        >
          Participant {cosignerTimeout.participantIndex + 1} has not sent a
          heartbeat since{" "}
          {new Date(cosignerTimeout.lastSeen * 1000).toLocaleTimeString()}.
          They may be offline. You can wait, or veto the active intent and
          retry when they are available.
        </AlertBanner>
      )}

      {counterDivergence && (
        <AlertBanner
          severity="warning"
          icon={<RefreshCw className="h-4 w-4" />}
          title="Transaction counter divergence"
          onDismiss={onDismissCounterDivergence}
        >
          Your local counter is {counterDivergence.localCounter} but
          participant {counterDivergence.peerId + 1} reports{" "}
          {counterDivergence.peerCounter}. A CounterProof exchange is needed
          to reconcile. This usually means a transaction was broadcast that
          your wallet missed. Let the reconciliation complete before signing
          new intents.
        </AlertBanner>
      )}

      {!relayConnected && (
        <AlertBanner severity="error" icon={<WifiOff className="h-4 w-4" />} title="Relay disconnected">
          No relay connection for 30+ minutes. Messages from co-signers may be
          delayed. Check your internet connection, or switch to a backup relay
          in Settings. You can also use file-based transport while the relay is
          down.
        </AlertBanner>
      )}

      {fingerprintChanged && (
        <AlertBanner severity="error" icon={<ShieldAlert className="h-4 w-4" />} title="Address fingerprint changed">
          The group address fingerprint differs from the last known value. This
          could indicate that a participant&apos;s key data has changed.
          <strong> Do not sign any new intents</strong> until you have verified
          the fingerprint with all group members out-of-band. Check the
          Settings tab for details.
        </AlertBanner>
      )}

      {stuckIntents.map((intent) => (
        <AlertBanner
          key={intent.intentHash}
          severity="warning"
          icon={<AlertTriangle className="h-4 w-4" />}
          title="Signed intent not broadcast"
        >
          Intent{" "}
          <code className="text-xs font-mono">{intent.intentHash.slice(0, 12)}...</code>{" "}
          has enough signatures but was never broadcast.
          {intent.expiresAt > Math.floor(Date.now() / 1000) ? (
            <> It expires in{" "}
              {Math.round((intent.expiresAt - Math.floor(Date.now() / 1000)) / 60)}{" "}
              minutes. Try rebroadcasting, or veto and recreate if the relay was
              down.</>
          ) : (
            <> It has expired. Create a new intent to retry the transaction.</>
          )}
        </AlertBanner>
      ))}

      {counterProofFailure && (
        <AlertBanner
          severity="error"
          icon={<ShieldAlert className="h-4 w-4" />}
          title="CounterProof verification failed"
          onDismiss={onDismissCounterProofFailure}
        >
          A CounterProof from participant {counterProofFailure.fromParticipant + 1}{" "}
          failed verification: {counterProofFailure.reason}. This may indicate a
          malicious participant or a chain reorganization. Do not accept their
          counter value. Contact them out-of-band to resolve, or wait for a
          valid proof.
        </AlertBanner>
      )}
    </div>
  );
}

// ─── Alert banner ─────────────────────────────────────────────────────────────

interface AlertBannerProps {
  severity: "warning" | "error";
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
  onDismiss?: () => void;
}

function AlertBanner({ severity, icon, title, children, onDismiss }: AlertBannerProps) {
  const colors =
    severity === "error"
      ? "border-red-500/30 bg-red-500/10 text-red-300"
      : "border-yellow-500/30 bg-yellow-500/10 text-yellow-300";

  return (
    <div className={`rounded-lg border p-4 ${colors}`}>
      <div className="flex items-start gap-3">
        <div className="mt-0.5 shrink-0">{icon}</div>
        <div className="flex-1 space-y-1">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-medium">{title}</h4>
            {onDismiss && (
              <button
                onClick={onDismiss}
                className="text-xs opacity-60 hover:opacity-100"
              >
                Dismiss
              </button>
            )}
          </div>
          <p className="text-xs leading-relaxed opacity-90">{children}</p>
        </div>
      </div>
    </div>
  );
}

// ─── Setup group ──────────────────────────────────────────────────────────────

function SetupGroup({ onCreated }: { onCreated: () => void }) {
  const [nTotal, setNTotal] = useState(2);
  const [mRequired, setMRequired] = useState(2);
  const [keys, setKeys] = useState<string[]>([""]);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  function updateKey(idx: number, value: string) {
    setKeys((prev) => prev.map((k, i) => (i === idx ? value : k)));
  }

  useEffect(() => {
    setKeys((prev) => {
      const next = [...prev];
      while (next.length < nTotal) next.push("");
      return next.slice(0, nTotal);
    });
  }, [nTotal]);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setCreating(true);
    try {
      await invoke("create_multisig_group", {
        nTotal,
        mRequired,
        participantKeys: keys.filter((k) => k.trim()),
      });
      onCreated();
    } catch (err) {
      setError(String(err));
    } finally {
      setCreating(false);
    }
  }

  return (
    <form onSubmit={handleCreate} className="card space-y-5">
      <p className="text-sm text-purple-300">
        Create a new PQC multisig group. You need the hybrid public key (hex)
        from each participant.
      </p>

      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-2">
          <label className="text-sm font-medium text-purple-200">
            Total Participants (N)
          </label>
          <input
            type="number"
            className="input"
            min={1}
            max={7}
            value={nTotal}
            onChange={(e) => setNTotal(Number(e.target.value))}
          />
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium text-purple-200">
            Required Signatures (M)
          </label>
          <input
            type="number"
            className="input"
            min={1}
            max={nTotal}
            value={mRequired}
            onChange={(e) => setMRequired(Number(e.target.value))}
          />
        </div>
      </div>

      <div className="space-y-3">
        <label className="text-sm font-medium text-purple-200">
          Participant Public Keys
        </label>
        {keys.map((k, i) => (
          <input
            key={i}
            type="text"
            className="input font-mono text-xs"
            placeholder={`Participant ${i + 1} hybrid public key (hex)`}
            value={k}
            onChange={(e) => updateKey(i, e.target.value)}
            required
          />
        ))}
      </div>

      {error && (
        <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      <button
        type="submit"
        disabled={creating}
        className="btn btn-primary w-full"
      >
        <Users className="h-4 w-4" />
        {creating ? "Creating..." : "Create Multisig Group"}
      </button>
    </form>
  );
}

// ─── Sign request ─────────────────────────────────────────────────────────────

function SignRequest() {
  const [request, setRequest] = useState("");
  const [response, setResponse] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);

  async function handleSign(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setResponse(null);
    setSigning(true);
    try {
      const result = await invoke<{ signature_response: string }>(
        "sign_multisig_partial",
        { signingRequest: request }
      );
      setResponse(result.signature_response);
    } catch (err) {
      setError(String(err));
    } finally {
      setSigning(false);
    }
  }

  return (
    <form onSubmit={handleSign} className="card space-y-5">
      <p className="text-sm text-purple-300">
        Paste a signing request from another participant, review, and sign with
        your key.
      </p>

      <div className="space-y-2">
        <label className="text-sm font-medium text-purple-200">
          Signing Request (JSON)
        </label>
        <textarea
          className="input min-h-[120px] font-mono text-xs"
          placeholder='{"version":1,"group_id":"...","payload_hash":"..."}'
          value={request}
          onChange={(e) => setRequest(e.target.value)}
          required
        />
      </div>

      {error && (
        <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      <button
        type="submit"
        disabled={signing}
        className="btn btn-primary w-full"
      >
        <PenTool className="h-4 w-4" />
        {signing ? "Signing..." : "Sign"}
      </button>

      {response && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-sm font-medium text-green-400">
              Signature Response
            </label>
            <button
              type="button"
              onClick={() => navigator.clipboard.writeText(response)}
              className="flex items-center gap-1 text-xs text-purple-400 hover:text-white"
            >
              <Copy className="h-3 w-3" />
              Copy
            </button>
          </div>
          <textarea
            className="input min-h-[120px] font-mono text-xs"
            value={response}
            readOnly
          />
          <p className="text-xs text-purple-400">
            Send this response back to the other participants.
          </p>
        </div>
      )}
    </form>
  );
}

// ─── File-based import & sign ─────────────────────────────────────────────────

function ImportAndSign() {
  const [request, setRequest] = useState<string | null>(null);
  const [response, setResponse] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);
  const [importPath, setImportPath] = useState("");

  async function handleImportFile() {
    if (!importPath.trim()) return;
    setError(null);
    setRequest(null);
    setResponse(null);
    try {
      const content = await invoke<string>("import_signing_request_file", {
        path: importPath.trim(),
      });
      setRequest(content);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleSign() {
    if (!request) return;
    setError(null);
    setSigning(true);
    try {
      const result = await invoke<{ signature_response: string }>(
        "sign_multisig_partial",
        { signingRequest: request }
      );
      setResponse(result.signature_response);
    } catch (err) {
      setError(String(err));
    } finally {
      setSigning(false);
    }
  }

  async function handleExportResponse() {
    if (!response) return;
    setError(null);
    const exportPath = importPath.replace(/\.[^.]+$/, "") + ".response.json";
    try {
      await invoke("export_signature_response_file", {
        response,
        path: exportPath,
      });
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="card space-y-5">
      <div className="flex items-center gap-3">
        <FileDown className="h-5 w-5 text-purple-400" />
        <div>
          <h3 className="font-medium text-white">File-Based Transport</h3>
          <p className="text-sm text-purple-300">
            For air-gapped operation: load signing requests from disk and save
            responses as files. No relay connection needed.
          </p>
        </div>
      </div>

      <div className="space-y-3">
        <label className="text-sm font-medium text-purple-200">
          Signing Request File
        </label>
        <div className="flex gap-2">
          <input
            type="text"
            className="input flex-1 font-mono text-xs"
            placeholder="/path/to/shekyl-ms-request.json"
            value={importPath}
            onChange={(e) => setImportPath(e.target.value)}
          />
          <button
            onClick={handleImportFile}
            disabled={!importPath.trim()}
            className="btn btn-secondary shrink-0"
          >
            <FileUp className="h-4 w-4" />
            Load
          </button>
        </div>
      </div>

      {request && (
        <div className="space-y-3">
          <div className="rounded-lg bg-purple-900/30 p-3">
            <label className="mb-1 block text-sm font-medium text-purple-200">
              Loaded Request
            </label>
            <pre className="max-h-32 overflow-auto whitespace-pre-wrap font-mono text-xs text-purple-300">
              {request.slice(0, 500)}{request.length > 500 ? "..." : ""}
            </pre>
          </div>

          <div className="flex gap-3">
            <button
              onClick={handleSign}
              disabled={signing}
              className="btn btn-primary flex-1"
            >
              <PenTool className="h-4 w-4" />
              {signing ? "Signing..." : "Sign This Request"}
            </button>
          </div>
        </div>
      )}

      {error && (
        <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {response && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <label className="text-sm font-medium text-green-400">
              Signature Response
            </label>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => navigator.clipboard.writeText(response)}
                className="flex items-center gap-1 text-xs text-purple-400 hover:text-white"
              >
                <Copy className="h-3 w-3" />
                Copy
              </button>
              <button
                type="button"
                onClick={handleExportResponse}
                className="flex items-center gap-1 text-xs text-gold-400 hover:text-white"
              >
                <FileDown className="h-3 w-3" />
                Save to File
              </button>
            </div>
          </div>
          <pre className="max-h-32 overflow-auto whitespace-pre-wrap rounded-lg bg-purple-900/30 p-3 font-mono text-xs text-purple-300">
            {response.slice(0, 500)}{response.length > 500 ? "..." : ""}
          </pre>
          <p className="text-xs text-purple-400">
            Send this file to the other participants via USB drive, encrypted
            email, or any other secure channel.
          </p>
        </div>
      )}
    </div>
  );
}
