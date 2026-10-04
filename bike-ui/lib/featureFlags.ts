import { usePublicFeatureFlags } from "./queries";

export const FLAG_ACTIVITY_LIST_FULL_MAPS = "activity_list_full_maps";
export const FLAG_ENHANCED_MAPS = "enhanced_maps";
export const FLAG_HEATMAPS = "heatmaps";

export function useFeatureFlag(flag: string): boolean {
  const { data: flags = [] } = usePublicFeatureFlags();
  return (
    flags.find((candidate) => candidate.feature_key === flag)?.enabled ?? false
  );
}
