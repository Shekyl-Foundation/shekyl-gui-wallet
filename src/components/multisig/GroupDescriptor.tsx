import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, Upload, CheckCircle2, AlertCircle, FileText } from "lucide-react";

interface GroupDescriptorPayload {
  version: number;
  group_id: string;
  m_required: number;
  n_total: number;
  spend_auth_version: number;
  participant_pubkeys: string[];
  address_fingerprint: string;
  relays: { url: string; operator_id: string }[];
  created_at: number;
  notes: string | null;
}

interface Props {
  groupId: string;
  mRequired: number;
  nTotal: number;
}

export default function GroupDescriptor({ groupId }: Props) {
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [importedDesc, setImportedDesc] = useState<GroupDescriptorPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  async function handleExport() {
    setError(null);
    setSuccess(null);
    try {
      const path = await save({
        defaultPath: `shekyl-group-${groupId.slice(0, 8)}.json`,
        filters: [{ name: "Group Descriptor", extensions: ["json"] }],
      });
      if (!path) return;

      setExporting(true);
      await invoke("export_group_descriptor", { path });
      setSuccess("Group descriptor exported successfully.");
    } catch (err) {
      setError(String(err));
    } finally {
      setExporting(false);
    }
  }

  async function handleImport() {
    setError(null);
    setSuccess(null);
    setImportedDesc(null);
    try {
      const path = await open({
        filters: [{ name: "Group Descriptor", extensions: ["json"] }],
        multiple: false,
      });
      if (!path) return;

      setImporting(true);
      const desc = await invoke<GroupDescriptorPayload>("import_group_descriptor", {
        path,
      });
      setImportedDesc(desc);
      setSuccess("Group descriptor loaded. Review details below.");
    } catch (err) {
      setError(String(err));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <FileText className="h-4 w-4 text-purple-400" />
        <h3 className="text-sm font-medium text-white">Group Descriptor</h3>
      </div>

      <p className="text-xs text-purple-300">
        A group descriptor contains everything needed to restore this multisig
        group from seeds — participant keys, threshold, relay config, and
        fingerprint. Export for backup; import to join or restore a group.
      </p>

      <div className="flex gap-3">
        <button
          onClick={handleExport}
          disabled={exporting || !groupId}
          className="btn btn-secondary flex-1"
          title={!groupId ? "Create a group first" : undefined}
        >
          <Download className="h-4 w-4" />
          {exporting ? "Exporting..." : "Export Descriptor"}
        </button>
        <button
          onClick={handleImport}
          disabled={importing}
          className="btn btn-secondary flex-1"
        >
          <Upload className="h-4 w-4" />
          {importing ? "Importing..." : "Import Descriptor"}
        </button>
      </div>

      {error && (
        <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {success && (
        <div className="flex items-start gap-2 rounded-lg bg-green-500/10 p-3 text-sm text-green-300">
          <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0" />
          {success}
        </div>
      )}

      {importedDesc && (
        <div className="space-y-3 rounded-lg bg-purple-900/30 p-4">
          <h4 className="text-sm font-medium text-gold-400">Imported Group Details</h4>
          <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
            <dt className="text-purple-400">Group ID</dt>
            <dd className="truncate font-mono text-xs text-purple-200">
              {importedDesc.group_id}
            </dd>
            <dt className="text-purple-400">Threshold</dt>
            <dd className="text-purple-200">
              {importedDesc.m_required}-of-{importedDesc.n_total}
            </dd>
            <dt className="text-purple-400">Fingerprint</dt>
            <dd className="truncate font-mono text-xs text-purple-200">
              {importedDesc.address_fingerprint}
            </dd>
            <dt className="text-purple-400">Participants</dt>
            <dd className="text-purple-200">{importedDesc.participant_pubkeys.length}</dd>
            <dt className="text-purple-400">Relays</dt>
            <dd className="text-purple-200">{importedDesc.relays.length || "None"}</dd>
          </dl>

          {groupId && importedDesc.group_id !== groupId && (
            <div className="flex items-start gap-2 rounded-lg bg-yellow-500/10 p-3 text-sm text-yellow-300">
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
              This descriptor is for a different group than your current wallet.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
