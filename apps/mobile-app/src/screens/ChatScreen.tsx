import React, { useEffect, useRef, useState } from "react";
import {
  ActivityIndicator,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from "react-native";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import type { RootStackParamList } from "../../App";
import { listMessages } from "../api";
import { useStreamChat } from "../hooks/useStreamChat";
import type { ChatMessage } from "../types";
import { colors } from "../theme";

type Props = NativeStackScreenProps<RootStackParamList, "Chat">;

export default function ChatScreen({ route, navigation }: Props) {
  const { sessionId, sessionTitle } = route.params;
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(true);
  const { isStreaming, streamingContent, sendStreaming } = useStreamChat();
  const listRef = useRef<FlatList>(null);

  useEffect(() => {
    navigation.setOptions({ title: sessionTitle });
    void listMessages(sessionId)
      .then(setMessages)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [sessionId, sessionTitle, navigation]);

  const scrollToEnd = () => {
    setTimeout(() => listRef.current?.scrollToEnd({ animated: true }), 100);
  };

  const handleSend = async () => {
    const text = input.trim();
    if (!text || isStreaming) return;
    setInput("");

    const userMsg: ChatMessage = {
      id: `u-${Date.now()}`,
      role: "user",
      content: text,
      timestamp: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, userMsg]);
    scrollToEnd();

    try {
      const reply = await sendStreaming(sessionId, text);
      const assistantMsg: ChatMessage = {
        id: `a-${Date.now()}`,
        role: "assistant",
        content: reply,
        timestamp: new Date().toISOString(),
      };
      setMessages((prev) => [...prev, assistantMsg]);
      scrollToEnd();
    } catch {
      // error shown via streamState
    }
  };

  const allMessages = isStreaming
    ? [...messages, { id: "streaming", role: "assistant" as const, content: streamingContent, timestamp: "" }]
    : messages;

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      keyboardVerticalOffset={90}
    >
      {loading ? (
        <ActivityIndicator color={colors.accent} style={{ marginTop: 40 }} />
      ) : (
        <FlatList
          ref={listRef}
          data={allMessages}
          keyExtractor={(item) => item.id}
          contentContainerStyle={styles.messageList}
          onContentSizeChange={scrollToEnd}
          renderItem={({ item }) => (
            <View style={[styles.bubble, item.role === "user" ? styles.userBubble : styles.assistantBubble]}>
              <Text style={item.role === "user" ? styles.userText : styles.assistantText}>
                {item.content || (isStreaming && item.id === "streaming" ? "▌" : "")}
              </Text>
            </View>
          )}
        />
      )}

      <View style={styles.inputRow}>
        <TextInput
          style={styles.input}
          value={input}
          onChangeText={setInput}
          placeholder="Message..."
          placeholderTextColor={colors.textMuted}
          multiline
          editable={!isStreaming}
        />
        <TouchableOpacity
          style={[styles.sendBtn, (!input.trim() || isStreaming) && styles.sendBtnDisabled]}
          onPress={handleSend}
          disabled={!input.trim() || isStreaming}
        >
          {isStreaming ? (
            <ActivityIndicator color="#fff" size="small" />
          ) : (
            <Text style={styles.sendText}>↑</Text>
          )}
        </TouchableOpacity>
      </View>
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: colors.bgPrimary },
  messageList: { paddingHorizontal: 16, paddingVertical: 12, gap: 8 },
  bubble: { maxWidth: "82%", borderRadius: 14, paddingHorizontal: 14, paddingVertical: 10, marginVertical: 4 },
  userBubble: { alignSelf: "flex-end", backgroundColor: colors.accent },
  assistantBubble: { alignSelf: "flex-start", backgroundColor: colors.bgCard },
  userText: { color: "#fff", fontSize: 14, lineHeight: 20 },
  assistantText: { color: colors.textPrimary, fontSize: 14, lineHeight: 20 },
  inputRow: { flexDirection: "row", alignItems: "flex-end", paddingHorizontal: 12, paddingVertical: 10, borderTopWidth: 1, borderTopColor: colors.borderPrimary, backgroundColor: colors.bgSecondary, gap: 8 },
  input: { flex: 1, minHeight: 40, maxHeight: 120, backgroundColor: colors.bgCard, borderRadius: 12, paddingHorizontal: 14, paddingVertical: 10, color: colors.textPrimary, fontSize: 14, borderWidth: 1, borderColor: colors.borderPrimary },
  sendBtn: { width: 40, height: 40, borderRadius: 20, backgroundColor: colors.accent, alignItems: "center", justifyContent: "center" },
  sendBtnDisabled: { backgroundColor: colors.bgActive },
  sendText: { color: "#fff", fontSize: 18, fontWeight: "700" },
});
