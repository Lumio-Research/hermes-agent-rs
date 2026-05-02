import AsyncStorage from "@react-native-async-storage/async-storage";
import type { AppConfig } from "./types";

const CONFIG_KEY = "hermes.mobile.config.v1";

export const DEFAULT_CONFIG: AppConfig = {
  api_base: "http://127.0.0.1:8787",
  token: "",
  default_model: "",
};

export async function loadConfig(): Promise<AppConfig> {
  try {
    const raw = await AsyncStorage.getItem(CONFIG_KEY);
    if (!raw) return { ...DEFAULT_CONFIG };
    const parsed = JSON.parse(raw) as Partial<AppConfig>;
    return {
      api_base: parsed.api_base || DEFAULT_CONFIG.api_base,
      token: parsed.token || "",
      default_model: parsed.default_model || "",
    };
  } catch {
    return { ...DEFAULT_CONFIG };
  }
}

export async function saveConfig(config: AppConfig): Promise<void> {
  await AsyncStorage.setItem(CONFIG_KEY, JSON.stringify(config));
}
