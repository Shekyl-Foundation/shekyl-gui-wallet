export interface ShardPreviewFixtureInfo {
  id: string;
  label: string;
  dominant_regime: string;
  shard_hash: string;
}

export interface CandidateRecipe {
  fg_tile: string;
  fg_phyllotaxis: string;
  fg_opacity: number;
  fg_tile_palette: string;
  fg_phyllotaxis_palette: string;
  bg_truchet: string;
  bg_crystalline: string;
  bg_opacity: number;
  bg_truchet_palette: string;
  bg_crystalline_palette: string;
  final_mode: string;
  final_opacity: number;
}

export interface RenderShardPreviewRequest {
  fixture_id: string;
  hash_override?: string | null;
  size?: number;
}

export interface RenderShardPreviewResponse {
  png_base64: string;
  recipe: CandidateRecipe;
  cache_key: string;
  label: string;
}
