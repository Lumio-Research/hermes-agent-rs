import React, { useEffect, useState } from "react";
import {
  Alert,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from "react-native";
import { configureApi, checkHealth } from "../api";
import { loadConfig, saveConfig } from "../storage";
import type { AppConfig } from "../types";
import { colors } from "../theme";

export default function SettingsScreen() {
  const [config, setConfig] = useState<AppConfig>({ api_base: "http://127.0.0.1:8787", token: "", default_model: "" });
  const [saved, setSaved] = useState(false);
  const [healthStatus, setHealthStatus] = useState<"unknown" | "ok" | "error">("unknown");

  useEffect(() => {
    void loadConfig().then((c) => { setConfig(c); configureApi(c); });
  }, []);

  const handleSave = async () => {
    await saveConfig(config);
    configureApi(config);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleHealthCheck = async () => {
    const ok = await checkHealth();
    setHealthStatus(ok ? "ok" : "error");
    Alert.alert("Health Check", ok ? "✅ Server is reachable" : "❌ Server unreachable");
  };

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.heading}>Settings</Text>

      <View style={styles.section}>
        <Text style={styles.label}>API Base URL</Text>
        <Text style={styles.hint}>e.g. http://127.0.0.1:8787</Text>
        <TextInput
          style={styles.input}
          value={config.api_base}
          onChangeText={(v) => setConfig({ ...config, api_base: v })}
          placeholder="http://127.0.0.1:8787"
          placeholderTextColor={colors.textMuted}
          autoCapitalize="none"
          autoCorrect={false}
          keyboardType="url"
        />
      </View>

      <View style={styles.section}>
        <Text style={styles.label}>Bearer Token</Text>
        <Text style={styles.hint}>Leave empty if server has no auth</Text>
        <TextInput
          style={styles.input}
          value={config.token}
          onChangeText={(v) => setConfig({ ...config, token: v })}
          placeholder="optional"
          placeholderTextColor={colors.textMuted}
          autoCapitalize="none"
          autoCorrect={false}
          secureTextEntry
        />
      </View>

      <View style={styles.section}>
        <Text style={styles.label}>Default Model</Text>
        <TextInput
          style={styles.input}
          value={config.default_model}
          onChangeText={(v) => setConfig({ ...config, default_model: v })}
          placeholder="Leave empty to use server default"
          placeholderTextColor={colors.textMuted}
          autoCapitalize="none"
          autoCorrect={false}
        />
      </View>

      <TouchableOpacity style={styles.saveBtn} onPress={handleSave}>
        <Text style={styles.saveBtnText}>{saved ? "Saved ✓" : "Save Settings"}</Text>
      </TouchableOpacity>

      <TouchableOpacity style={styles.healthBtn} onPress={handleHealthCheck}>
        <Text style={styles.healthBtnText}>
          {healthStatus === "ok" ? "✅ " : healthStatus === "error" ? "❌ " : ""}Test Connection
        </Text>
      </TouchableOpacity>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: colors.bgPrimary },
  content: { padding: 20, gap: 8 },
  heading: { fontSize: 22, fontWeight: "700", color: colors.textPrimary, marginBottom: 16 },
  section: { marginBottom: 20 },
  label: { fontSize: 14, fontWeight: "600", color: colors.textPrimary, marginBottom: 4 },
  hint: { fontSize: 12, color: colors.textMuted, marginBottom: 8 },
  input: { backgroundColor: colors.bgCard, borderWidth: 1, borderColor: colors.borderPrimary, borderRadius: 10, paddingHorizontal: 14, paddingVertical: 12, color: colors.textPrimary, fontSize: 14 },
  saveBtn: { backgroundColor: colors.accent, borderRadius: 10, paddingVertical: 13, alignItems: "center", marginTop: 8 },
  saveBtnText: { color: "#fff", fontWeight: "700", fontSize: 15 },
  healthBtn: { backgroundColor: colors.bgCard, borderWidth: 1, borderColor: colors.borderPrimary, borderRadius: 10, paddingVertical: 13, alignItems: "center", marginTop: 12 },
  healthBtnText: { color: colors.textSecondary, fontSize: 14 },
});
