import { useState, useEffect } from "react";
import { useAuth } from "@/contexts/AuthContext";

export default function SubscriptionPage() {
  const { plan, jwt } = useAuth();
  const [subscription, setSubscription] = useState<{
    plan: string;
    status: string;
    payment_method: string | null;
    current_period_end: string | null;
  } | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!jwt) return;
    const base = (import.meta.env.VITE_API_BASE_URL ?? "").replace(/\/$/, "");
    fetch(`${base}/api/v1/tenant/subscription`, {
      headers: { Authorization: `Bearer ${jwt}` },
    })
      .then((r) => r.json())
      .then(setSubscription)
      .catch(() => {});
  }, [jwt]);

  const handleSubscribe = async (method: "alipay" | "circle_usdc") => {
    if (!jwt) return;
    setLoading(true);
    try {
      const base = (import.meta.env.VITE_API_BASE_URL ?? "").replace(/\/$/, "");
      const res = await fetch(`${base}/api/v1/tenant/subscription`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${jwt}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ payment_method: method }),
      });
      const data = await res.json();
      if (data.payment_url) {
        window.open(data.payment_url, "_blank");
      }
    } catch {
      // Handle error
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-8">
      <h1 className="text-xl font-bold uppercase tracking-wider">
        Subscription
      </h1>

      {/* Current Plan */}
      <div className="rounded-xl border border-current/10 p-6">
        <h2 className="text-sm font-bold uppercase tracking-wider mb-4">
          Current Plan
        </h2>
        <div className="flex items-baseline gap-3">
          <span className="text-2xl font-bold uppercase">
            {plan === "cloud_pro" ? "Cloud Pro" : "Free"}
          </span>
          {subscription?.status && (
            <span className="text-xs text-midground/50 uppercase">
              ({subscription.status})
            </span>
          )}
        </div>
        {subscription?.current_period_end && (
          <p className="mt-2 text-xs text-midground/40 normal-case">
            Current period ends:{" "}
            {new Date(subscription.current_period_end).toLocaleDateString()}
          </p>
        )}
        {subscription?.payment_method && (
          <p className="mt-1 text-xs text-midground/40 normal-case">
            Payment method: {subscription.payment_method}
          </p>
        )}
      </div>

      {/* Upgrade */}
      {plan !== "cloud_pro" && (
        <div className="rounded-xl border border-midground/20 p-6">
          <h2 className="text-sm font-bold uppercase tracking-wider mb-2">
            Upgrade to Cloud Pro
          </h2>
          <p className="text-xs text-midground/50 normal-case mb-6">
            Get cloud-hosted agent, 17 platform connections, cron scheduling,
            skills, and memory.
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <button
              onClick={() => handleSubscribe("alipay")}
              disabled={loading}
              className="rounded-lg border border-current/20 bg-midground/5 px-6 py-3 text-sm hover:bg-midground/15 transition-colors disabled:opacity-50"
            >
              <div className="font-bold">¥69/month</div>
              <div className="text-xs text-midground/40 normal-case mt-1">
                Pay with Alipay
              </div>
            </button>
            <button
              onClick={() => handleSubscribe("circle_usdc")}
              disabled={loading}
              className="rounded-lg border border-current/20 bg-midground/5 px-6 py-3 text-sm hover:bg-midground/15 transition-colors disabled:opacity-50"
            >
              <div className="font-bold">$9.90/month</div>
              <div className="text-xs text-midground/40 normal-case mt-1">
                Pay with USDC
              </div>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
