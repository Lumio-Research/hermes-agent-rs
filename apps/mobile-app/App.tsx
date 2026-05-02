import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { StatusBar } from "expo-status-bar";
import { useEffect } from "react";
import HomeScreen from "./src/screens/HomeScreen";
import ChatScreen from "./src/screens/ChatScreen";
import SettingsScreen from "./src/screens/SettingsScreen";
import { loadConfig } from "./src/storage";
import { configureApi } from "./src/api";
import { colors } from "./src/theme";

export type RootStackParamList = {
  Home: undefined;
  Chat: { sessionId: string; sessionTitle: string };
  Settings: undefined;
};

const Stack = createNativeStackNavigator<RootStackParamList>();

export default function App() {
  useEffect(() => {
    void loadConfig().then(configureApi);
  }, []);

  return (
    <SafeAreaProvider>
      <StatusBar style="light" />
      <NavigationContainer>
        <Stack.Navigator
          screenOptions={{
            headerStyle: { backgroundColor: colors.bgSecondary },
            headerTintColor: colors.textPrimary,
            headerTitleStyle: { color: colors.textPrimary, fontWeight: "700" },
            contentStyle: { backgroundColor: colors.bgPrimary },
            headerShadowVisible: false,
          }}
        >
          <Stack.Screen name="Home" component={HomeScreen} options={{ headerShown: false }} />
          <Stack.Screen name="Chat" component={ChatScreen} />
          <Stack.Screen name="Settings" component={SettingsScreen} options={{ title: "Settings" }} />
        </Stack.Navigator>
      </NavigationContainer>
    </SafeAreaProvider>
  );
}
