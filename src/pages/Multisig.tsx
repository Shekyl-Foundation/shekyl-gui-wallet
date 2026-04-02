import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Users,
  AlertCircle,
  CheckCircle2,
  FileDown,
  FileUp,
  PenTool,
  Copy,
} from "lucide-react";

interface MultisigInfo {
  is_multisig: boolean;
  n_total: number;
  m_required: number;
  group_id: string;
}

type Tab = "setup" | "sign" | "import";

export default function Multisig() {
  const [info, setInfo] = useState<MultisigInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<Tab>("setup");

  useEffect(() => {
    loadInfo();
  }, []);

  async function loadInfo() {
    try {
      const result = await invoke<MultisigInfo>("get_multisig_info");
      setInfo(result);
      if (result.is_multisig) setTab("sign");
    } catch {
      setInfo(null);
    } finally {
      setLoading(false);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-gold-400 border-t-transparent" />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold text-white">Multisig</h1>
        {info?.is_multisig && (
          <span className="rounded-full bg-gold-500/15 px-3 py-1 text-xs font-medium text-gold-400">
            {info.m_required}-of-{info.n_total}
          </span>
        )}
      </div>

      {info?.is_multisig && (
        <div className="card flex items-center gap-3 text-sm">
          <CheckCircle2 className="h-4 w-4 shrink-0 text-green-400" />
          <span className="text-purple-200">Group ID:</span>
          <code className="flex-1 truncate font-mono text-xs text-purple-300">
            {info.group_id}
          </code>
          <button
            onClick={() => navigator.clipboard.writeText(info.group_id)}
            className="text-purple-400 hover:text-white"
            title="Copy group ID"
          >
            <Copy className="h-4 w-4" />
          </button>
        </div>
      )}

      <div className="flex gap-1 rounded-lg bg-purple-900/50 p-1">
        {(!info?.is_multisig
          ? [{ id: "setup" as Tab, label: "Setup Group", icon: Users }]
          : [
              { id: "sign" as Tab, label: "Sign Request", icon: PenTool },
              {
                id: "import" as Tab,
                label: "Import & Sign",
                icon: FileUp,
              },
            ]
        ).map(({ id, label, icon: Icon }) => (
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

      {tab === "setup" && <SetupGroup onCreated={loadInfo} />}
      {tab === "sign" && <SignRequest />}
      {tab === "import" && <ImportAndSign />}
    </div>
  );
}

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
        Paste a signing request from the coordinator, review, and sign with your
        key.
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
            Send this response back to the coordinator.
          </p>
        </div>
      )}
    </form>
  );
}

function ImportAndSign() {
  return (
    <div className="card space-y-4">
      <div className="flex items-center gap-3">
        <FileDown className="h-5 w-5 text-purple-400" />
        <div>
          <h3 className="font-medium text-white">Import Signing Request</h3>
          <p className="text-sm text-purple-300">
            Load a signing request file from disk, review the transaction
            details, and produce your signature.
          </p>
        </div>
      </div>
      <p className="rounded-lg bg-purple-800/30 p-3 text-center text-sm text-purple-400">
        File-based import/export coming in a future update. Use the JSON
        paste workflow above for now.
      </p>
    </div>
  );
}
