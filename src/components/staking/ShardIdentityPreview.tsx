import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, ImageIcon, RefreshCw } from "lucide-react";
import type {
  RenderShardPreviewResponse,
  ShardPreviewFixtureInfo,
} from "../../types/shardPreview";

const PREVIEW_SIZE = 192;
const HEX_HASH_RE = /^[0-9a-fA-F]{64}$/;

export default function ShardIdentityPreview() {
  const [fixtures, setFixtures] = useState<ShardPreviewFixtureInfo[]>([]);
  const [fixtureId, setFixtureId] = useState<string>("");
  const [hashOverride, setHashOverride] = useState("");
  const [preview, setPreview] = useState<RenderShardPreviewResponse | null>(
    null,
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ShardPreviewFixtureInfo[]>("list_shard_preview_fixtures")
      .then((list) => {
        setFixtures(list);
        if (list.length > 0) {
          setFixtureId(list[0].id);
        }
      })
      .catch(() => {
        setError("Could not load shard preview fixtures.");
      });
  }, []);

  const hashError = useMemo(() => {
    const trimmed = hashOverride.trim();
    if (!trimmed) return null;
    if (!HEX_HASH_RE.test(trimmed)) {
      return "Hash must be exactly 64 hexadecimal characters.";
    }
    return null;
  }, [hashOverride]);

  const renderPreview = useCallback(async () => {
    if (!fixtureId || hashError) return;
    setLoading(true);
    setError(null);
    try {
      const response = await invoke<RenderShardPreviewResponse>(
        "render_shard_preview",
        {
          request: {
            fixture_id: fixtureId,
            hash_override: hashOverride.trim() || null,
            size: PREVIEW_SIZE,
          },
        },
      );
      setPreview(response);
    } catch (e) {
      setPreview(null);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [fixtureId, hashError, hashOverride]);

  useEffect(() => {
    if (!fixtureId || hashError) return;
    const timer = window.setTimeout(() => {
      void renderPreview();
    }, 350);
    return () => window.clearTimeout(timer);
  }, [fixtureId, hashError, hashOverride, renderPreview]);

  const pngSrc = preview
    ? `data:image/png;base64,${preview.png_base64}`
    : null;

  return (
    <div className="card space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <ImageIcon className="h-4 w-4 text-gold-400" />
            <h2 className="text-sm font-semibold text-purple-200">
              Shard Identity Preview
            </h2>
          </div>
          <p className="mt-1 text-xs leading-relaxed text-purple-300">
            Beta visualization of how archived shard identities will look once
            archival staking ships. Fixtures use representative chain regimes;
            optional hash overrides explore palette and compositor variation.
          </p>
        </div>
        <span className="shrink-0 rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-amber-200">
          Beta
        </span>
      </div>

      <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-3 text-xs text-amber-100/90">
        <div className="flex items-start gap-2">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-300" />
          <p>
            Pre-archival preview only. These thumbnails are not tied to live
            wallet shards yet; production will render from real{" "}
            <code className="text-amber-100">content_hash</code> once Stage 5
            archival lands.
          </p>
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_192px]">
        <div className="space-y-3">
          <label className="block space-y-1">
            <span className="text-xs font-medium text-purple-200">Regime fixture</span>
            <select
              className="input"
              value={fixtureId}
              onChange={(e) => setFixtureId(e.target.value)}
            >
              {fixtures.map((f) => (
                <option key={f.id} value={f.id}>
                  {f.label} ({f.dominant_regime})
                </option>
              ))}
            </select>
          </label>

          <label className="block space-y-1">
            <span className="text-xs font-medium text-purple-200">
              Optional shard hash override (64 hex)
            </span>
            <input
              className="input font-mono text-xs"
              placeholder="Leave empty to use fixture hash"
              value={hashOverride}
              onChange={(e) => setHashOverride(e.target.value)}
              spellCheck={false}
            />
            {hashError && (
              <p className="text-xs text-red-300">{hashError}</p>
            )}
          </label>

          {preview?.recipe && (
            <div className="rounded-lg border border-purple-600/30 bg-purple-900/20 p-3 text-[11px] text-purple-200">
              <p className="font-semibold text-purple-100">
                Recipe (candidate.v1)
                {preview.recipe.canonical === false && (
                  <span className="ml-2 rounded bg-amber-500/20 px-1.5 py-0.5 font-semibold text-amber-300">
                    NON-CANONICAL — viewer-chosen, not chain state
                  </span>
                )}
              </p>
              <p className="mt-1 text-purple-300">
                FG: {preview.recipe.fg_tile} + {preview.recipe.fg_phyllotaxis} @{" "}
                {preview.recipe.fg_opacity.toFixed(2)}
              </p>
              <p className="text-purple-300">
                BG: {preview.recipe.bg_truchet} + {preview.recipe.bg_crystalline} @{" "}
                {preview.recipe.bg_opacity.toFixed(2)}
              </p>
              <p className="text-purple-300">
                Final {preview.recipe.final_mode} @{" "}
                {preview.recipe.final_opacity.toFixed(2)}
              </p>
            </div>
          )}

          {error && (
            <div className="flex items-center justify-between gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
              <span>{error}</span>
              <button
                type="button"
                className="btn btn-secondary px-2 py-1 text-[10px]"
                onClick={() => void renderPreview()}
              >
                Retry
              </button>
            </div>
          )}
        </div>

        <div className="flex flex-col items-center gap-2">
          <div className="relative flex h-48 w-48 items-center justify-center overflow-hidden rounded-xl border border-purple-600/40 bg-purple-950/60">
            {loading && (
              <RefreshCw className="h-6 w-6 animate-spin text-purple-300" />
            )}
            {!loading && pngSrc && (
              <img
                src={pngSrc}
                alt={preview?.label ?? "Shard preview"}
                className="h-full w-full object-cover"
                width={PREVIEW_SIZE}
                height={PREVIEW_SIZE}
              />
            )}
            {!loading && !pngSrc && !error && (
              <span className="px-2 text-center text-[11px] text-purple-400">
                Select a fixture to render
              </span>
            )}
          </div>
          {preview && (
            <p className="text-center text-[10px] text-purple-400">
              {preview.label}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
