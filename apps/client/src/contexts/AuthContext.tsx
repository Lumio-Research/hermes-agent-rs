import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  authLogin,
  authMe,
  authOAuthStart,
  authRegister,
  type AuthUserDto,
} from "../api";

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

const LS_TOKEN_KEY = "hermes_api_token";

export function AuthProvider({ children }: { children: ReactNode }) {
  const devBypass = import.meta.env.VITE_DEV_BYPASS_AUTH === "1";
  const devToken = import.meta.env.VITE_DEV_BYPASS_TOKEN ?? "dev-local-token";
  const devTenantId = "11111111-1111-1111-1111-111111111111";
  const [user, setUser] = useState<AuthUser | null>(null);
  const [plan, setPlan] = useState<PlanType>("free");
  const [jwt, setJwt] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const applyAuth = useCallback(
    (token: string, nextUser: AuthUserDto | null) => {
      if (!nextUser) {
        setUser(null);
        setJwt(null);
        setPlan("free");
        localStorage.removeItem(LS_TOKEN_KEY);
        return;
      }
      setUser({
        id: nextUser.id,
        email: nextUser.email,
        tenantId: nextUser.tenant_id,
      });
      setJwt(token);
      setPlan("cloud_pro");
      localStorage.setItem(LS_TOKEN_KEY, token);
    },
    [],
  );

  useEffect(() => {
    if (devBypass) {
      const email = localStorage.getItem("hermes_dev_email") ?? "dev@local";
      setUser({ id: "dev-user", email, tenantId: devTenantId });
      setJwt(devToken);
      setPlan("cloud_pro");
      localStorage.setItem(LS_TOKEN_KEY, devToken);
      setLoading(false);
      return;
    }

    const query = new URLSearchParams(window.location.search);
    const tokenFromQuery = query.get("token");
    const token = tokenFromQuery ?? localStorage.getItem(LS_TOKEN_KEY);
    if (!token) {
      setLoading(false);
      return;
    }
    authMe(token)
      .then((res) => {
        applyAuth(token, res.user);
      })
      .catch(() => {
        applyAuth("", null);
      })
      .finally(() => {
        if (tokenFromQuery) {
          const url = new URL(window.location.href);
          url.searchParams.delete("token");
          url.searchParams.delete("oauth");
          url.searchParams.delete("state");
          window.history.replaceState({}, "", url.toString());
        }
        setLoading(false);
      });
  }, [applyAuth, devBypass, devTenantId, devToken]);

  const login = useCallback(
    async (email: string, password: string) => {
      if (devBypass) {
        localStorage.setItem("hermes_dev_email", email);
        setUser({ id: "dev-user", email, tenantId: devTenantId });
        setJwt(devToken);
        setPlan("cloud_pro");
        localStorage.setItem(LS_TOKEN_KEY, devToken);
        return;
      }
      const payload = await authLogin(email.trim(), password);
      applyAuth(payload.access_token, payload.user);
    },
    [applyAuth, devBypass, devTenantId, devToken],
  );

  const register = useCallback(
    async (email: string, password: string) => {
      if (devBypass) {
        localStorage.setItem("hermes_dev_email", email);
        setUser({ id: "dev-user", email, tenantId: devTenantId });
        setJwt(devToken);
        setPlan("cloud_pro");
        localStorage.setItem(LS_TOKEN_KEY, devToken);
        return;
      }
      const payload = await authRegister(email.trim(), password);
      applyAuth(payload.access_token, payload.user);
    },
    [applyAuth, devBypass, devTenantId, devToken],
  );

  const loginWithOAuth = useCallback(
    async (provider: "google" | "github") => {
      if (devBypass) {
        const email = provider === "google" ? "dev-google@local" : "dev-github@local";
        localStorage.setItem("hermes_dev_email", email);
        setUser({ id: "dev-user", email, tenantId: devTenantId });
        setJwt(devToken);
        setPlan("cloud_pro");
        localStorage.setItem(LS_TOKEN_KEY, devToken);
        return;
      }
      const data = await authOAuthStart(provider);
      window.location.href = data.auth_url;
    },
    [devBypass, devTenantId, devToken],
  );

  const logout = useCallback(async () => {
    setUser(null);
    setJwt(null);
    setPlan("free");
    localStorage.removeItem(LS_TOKEN_KEY);
  }, []);

  const value = useMemo<AuthState>(
    () => ({ user, plan, jwt, loading, login, register, loginWithOAuth, logout }),
    [user, plan, jwt, loading, login, register, loginWithOAuth, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  return useContext(AuthContext);
}

