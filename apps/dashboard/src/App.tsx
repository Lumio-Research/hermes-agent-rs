import { useMemo } from "react";
import { Routes, Route, NavLink, Navigate } from "react-router-dom";
import {
  Bot,
  Activity,
  BarChart3,
  Clock,
  FileText,
  KeyRound,
  MessageSquare,
  Package,
  Settings,
  Puzzle,
  Sparkles,
  Terminal,
  Globe,
  Database,
  Shield,
  Wrench,
  Zap,
  Heart,
  Star,
  Code,
  Eye,
} from "lucide-react";
import { Cell, Grid, SelectionSwitcher, Typography } from "@/components/ds";
import { cn } from "@/lib/utils";
import { Backdrop } from "@/components/Backdrop";
import StatusPage from "@/pages/StatusPage";
import ConfigPage from "@/pages/ConfigPage";
import EnvPage from "@/pages/EnvPage";
import SessionsPage from "@/pages/SessionsPage";
import LogsPage from "@/pages/LogsPage";
import AnalyticsPage from "@/pages/AnalyticsPage";
import CronPage from "@/pages/CronPage";
import SkillsPage from "@/pages/SkillsPage";
import LandingPage from "@/pages/LandingPage";
import LoginPage from "@/pages/LoginPage";
import RegisterPage from "@/pages/RegisterPage";
import SubscriptionPage from "@/pages/SubscriptionPage";
import ApiKeysPage from "@/pages/ApiKeysPage";
import PlatformConnectionsPage from "@/pages/PlatformConnectionsPage";
import { ProtectedRoute } from "@/components/ProtectedRoute";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";
import { ThemeSwitcher } from "@/components/ThemeSwitcher";
import { useI18n } from "@/i18n";
import { usePlugins } from "@/plugins";
import type { RegisteredPlugin } from "@/plugins";

const BUILTIN_NAV: NavItem[] = [
  { path: "/dashboard", labelKey: "status", label: "Status", icon: Activity },
  {
    path: "/dashboard/sessions",
    labelKey: "sessions",
    label: "Sessions",
    icon: MessageSquare,
  },
  {
    path: "/dashboard/analytics",
    labelKey: "analytics",
    label: "Analytics",
    icon: BarChart3,
  },
  { path: "/dashboard/logs", labelKey: "logs", label: "Logs", icon: FileText },
  { path: "/dashboard/cron", labelKey: "cron", label: "Cron", icon: Clock },
  { path: "/dashboard/skills", labelKey: "skills", label: "Skills", icon: Package },
  { path: "/dashboard/config", labelKey: "config", label: "Config", icon: Settings },
  { path: "/dashboard/env", labelKey: "keys", label: "Keys", icon: KeyRound },
];

// Plugins can reference any of these by name in their manifest — keeps bundle
// size sane vs. importing the full lucide-react set.
const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  Bot,
  Activity,
  BarChart3,
  Clock,
  FileText,
  KeyRound,
  MessageSquare,
  Package,
  Settings,
  Puzzle,
  Sparkles,
  Terminal,
  Globe,
  Database,
  Shield,
  Wrench,
  Zap,
  Heart,
  Star,
  Code,
  Eye,
};

function resolveIcon(
  name: string,
): React.ComponentType<{ className?: string }> {
  return ICON_MAP[name] ?? Puzzle;
}

function buildNavItems(
  builtIn: NavItem[],
  plugins: RegisteredPlugin[],
): NavItem[] {
  const items = [...builtIn];

  for (const { manifest } of plugins) {
    const pluginItem: NavItem = {
      path: manifest.tab.path,
      label: manifest.label,
      icon: resolveIcon(manifest.icon),
    };

    const pos = manifest.tab.position ?? "end";
    if (pos === "end") {
      items.push(pluginItem);
    } else if (pos.startsWith("after:")) {
      const target = "/" + pos.slice(6);
      const idx = items.findIndex((i) => i.path === target);
      items.splice(idx >= 0 ? idx + 1 : items.length, 0, pluginItem);
    } else if (pos.startsWith("before:")) {
      const target = "/" + pos.slice(7);
      const idx = items.findIndex((i) => i.path === target);
      items.splice(idx >= 0 ? idx : items.length, 0, pluginItem);
    } else {
      items.push(pluginItem);
    }
  }

  return items;
}

