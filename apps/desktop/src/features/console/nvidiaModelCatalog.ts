// Compatibility exports for the original NVIDIA-only catalog integration.
import { z } from "zod";
import { officialCatalogSchema, officialCatalogBase } from "./officialModelCatalog";
export { applyOfficialCatalog as applyNvidiaCatalog, catalogModels } from "./officialModelCatalog";
export type { CatalogInput as NvidiaCatalogInput } from "./officialModelCatalog";
export const nvidiaCatalogSchema = officialCatalogSchema.extend({ sourceUrl: z.literal("https://integrate.api.nvidia.com/v1/models") });
export const nvidiaCatalogResponseSchema = z.object({ revision: z.string(), catalog: nvidiaCatalogSchema });
export type NvidiaCatalog = z.infer<typeof nvidiaCatalogSchema>;
export type DiscoverNvidiaModels = (input: import("./officialModelCatalog").CatalogInput, signal?: AbortSignal) => Promise<NvidiaCatalog>;
export const isNvidiaCatalogProvider = (provider: Record<string, unknown>) => provider.preset === "nvidia-openai";
export const officialNvidiaBase = (base: string) => officialCatalogBase("nvidia", base);
