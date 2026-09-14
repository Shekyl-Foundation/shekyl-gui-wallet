import ShardCoverageGallery from "../components/shards/ShardCoverageGallery";

/**
 * Operator gallery: profit-ordered coverage from the local daemon.
 * Fetch, load state, and fail-closed render live in the panel.
 */
export default function Shards() {
  return (
    <div className="mx-auto max-w-5xl">
      <ShardCoverageGallery />
    </div>
  );
}