export default function App() {
  const { t } = useI18n();
  const { plugins } = usePlugins();

  const navItems = useMemo(
    () => buildNavItems(BUILTIN_NAV, plugins),
    [plugins],
  );

  return (
    <div className="text-midground font-mondwest bg-black min-h-screen flex flex-col uppercase antialiased overflow-x-hidden">
      <SelectionSwitcher />
      <Backdrop />

      <header
        className={cn(
          "fixed top-0 left-0 right-0 z-40",
          "border-b border-current/20",
          "bg-background-base/90 backdrop-blur-sm",
        )}
      >
        <div className="mx-auto flex h-12 max-w-[1600px]">
          <div className="min-w-0 flex-1 overflow-x-auto scrollbar-none">
            <Grid
              className="h-full !border-t-0 !border-b-0"
              style={{
                gridTemplateColumns: `auto repeat(${navItems.length}, auto)`,
              }}
            >
              <Cell className="flex items-center !p-0 !px-3 sm:!px-5">
                <Typography
                  className="font-bold text-[1.0625rem] sm:text-[1.125rem] leading-[0.95] tracking-[0.0525rem] text-midground"
                  style={{ mixBlendMode: "plus-lighter" }}
                >
                  Hermes
                  <br />
                  Agent
                </Typography>
              </Cell>

              {navItems.map(({ path, label, labelKey, icon: Icon }) => (
                <Cell key={path} className="relative !p-0">
                  <NavLink
                    to={path}
                    end={path === "/dashboard"}
                    className={({ isActive }) =>
                      cn(
                        "group relative flex h-full w-full items-center gap-1.5",
                        "px-2.5 sm:px-4 py-2",
                        "font-mondwest text-[0.65rem] sm:text-[0.8rem] tracking-[0.12em]",
                        "whitespace-nowrap transition-colors cursor-pointer",
                        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-midground",
                        isActive
                          ? "text-midground"
                          : "opacity-75 hover:opacity-100",
                      )
                    }
                  >
                    {({ isActive }) => (
                      <>
                        <Icon className="h-3.5 w-3.5 shrink-0" />
                        <span className="hidden sm:inline">
                          {labelKey
                            ? ((t.app.nav as Record<string, string>)[
                                labelKey
                              ] ?? label)
                            : label}
                        </span>

                        <span
                          aria-hidden
                          className="absolute inset-1 bg-midground opacity-0 pointer-events-none transition-opacity duration-200 group-hover:opacity-5"
                        />

                        {isActive && (
                          <span
                            aria-hidden
                            className="absolute bottom-0 left-0 right-0 h-px bg-midground"
                            style={{ mixBlendMode: "plus-lighter" }}
                          />
                        )}
                      </>
                    )}
                  </NavLink>
                </Cell>
              ))}
            </Grid>
          </div>

          <Grid className="h-full shrink-0 !border-t-0 !border-b-0">
            <Cell className="flex items-center gap-2 !p-0 !px-2 sm:!px-4">
              <ThemeSwitcher />
              <LanguageSwitcher />
              <Typography
                mondwest
                className="hidden sm:inline text-[0.7rem] tracking-[0.15em] opacity-50"
              >
                {t.app.webUi}
              </Typography>
            </Cell>
          </Grid>
        </div>
      </header>

      <main className="relative z-2 mx-auto w-full max-w-[1600px] flex-1 px-3 sm:px-6 pt-16 sm:pt-20 pb-4 sm:pb-8">
        <Routes>
          {/* Public routes */}
          <Route path="/" element={<LandingPage />} />
          <Route path="/login" element={<LoginPage />} />
          <Route path="/register" element={<RegisterPage />} />

          {/* Protected dashboard routes */}
          <Route path="/dashboard" element={<ProtectedRoute><StatusPage /></ProtectedRoute>} />
          <Route path="/dashboard/sessions" element={<ProtectedRoute><SessionsPage /></ProtectedRoute>} />
          <Route path="/dashboard/analytics" element={<ProtectedRoute><AnalyticsPage /></ProtectedRoute>} />
          <Route path="/dashboard/logs" element={<ProtectedRoute><LogsPage /></ProtectedRoute>} />
          <Route path="/dashboard/cron" element={<ProtectedRoute><CronPage /></ProtectedRoute>} />
          <Route path="/dashboard/skills" element={<ProtectedRoute><SkillsPage /></ProtectedRoute>} />
          <Route path="/dashboard/config" element={<ProtectedRoute><ConfigPage /></ProtectedRoute>} />
          <Route path="/dashboard/env" element={<ProtectedRoute><EnvPage /></ProtectedRoute>} />
          <Route path="/dashboard/subscription" element={<ProtectedRoute><SubscriptionPage /></ProtectedRoute>} />
          <Route path="/dashboard/keys" element={<ProtectedRoute><ApiKeysPage /></ProtectedRoute>} />
          <Route path="/dashboard/platforms" element={<ProtectedRoute><PlatformConnectionsPage /></ProtectedRoute>} />

          {plugins.map(({ manifest, component: PluginComponent }) => (
            <Route
              key={manifest.name}
              path={`/dashboard${manifest.tab.path}`}
              element={<ProtectedRoute><PluginComponent /></ProtectedRoute>}
            />
          ))}

          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>

      <footer className="relative z-2 border-t border-current/20">
        <Grid className="mx-auto max-w-[1600px] !border-t-0 !border-b-0">
          <Cell className="flex items-center !px-3 sm:!px-6 !py-3">
            <Typography
              mondwest
              className="text-[0.7rem] sm:text-[0.8rem] tracking-[0.12em] opacity-75"
            >
              {t.app.footer.name}
            </Typography>
          </Cell>
          <Cell className="flex items-center justify-end !px-3 sm:!px-6 !py-3">
            <Typography
              mondwest
              className="text-[0.6rem] sm:text-[0.7rem] tracking-[0.15em] text-midground"
              style={{ mixBlendMode: "plus-lighter" }}
            >
              {t.app.footer.org}
            </Typography>
          </Cell>
        </Grid>
      </footer>
    </div>
  );
}

interface NavItem {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  labelKey?: string;
  path: string;
}
