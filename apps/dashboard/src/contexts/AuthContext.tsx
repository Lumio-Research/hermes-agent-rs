import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { apiUrl } from "@/lib/api";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type PlanType = "free" | "cloud_pro";

export interface AuthUser {
  id: string;
  email: string;
  tenantId: string;
}

export interface AuthState {
  user: AuthUser | null;
  plan: PlanType;
  jwt: string | null;
  loading: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string) => Promise<void>;
  loginWithOAuth: (provider: "google" | "github") => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthState>({
  user: null,
  plan: "free",
  jwt: null,
  loading: true,
  login: async () => {},
  register: async () => {},
  loginWithOAuth: async () => {},
  logout: async () => {},
});

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export function AuthProvider({ children }: { children: ReactNode }) {
  const devBypass = import.meta.env.VITE_DEV_BYPASS_AUTH === "1";
  const devToken = import.meta.env.VITE_DEV_BYPASS_TOKEN ?? "dev-local-token";
  const devTenantId = "11111111-1111-1111-1111-111111111111";
  const [user, setUser] = useState<AuthUser | null>(null);
  const [plan, setPlan] = useState<PlanType>("free");
  const [jwt, setJwt] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchPlan = async (token: string) => {
    try {
      const base = (import.meta.env.VITE_API_BASE_URL ?? "").replace(
        /\/$/,
        "",
      );
      const res = await fetch(`${base}/api/v1/tenant/subscription`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setPlan(data.plan === "cloud_pro" ? "cloud_pro" : "free");
      }
    } catch {
      // Backend may not be running yet (self-hosted mode)
    }
  };

  const applyAuth = useCallback(
    async (
      token: string,
      nextUser: { id: string; email: string; tenant_id: string } | null,
    ) => {
      if (!nextUser) {
        setUser(null);
        setJwt(null);
        setPlan("free");
        localStorage.removeItem("hermes_api_token");
        return;
      }
      setUser({
        id: nextUser.id,
        email: nextUser.email,
        tenantId: nextUser.tenant_id,
      });
      setJwt(token);
      localStorage.setItem("hermes_api_token", token);
      await fetchPlan(token);
    },
    [],
  );

  const fetchMe = useCallback(async (token: string) => {
    const res = await fetch(apiUrl("/api/v1/auth/me"), {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) {
      throw new Error(await res.text().catch(() => "auth me failed"));
    }
    const data = (await res.json()) as {
      user: { id: string; email: string; tenant_id: string };
    };
    return data.user;
  }, []);

  // Initialize: check existing session
  useEffect(() => {
    if (devBypass) {
      const local = localStorage.getItem("hermes_dev_email") ?? "dev@local";
      setUser({
        id: "dev-user",
        email: local,
        tenantId: devTenantId,
      });
      setJwt(devToken);
      setPlan("cloud_pro");
      localStorage.setItem("hermes_api_token", devToken);
      setLoading(false);
      return;
    }
    const query = new URLSearchParams(window.location.search);
    const tokenFromQuery = query.get("token");
    if (tokenFromQuery) {
      const url = new URL(window.location.href);
      url.searchParams.delete("token");
      url.searchParams.delete("oauth");
      url.searchParams.delete("state");
      window.history.replaceState({}, "", url.toString());
      fetchMe(tokenFromQuery)
        .then((nextUser) => applyAuth(tokenFromQuery, nextUser))
        .catch(() => {
          localStorage.removeItem("hermes_api_token");
          setUser(null);
          setJwt(null);
          setPlan("free");
        })
        .finally(() => setLoading(false));
      return;
    }

    const token = localStorage.getItem("hermes_api_token");
    if (!token) {
      setLoading(false);
      return;
    }
    fetchMe(token)
      .then((nextUser) => applyAuth(token, nextUser))
      .catch(() => {
        localStorage.removeItem("hermes_api_token");
        setUser(null);
        setJwt(null);
        setPlan("free");
      })
      .finally(() => setLoading(false));
  }, [applyAuth, devBypass, devTenantId, devToken, fetchMe]);

  const login = useCallback(async (email: string, password: string) => {
    if (devBypass) {
      localStorage.setItem("hermes_dev_email", email);
      setUser({ id: "dev-user", email, tenantId: devTenantId });
      setJwt(devToken);
      setPlan("cloud_pro");
      localStorage.setItem("hermes_api_token", devToken);
      return;
    }
    const res = await fetch(apiUrl("/api/v1/auth/login"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        email: email.trim(),
        password,
      }),
    });
    if (!res.ok) {
      throw new Error(await res.text().catch(() => "login failed"));
    }
    const payload = (await res.json()) as {
      access_token: string;
      user: { id: string; email: string; tenant_id: string };
    };
    await applyAuth(payload.access_token, payload.user);
  }, [applyAuth, devBypass, devTenantId, devToken]);

  const register = useCallback(async (email: string, password: string) => {
    if (devBypass) {
      localStorage.setItem("hermes_dev_email", email);
      setUser({ id: "dev-user", email, tenantId: devTenantId });
      setJwt(devToken);
      setPlan("cloud_pro");
      localStorage.setItem("hermes_api_token", devToken);
      return;
    }
    const res = await fetch(apiUrl("/api/v1/auth/register"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        email: email.trim(),
        password,
      }),
    });
    if (!res.ok) {
      throw new Error(await res.text().catch(() => "register failed"));
    }
    const payload = (await res.json()) as {
      access_token: string;
      user: { id: string; email: string; tenant_id: string };
    };
    await applyAuth(payload.access_token, payload.user);
  }, [applyAuth, devBypass, devTenantId, devToken]);

  const loginWithOAuth = useCallback(
    async (provider: "google" | "github") => {
      if (devBypass) {
        const email = provider === "google" ? "dev-google@local" : "dev-github@local";
        localStorage.setItem("hermes_dev_email", email);
        setUser({ id: "dev-user", email, tenantId: devTenantId });
        setJwt(devToken);
        setPlan("cloud_pro");
        localStorage.setItem("hermes_api_token", devToken);
        return;
      }
      const res = await fetch(apiUrl(`/api/v1/auth/oauth/${provider}/start`), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      if (!res.ok) {
        throw new Error(await res.text().catch(() => "oauth start failed"));
      }
      const data = (await res.json()) as { auth_url: string };
      window.location.href = data.auth_url;
    },
    [devBypass, devTenantId, devToken],
  );

  const logout = useCallback(async () => {
    if (devBypass) {
      setUser(null);
      setJwt(null);
      setPlan("free");
      localStorage.removeItem("hermes_api_token");
      return;
    }
    setUser(null);
    setJwt(null);
    setPlan("free");
    localStorage.removeItem("hermes_api_token");
  }, [devBypass]);

  const value = useMemo<AuthState>(
    () => ({ user, plan, jwt, loading, login, register, loginWithOAuth, logout }),
    [user, plan, jwt, loading, login, register, loginWithOAuth, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAuth(): AuthState {
  return useContext(AuthContext);
}
