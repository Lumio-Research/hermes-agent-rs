import React, { useCallback, useEffect, useState } from "react";
import {
  ActivityIndicator,
  FlatList,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";
import type { NativeStackNavigationProp } from "@react-navigation/native-stack";
import { useFocusEffect } from "@react-navigation/native";
import type { RootStackParamList } from "../../App";
import { createSession, deleteSession, listSessions } from "../api";
import type { Session } from "../types";
import { colors } from "../theme";

type Props = { navigation: NativeStackNavigationProp<RootStackParamList, "Home"> };

export default function HomeScreen({ navigation }: Props) {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listSessions();
      setSessions(data);
    } catch {
      setSessions([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useFocusEffect(useCallback(() => { void load(); }, [load]));

  const handleNew = async () => {
    const session = await createSession("New chat");
    navigation.navigate("Chat", { sessionId: session.id, sessionTitle: session.title });
  };

  const handleDelete = async (id: string) => {
    try { await deleteSession(id); } catch { /* ignore */ }
    setSessions((prev) => prev.filter((s) => s.id !== id));
  };

  const formatTime = (ts: string) => {
    const diff = Date.now() - new Date(ts).getTime();
    const h = Math.floor(diff / 3600000);
    const d = Math.floor(diff / 86400000);
    if (h < 1) return "now";
    if (h < 24) return `${h}h`;
    return `${d}d`;
  };

  return (
    <View style={styles.container}>
      {/* Header */}
      <View style={styles.header}>
        <Text style={styles.headerTitle}>Hermes</Text>
        <View style={styles.headerActions}>
          <TouchableOpacity onPress={() => navigation.navigate("Settings")} style={styles.iconBtn}>
            <Text style={styles.iconText}>⚙</Text>
          </TouchableOpacity>
          <TouchableOpacity onPress={handleNew} style={[styles.iconBtn, styles.newBtn]}>
            <Text style={styles.newBtnText}>+ New chat</Text>
          </TouchableOpacity>
        </View>
      </View>

      {loading ? (
        <ActivityIndicator color={colors.accent} style={{ marginTop: 40 }} />
      ) : sessions.length === 0 ? (
        <View style={styles.empty}>
          <Text style={styles.emptyText}>No conversations yet</Text>
          <TouchableOpacity onPress={handleNew} style={styles.emptyBtn}>
            <Text style={styles.emptyBtnText}>Start a new chat</Text>
          </TouchableOpacity>
        </View>
      ) : (
        <FlatList
          data={sessions}
          keyExtractor={(item) => item.id}
          contentContainerStyle={{ paddingBottom: 24 }}
          renderItem={({ item }) => (
            <TouchableOpacity
              style={styles.sessionRow}
              onPress={() => navigation.navigate("Chat", { sessionId: item.id, sessionTitle: item.title })}
            >
              <View style={styles.sessionInfo}>
                <Text style={styles.sessionTitle} numberOfLines={1}>{item.title}</Text>
                <Text style={styles.sessionTime}>{formatTime(item.updated_at)}</Text>
              </View>
              <TouchableOpacity onPress={() => handleDelete(item.id)} style={styles.deleteBtn}>
                <Text style={styles.deleteText}>✕</Text>
              </TouchableOpacity>
            </TouchableOpacity>
          )}
        />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: colors.bgPrimary },
  header: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", paddingHorizontal: 16, paddingVertical: 12, backgroundColor: colors.bgSecondary, borderBottomWidth: 1, borderBottomColor: colors.borderPrimary },
  headerTitle: { fontSize: 18, fontWeight: "700", color: colors.textPrimary },
  headerActions: { flexDirection: "row", alignItems: "center", gap: 8 },
  iconBtn: { padding: 8 },
  iconText: { fontSize: 18, color: colors.textSecondary },
  newBtn: { backgroundColor: colors.accent, borderRadius: 8, paddingHorizontal: 12, paddingVertical: 6 },
  newBtnText: { color: "#fff", fontWeight: "600", fontSize: 13 },
  sessionRow: { flexDirection: "row", alignItems: "center", paddingHorizontal: 16, paddingVertical: 14, borderBottomWidth: 1, borderBottomColor: colors.borderPrimary },
  sessionInfo: { flex: 1 },
  sessionTitle: { fontSize: 14, color: colors.textPrimary, marginBottom: 2 },
  sessionTime: { fontSize: 12, color: colors.textMuted },
  deleteBtn: { padding: 8 },
  deleteText: { color: colors.textMuted, fontSize: 14 },
  empty: { flex: 1, alignItems: "center", justifyContent: "center", gap: 16 },
  emptyText: { color: colors.textMuted, fontSize: 15 },
  emptyBtn: { backgroundColor: colors.accent, borderRadius: 10, paddingHorizontal: 20, paddingVertical: 10 },
  emptyBtnText: { color: "#fff", fontWeight: "600" },
});
