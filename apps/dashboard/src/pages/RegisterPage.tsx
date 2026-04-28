import { useState } from "react";
import { Link } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";

export default function RegisterPage() {
  const { register, loginWithOAuth } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (password !== confirmPassword) {
      setError("Passwords do not match");
      return;
    }
    if (password.length < 6) {
      setError("Password must be at least 6 characters");
      return;
    }

    setLoading(true);
    try {
      await register(email, password);
      setSuccess(true);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Registration failed");
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

  if (success) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-black px-4">
        <div className="w-full max-w-sm space-y-6 text-center">
          <h1 className="text-2xl font-bold text-midground tracking-wider uppercase">
            Check your email
          </h1>
          <p className="text-sm text-midground/60">
            We sent a confirmation link to <strong>{email}</strong>. Click it to
            activate your account.
          </p>
          <Link
            to="/login"
            className="inline-block text-sm text-midground/70 hover:text-midground underline"
          >
            Back to login
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-black px-4">
      <div className="w-full max-w-sm space-y-6">
        <div className="text-center">
          <h1 className="text-2xl font-bold text-midground tracking-wider uppercase">
            Create Account
          </h1>
          <p className="mt-2 text-sm text-midground/60 tracking-wide">
            Get started with Hermes Cloud
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

          <div>
            <label className="block text-xs text-midground/60 uppercase tracking-wider mb-1">
              Confirm Password
            </label>
            <input
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
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
            {loading ? "Creating account..." : "Create account"}
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
          Already have an account?{" "}
          <Link
            to="/login"
            className="text-midground/70 hover:text-midground underline"
          >
            Sign in
          </Link>
        </p>
      </div>
    </div>
  );
}
