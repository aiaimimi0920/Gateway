import { defineDeclarativeProvider } from "./declarativeProviderSpec";

export const zenmux = defineDeclarativeProvider({
  id: "zenmux-api", preset: "zenmux-api", vendorKey: "zenmux", vendorName: "ZenMux",
  category: "aggregator", defaultBaseUrl: "https://zenmux.ai/api/v1", modelsPath: "models",
  descriptionZh: "ZenMux 官方 API Key；从目录选择模型，通过 Chat Completions 调用。工具支持取决于模型。",
  descriptionEn: "ZenMux API Key with Chat Completions. Select models from the directory; tool support depends on the model.",
});
