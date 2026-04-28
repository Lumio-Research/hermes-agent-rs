import { Link } from "react-router-dom";
import { Zap, Shield, Globe, Terminal, Cpu, Clock } from "lucide-react";

const FEATURES = [
  {
    icon: Zap,
    title: "10 LLM Providers",
    desc: "OpenAI, Anthropic, DeepSeek, Qwen, and more. Switch models mid-conversation.",
  },
  {
    icon: Globe,
    title: "17 Platform Adapters",
    desc: "Telegram, Discord, Slack, WeChat, WhatsApp, Signal, Matrix, and 10 more.",
  },
  {
    icon: Terminal,
    title: "30+ Tools",
    desc: "File ops, browser, code execution, vision, voice, web search, Home Assistant.",
  },
  {
    icon: Cpu,
    title: "Single Binary",
    desc: "~16MB static binary. No Python, no Docker. Runs on a Raspberry Pi.",
  },
  {
    icon: Shield,
    title: "BYOK Security",
    desc: "Bring your own API keys. AES-256 encrypted. We never see your keys.",
  },
  {
    icon: Clock,
    title: "Cron & Memory",
    desc: "Schedule tasks, persistent memory across sessions. The agent learns.",
  },
];

export default function LandingPage() {
  return (
    <div className="min-h-screen bg-black text-midground">
      {/* Hero */}
      <header className="border-b border-current/10">
        <div className="mx-auto max-w-5xl flex items-center justify-between px-6 py-4">
          <span className="font-bold text-lg tracking-wider uppercase">
            Hermes Agent
          </span>
          <div className="flex gap-3">
            <Link
              to="/login"
              className="rounded-lg border border-current/20 px-4 py-2 text-sm hover:bg-midground/10 transition-colors"
            >
              Sign in
            </Link>
            <Link
              to="/register"
              className="rounded-lg bg-midground/10 border border-current/20 px-4 py-2 text-sm font-medium hover:bg-midground/20 transition-colors"
            >
              Get started
            </Link>
          </div>
        </div>
      </header>

      <section className="mx-auto max-w-5xl px-6 py-20 text-center">
        <h1 className="text-4xl sm:text-5xl font-bold tracking-wider uppercase leading-tight">
          Self-evolving AI Agent
          <br />
          <span className="text-midground/60">One binary. Every platform.</span>
        </h1>
        <p className="mt-6 text-lg text-midground/50 max-w-2xl mx-auto">
          Connect your LLM to Telegram, Discord, Slack, WeChat, and 13 more
          platforms. Hosted for you, or self-hosted for free.
        </p>
        <div className="mt-10 flex justify-center gap-4">
          <Link
            to="/register"
            className="rounded-lg bg-midground/15 border border-current/20 px-8 py-3 text-sm font-medium uppercase tracking-wider hover:bg-midground/25 transition-colors"
          >
            Start free trial
          </Link>
          <a
            href="https://github.com/Lumio-Research/hermes-agent-rs"
            target="_blank"
            rel="noopener noreferrer"
            className="rounded-lg border border-current/20 px-8 py-3 text-sm uppercase tracking-wider hover:bg-midground/10 transition-colors"
          >
            View on GitHub
          </a>
        </div>
      </section>

      {/* Features */}
      <section className="border-t border-current/10 py-16">
        <div className="mx-auto max-w-5xl px-6">
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-8">
            {FEATURES.map((f) => (
              <div
                key={f.title}
                className="rounded-xl border border-current/10 p-6 hover:border-current/20 transition-colors"
              >
                <f.icon className="h-6 w-6 mb-3 text-midground/70" />
                <h3 className="text-sm font-bold uppercase tracking-wider mb-2">
                  {f.title}
                </h3>
                <p className="text-xs text-midground/50 leading-relaxed normal-case">
                  {f.desc}
                </p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Pricing */}
      <section className="border-t border-current/10 py-16" id="pricing">
        <div className="mx-auto max-w-5xl px-6">
          <h2 className="text-2xl font-bold text-center uppercase tracking-wider mb-12">
            Pricing
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-8 max-w-3xl mx-auto">
            {/* Free */}
            <div className="rounded-xl border border-current/10 p-8">
              <h3 className="text-lg font-bold uppercase tracking-wider">
                Free
              </h3>
              <p className="mt-1 text-xs text-midground/50 normal-case">
                Self-hosted
              </p>
              <div className="mt-4 text-3xl font-bold">$0</div>
              <p className="text-xs text-midground/40 normal-case">forever</p>
              <ul className="mt-6 space-y-2 text-xs text-midground/60 normal-case">
                <li>✓ Full agent capabilities</li>
                <li>✓ All 10 LLM providers</li>
                <li>✓ 30+ tools</li>
                <li>✓ CLI + TUI</li>
                <li>✓ Open source (MIT)</li>
                <li className="text-midground/30">✗ Cloud hosting</li>
                <li className="text-midground/30">✗ Platform connections</li>
                <li className="text-midground/30">✗ Web dashboard</li>
              </ul>
              <a
                href="https://github.com/Lumio-Research/hermes-agent-rs"
                target="_blank"
                rel="noopener noreferrer"
                className="mt-8 block w-full rounded-lg border border-current/20 py-2.5 text-center text-sm hover:bg-midground/10 transition-colors"
              >
                Download
              </a>
            </div>

            {/* Cloud Pro */}
            <div className="rounded-xl border border-midground/30 p-8 relative">
              <span className="absolute -top-3 left-6 bg-black px-2 text-xs text-midground/70 uppercase tracking-wider">
                Recommended
              </span>
              <h3 className="text-lg font-bold uppercase tracking-wider">
                Cloud Pro
              </h3>
              <p className="mt-1 text-xs text-midground/50 normal-case">
                We host it for you
              </p>
              <div className="mt-4 text-3xl font-bold">
                $9.90
                <span className="text-sm font-normal text-midground/40">
                  /month
                </span>
              </div>
              <p className="text-xs text-midground/40 normal-case">
                or ¥69/month via Alipay
              </p>
              <ul className="mt-6 space-y-2 text-xs text-midground/60 normal-case">
                <li>✓ Everything in Free</li>
                <li>✓ Cloud-hosted agent</li>
                <li>✓ 17 platform connections</li>
                <li>✓ Web dashboard</li>
                <li>✓ Cron scheduling</li>
                <li>✓ Skills & memory</li>
                <li>✓ BYOK (bring your own key)</li>
                <li>✓ Alipay & USDC payments</li>
              </ul>
              <Link
                to="/register"
                className="mt-8 block w-full rounded-lg bg-midground/15 border border-current/20 py-2.5 text-center text-sm font-medium hover:bg-midground/25 transition-colors"
              >
                Get started
              </Link>
            </div>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-current/10 py-8">
        <div className="mx-auto max-w-5xl px-6 flex items-center justify-between text-xs text-midground/40 normal-case">
          <span>© 2026 Lumio Research</span>
          <a
            href="https://github.com/Lumio-Research/hermes-agent-rs"
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-midground/60"
          >
            GitHub
          </a>
        </div>
      </footer>
    </div>
  );
}
