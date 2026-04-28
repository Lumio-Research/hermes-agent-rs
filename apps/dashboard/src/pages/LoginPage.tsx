import { useState } from "react";
import { useNavigate, Link } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";

export default function LoginPage() {
  const { login, loginWithOAuth } = useAuth();
  const navigate = useNavigate();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      await login(email, password);
      navigate("/dashboard");
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setLoading(false);
    }
  };

  const handleOAuth = async (provider: "google" | "github") => {
    try {
      await loginWithOAuth(provider);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "OAuth login failed");
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-black px-4">
      <div className="w-full max-w-sm space-y-6">
        <div className="text-center">
          <h1 className="text-2xl font-bold text-midground tracking-wider uppercase">
            Hermes Cloud
          </h1>
          <p className="mt-2 text-sm text-midground/60 tracking-wide">
            Sign in to your account
          </p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {error && (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-400">
              {error}
            </div>
          )}

          <div>
            <label className="block text-xs text-midground/60 uppercase tracking-wider mb-1">
              Email
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              className="w-full rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground placeholder:text-midground/30 focus:border-midground/40 focus:outline-none"
              placeholder="you@example.com"
            />
          </div>

          <div>
            <label className="block text-xs text-midground/60 uppercase tracking-wider mb-1">
              Password
            </label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              className="w-full rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground placeholder:text-midground/30 focus:border-midground/40 focus:outline-none"
              placeholder="••••••••"
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full rounded-lg bg-midground/10 border border-current/20 px-4 py-2.5 text-sm font-medium text-midground uppercase tracking-wider hover:bg-midground/20 disabled:opacity-50 transition-colors"
          >
            {loading ? "Signing in..." : "Sign in"}
          </button>
        </form>

        <div className="relative">
          <div className="absolute inset-0 flex items-center">
            <div className="w-full border-t border-current/10" />
          </div>
          <div className="relative flex justify-center text-xs">
            <span className="bg-black px-2 text-midground/40 uppercase tracking-wider">
              Or continue with
            </span>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <button
            onClick={() => handleOAuth("google")}
            className="rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground hover:bg-midground/10 transition-colors"
          >
            Google
          </button>
          <button
            onClick={() => handleOAuth("github")}
            className="rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground hover:bg-midground/10 transition-colors"
          >
            GitHub
          </button>
        </div>

        <p className="text-center text-xs text-midground/40">
          Don't have an account?{" "}
          <Link
            to="/register"
            className="text-midground/70 hover:text-midground underline"
          >
            Sign up
          </Link>
        </p>
      </div>
    </div>
  );
}
