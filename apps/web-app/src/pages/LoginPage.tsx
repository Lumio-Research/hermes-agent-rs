import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useAuth } from "../contexts/AuthContext";

export default function LoginPage() {
  const { login, loginWithOAuth } = useAuth();
  const navigate = useNavigate();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      await login(email, password);
      navigate("/");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-bg-primary text-text-primary flex items-center justify-center px-4">
      <div className="w-full max-w-sm rounded-2xl border border-border-secondary bg-bg-card p-6 space-y-4">
        <h1 className="text-xl font-semibold">Hermes Client Login</h1>
        {error && <div className="text-sm text-red-400">{error}</div>}
        <form className="space-y-3" onSubmit={onSubmit}>
          <input
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Email"
            type="email"
            className="w-full rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
            required
          />
          <input
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Password"
            type="password"
            className="w-full rounded-md border border-border-primary bg-bg-tertiary px-3 py-2 text-sm"
            required
          />
          <button
            disabled={loading}
            className="w-full rounded-md bg-accent px-3 py-2 text-sm text-white disabled:opacity-60"
            type="submit"
          >
            {loading ? "Signing in..." : "Sign in"}
          </button>
        </form>
        <div className="grid grid-cols-2 gap-2">
          <button
            onClick={() => loginWithOAuth("google").catch((e) => setError(String(e)))}
            className="rounded-md border border-border-primary px-3 py-2 text-sm"
          >
            Google
          </button>
          <button
            onClick={() => loginWithOAuth("github").catch((e) => setError(String(e)))}
            className="rounded-md border border-border-primary px-3 py-2 text-sm"
          >
            GitHub
          </button>
        </div>
        <p className="text-sm text-text-muted">
          No account? <Link className="underline" to="/register">Register</Link>
        </p>
      </div>
    </div>
  );
}

