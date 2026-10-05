import { useContext } from "react";
import { ShardPickerContext } from "./shardPickerState";

export function useShardPicker() {
  return useContext(ShardPickerContext);
}
