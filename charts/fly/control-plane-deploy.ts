#!/usr/bin/env node
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
  appendFileSync,
  openSync,
  closeSync,
} from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import YAML from "yaml";
import pg from "pg";
const { Client } = pg as { Client: new (...args: unknown[]) => any };

type TenantSpec = {
  metadata: {
    tenantId: string;
    displayName: string;
    environment: string;
  };
  spec: {
    deployment: {
      appName: string;
      region: string;
      image: {
        repository: string;
        tag: string;
      };
      vmSize: string;
      minMachines: number;
      internalPort: number;
    };
    domain?: {
      hostname?: string;
      enableTls?: boolean;
    };
    runtime: {
      logLevel?: string;
      hermesHome: string;
      publicBaseUrl: string;
    };
    knowledgeBase: {
      enabled: boolean;
      volume: {
        name: string;
        sizeGb: number;
      };
    };
    secrets?: string[];
  };
};

type DeployState = {
  tenantId: string;
  appName: string;
  status:
    | "draft"
    | "validating"
    | "provisioning"
    | "deploying"
    | "verifying"
    | "running"
    | "failed"
    | "rollback_in_progress"
    | "rolled_back";
  currentImage?: string;
  lastStableImage?: string;
  lastError?: string;
  updatedAt: string;
};

type CliOptions = {
  action: "deploy" | "rollback" | "status";
  specPath: string;
  templatePath: string;
  stateDir: string;
  stateBackend: "file" | "postgres";
  pgConnectionString?: string;
  pgSchema: string;
  outputDir?: string;
  secretsBackend: "env" | "file" | "vault";
  secretsFile?: string;
  secretPrefix: string;
  vaultAddress?: string;
  vaultToken?: string;
  vaultMount: string;
  vaultPathPrefix: string;
  healthTimeoutSec: number;
  lockTimeoutSec: number;
  autoRollback: boolean;
};

type LockHandle = {
  lockPath: string;
  fd: number;
};

const HEALTH_PATHS = ["/healthz", "/readyz"];

function usage(): void {
  console.log(`Usage:
  pnpm exec tsx control-plane-deploy.ts --action deploy --spec tenant-spec.example.yaml [options]

Required:
  --spec <path>                Tenant spec path

Optional:
  --action <deploy|rollback|status>  Action to run (default: deploy)
  --template <path>            fly.toml template (default: ./fly.toml.tmpl)
  --state-backend <file|postgres>    State backend (default: file)
  --state-dir <path>           state storage dir (default: .control-plane-state)
  --pg-connection-string <dsn> Postgres DSN (required for postgres backend)
  --pg-schema <name>           Postgres schema for state tables (default: hermes_control_plane)
  --output-dir <path>          rendered output root dir (default: .fly-generated/<tenantId>)
  --secrets-backend <env|file|vault> secret backend (default: env)
  --secrets-file <path>        required when --secrets-backend file
  --secret-prefix <prefix>     env prefix for secrets backend env (default: "")
  --vault-address <url>        Vault address (required for vault backend)
  --vault-token <token>        Vault token (required for vault backend)
  --vault-mount <name>         Vault KVv2 mount (default: secret)
  --vault-path-prefix <path>   Vault path prefix (default: hermes/tenants)
  --health-timeout-sec <n>     health check timeout seconds (default: 120)
  --lock-timeout-sec <n>       lock stale timeout seconds (default: 900)
  --no-auto-rollback           disable rollback on failed verify stage
  -h, --help                   show help
`);
}

function parseArgs(argv: string[]): CliOptions {
  let action: CliOptions["action"] = "deploy";
  let specPath = "";
  let templatePath = "./fly.toml.tmpl";
  let stateDir = ".control-plane-state";
  let stateBackend: CliOptions["stateBackend"] = "file";
  let pgConnectionString: string | undefined;
  let pgSchema = "hermes_control_plane";
  let outputDir: string | undefined;
  let secretsBackend: CliOptions["secretsBackend"] = "env";
  let secretsFile: string | undefined;
  let secretPrefix = "";
  let vaultAddress: string | undefined;
  let vaultToken: string | undefined;
  let vaultMount = "secret";
  let vaultPathPrefix = "hermes/tenants";
  let healthTimeoutSec = 120;
  let lockTimeoutSec = 900;
  let autoRollback = true;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--") {
      continue;
    }
    switch (arg) {
      case "--action": {
        const v = argv[++i] ?? "";
        if (v !== "deploy" && v !== "rollback" && v !== "status") {
          throw new Error(`invalid --action: ${v}`);
        }
        action = v;
        break;
      }
      case "--spec":
        specPath = argv[++i] ?? "";
        break;
      case "--template":
        templatePath = argv[++i] ?? "";
        break;
      case "--state-backend": {
        const v = argv[++i] ?? "";
        if (v !== "file" && v !== "postgres") throw new Error(`invalid --state-backend: ${v}`);
        stateBackend = v;
        break;
      }
      case "--state-dir":
        stateDir = argv[++i] ?? "";
        break;
      case "--pg-connection-string":
        pgConnectionString = argv[++i] ?? "";
        break;
      case "--pg-schema":
        pgSchema = argv[++i] ?? "";
        break;
      case "--output-dir":
        outputDir = argv[++i] ?? "";
        break;
      case "--secrets-backend": {
        const v = argv[++i] ?? "";
        if (v !== "env" && v !== "file" && v !== "vault") throw new Error(`invalid --secrets-backend: ${v}`);
        secretsBackend = v;
        break;
      }
      case "--secrets-file":
        secretsFile = argv[++i] ?? "";
        break;
      case "--secret-prefix":
        secretPrefix = argv[++i] ?? "";
        break;
      case "--vault-address":
        vaultAddress = argv[++i] ?? "";
        break;
      case "--vault-token":
        vaultToken = argv[++i] ?? "";
        break;
      case "--vault-mount":
        vaultMount = argv[++i] ?? "";
        break;
      case "--vault-path-prefix":
        vaultPathPrefix = argv[++i] ?? "";
        break;
      case "--health-timeout-sec":
        healthTimeoutSec = Number(argv[++i] ?? "120");
        break;
      case "--lock-timeout-sec":
        lockTimeoutSec = Number(argv[++i] ?? "900");
        break;
      case "--no-auto-rollback":
        autoRollback = false;
        break;
      case "-h":
      case "--help":
        usage();
        process.exit(0);
      default:
        throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!specPath) throw new Error("--spec is required");
  if (!Number.isFinite(healthTimeoutSec) || healthTimeoutSec < 10 || healthTimeoutSec > 1800) {
    throw new Error("--health-timeout-sec must be between 10 and 1800");
  }
  if (!Number.isFinite(lockTimeoutSec) || lockTimeoutSec < 30 || lockTimeoutSec > 86400) {
    throw new Error("--lock-timeout-sec must be between 30 and 86400");
  }
  if (secretsBackend === "file" && !secretsFile) {
    throw new Error("--secrets-file is required when --secrets-backend file");
  }
  if (secretsBackend === "vault" && (!vaultAddress || !vaultToken)) {
    throw new Error("--vault-address and --vault-token are required when --secrets-backend vault");
  }
  if (stateBackend === "postgres" && !pgConnectionString) {
    throw new Error("--pg-connection-string is required when --state-backend postgres");
  }
  if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(pgSchema)) {
    throw new Error("--pg-schema must be a valid SQL identifier");
  }

  return {
    action,
    specPath,
    templatePath,
    stateDir,
    stateBackend,
    pgConnectionString,
    pgSchema,
    outputDir,
    secretsBackend,
    secretsFile,
    secretPrefix,
    vaultAddress,
    vaultToken,
    vaultMount,
    vaultPathPrefix,
    healthTimeoutSec,
    lockTimeoutSec,
    autoRollback,
  };
}

function must(value: unknown, field: string): string {
  if (typeof value !== "string" || value.trim().length === 0) throw new Error(`${field} is required`);
  return value.trim();
}

function mustNumber(value: unknown, field: string): number {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) throw new Error(`${field} must be number`);
  return n;
}

function isValidHostname(hostname: string): boolean {
  if (hostname.length > 253 || hostname.includes("://")) return false;
  const labels = hostname.split(".");
  return labels.every((label) => /^[a-zA-Z0-9-]{1,63}$/.test(label) && !label.startsWith("-") && !label.endsWith("-"));
}

function ensureHttpUrl(url: string, field: string): void {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`${field} must be valid URL`);
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") throw new Error(`${field} must be http/https`);
}

function loadSpec(specPath: string): TenantSpec {
  if (!existsSync(specPath)) throw new Error(`spec not found: ${specPath}`);
  const raw = readFileSync(specPath, "utf8");
  const parsed = YAML.parse(raw) as TenantSpec;

  const tenantId = must(parsed?.metadata?.tenantId, "metadata.tenantId");
  const displayName = must(parsed?.metadata?.displayName, "metadata.displayName");
  const environment = must(parsed?.metadata?.environment, "metadata.environment");
  const appName = must(parsed?.spec?.deployment?.appName, "spec.deployment.appName");
  const region = must(parsed?.spec?.deployment?.region, "spec.deployment.region");
  const repo = must(parsed?.spec?.deployment?.image?.repository, "spec.deployment.image.repository");
  const tag = must(parsed?.spec?.deployment?.image?.tag, "spec.deployment.image.tag");
  const vmSize = must(parsed?.spec?.deployment?.vmSize, "spec.deployment.vmSize");
  const minMachines = mustNumber(parsed?.spec?.deployment?.minMachines, "spec.deployment.minMachines");
  const internalPort = mustNumber(parsed?.spec?.deployment?.internalPort, "spec.deployment.internalPort");
  const home = must(parsed?.spec?.runtime?.hermesHome, "spec.runtime.hermesHome");
  const baseUrl = must(parsed?.spec?.runtime?.publicBaseUrl, "spec.runtime.publicBaseUrl");
  const volName = must(parsed?.spec?.knowledgeBase?.volume?.name, "spec.knowledgeBase.volume.name");
  const volSize = mustNumber(parsed?.spec?.knowledgeBase?.volume?.sizeGb, "spec.knowledgeBase.volume.sizeGb");

  if (!/^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/.test(appName)) throw new Error("spec.deployment.appName invalid");
  if (!/^[a-z]{3}$/.test(region)) throw new Error("spec.deployment.region invalid");
  if (!/^[a-zA-Z0-9._/-]+$/.test(repo)) throw new Error("spec.deployment.image.repository invalid");
  if (!/^[a-zA-Z0-9._-]+$/.test(tag)) throw new Error("spec.deployment.image.tag invalid");
  if (!vmSize) throw new Error("spec.deployment.vmSize invalid");
  if (!Number.isInteger(minMachines) || minMachines < 1 || minMachines > 20) throw new Error("spec.deployment.minMachines out of range");
  if (!Number.isInteger(internalPort) || internalPort < 1 || internalPort > 65535) throw new Error("spec.deployment.internalPort out of range");
  if (!home.startsWith("/")) throw new Error("spec.runtime.hermesHome must be absolute");
  ensureHttpUrl(baseUrl, "spec.runtime.publicBaseUrl");
  if (!/^[a-zA-Z0-9-]{3,63}$/.test(volName)) throw new Error("spec.knowledgeBase.volume.name invalid");
  if (!Number.isInteger(volSize) || volSize < 1 || volSize > 500) throw new Error("spec.knowledgeBase.volume.sizeGb out of range");
  if (parsed?.spec?.domain?.hostname && !isValidHostname(parsed.spec.domain.hostname)) {
    throw new Error("spec.domain.hostname invalid");
  }
  if (tenantId.length < 2 || tenantId.length > 64) throw new Error("metadata.tenantId length invalid");
  if (displayName.length > 120) throw new Error("metadata.displayName too long");
  if (!environment) throw new Error("metadata.environment invalid");

  return parsed;
}

function collectSecretRefs(node: unknown, out: Set<string> = new Set()): Set<string> {
  if (Array.isArray(node)) {
    for (const item of node) collectSecretRefs(item, out);
    return out;
  }
  if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
      if (key.endsWith("SecretRef") && typeof value === "string" && value.trim()) out.add(value.trim());
      else collectSecretRefs(value, out);
    }
  }
  return out;
}

function parseSecretsFile(filePath: string): Map<string, string> {
  const lines = readFileSync(filePath, "utf8").split(/\r?\n/);
  const map = new Map<string, string>();
  for (const line of lines) {
    const t = line.trim();
    if (!t || t.startsWith("#")) continue;
    const idx = t.indexOf("=");
    if (idx <= 0) throw new Error(`invalid secrets line: ${line}`);
    const key = t.slice(0, idx).trim();
    const value = t.slice(idx + 1);
    if (!/^[A-Z][A-Z0-9_]*$/.test(key)) throw new Error(`invalid secret key: ${key}`);
    if (!value) throw new Error(`secret ${key} has empty value`);
    map.set(key, value);
  }
  return map;
}

async function resolveSecrets(spec: TenantSpec, options: CliOptions): Promise<Map<string, string>> {
  const required = collectSecretRefs(spec);
  for (const name of spec.spec.secrets ?? []) required.add(name);
  if (required.size === 0) return new Map();

  const resolved = new Map<string, string>();
  if (options.secretsBackend === "file") {
    const source = parseSecretsFile(options.secretsFile!);
    for (const key of required) {
      const val = source.get(key);
      if (!val) throw new Error(`missing secret in file backend: ${key}`);
      resolved.set(key, val);
    }
    return resolved;
  }

  if (options.secretsBackend === "vault") {
    const tenantId = spec.metadata.tenantId;
    const mount = options.vaultMount.replace(/^\/+|\/+$/g, "");
    const prefix = options.vaultPathPrefix.replace(/^\/+|\/+$/g, "");
    const url = new URL(
      `/v1/${mount}/data/${prefix}/${tenantId}`,
      options.vaultAddress!,
    ).toString();
    const res = await fetch(url, {
      method: "GET",
      headers: {
        "X-Vault-Token": options.vaultToken!,
      },
    });
    if (!res.ok) {
      throw new Error(`vault request failed (${res.status}) for ${url}`);
    }
    const body = (await res.json()) as {
      data?: { data?: Record<string, string> };
    };
    const data = body?.data?.data ?? {};
    for (const key of required) {
      const value = data[key];
      if (!value || !value.trim()) {
        throw new Error(`missing secret in vault backend: ${key}`);
      }
      resolved.set(key, value);
    }
    return resolved;
  }

  for (const key of required) {
    const envKey = `${options.secretPrefix}${key}`;
    const value = process.env[envKey];
    if (!value || !value.trim()) throw new Error(`missing secret in env backend: ${envKey}`);
    resolved.set(key, value);
  }
  return resolved;
}

function runFly(args: string[], opts?: { allowFail?: boolean; retries?: number; capture?: boolean }): { code: number; stdout: string; stderr: string } {
  const retries = opts?.retries ?? 1;
  let last: { code: number; stdout: string; stderr: string } = { code: 1, stdout: "", stderr: "" };
  for (let i = 1; i <= retries; i += 1) {
    const res = spawnSync("fly", args, { encoding: "utf8", stdio: opts?.capture ? "pipe" : "pipe" });
    last = {
      code: res.status ?? 1,
      stdout: res.stdout ?? "",
      stderr: res.stderr ?? "",
    };
    if (last.code === 0) return last;
    if (i < retries) {
      const backoff = i * 1000;
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, backoff);
    }
  }
  if (!opts?.allowFail) {
    throw new Error(`fly ${args.join(" ")} failed: ${last.stderr || last.stdout}`);
  }
  return last;
}

function ensureFlyReady(): void {
  runFly(["version"]);
  runFly(["auth", "whoami"]);
}

type StateStore = {
  init(): Promise<void>;
  loadState(tenantId: string, appName: string): Promise<DeployState>;
  saveState(state: DeployState): Promise<void>;
  appendEvent(
    tenantId: string,
    level: "info" | "warn" | "error",
    message: string,
    data?: unknown,
  ): Promise<void>;
  getEventsTail(tenantId: string, limit: number): Promise<unknown[]>;
  withTenantLock<T>(tenantId: string, lockTimeoutSec: number, fn: () => Promise<T>): Promise<T>;
  close(): Promise<void>;
};

function tenantPaths(stateDir: string, tenantId: string): { root: string; state: string; events: string; lock: string } {
  const root = path.resolve(stateDir, tenantId);
  return {
    root,
    state: path.join(root, "state.json"),
    events: path.join(root, "events.jsonl"),
    lock: path.join(root, "deploy.lock"),
  };
}

function loadState(statePath: string, tenantId: string, appName: string): DeployState {
  if (!existsSync(statePath)) {
    return {
      tenantId,
      appName,
      status: "draft",
      updatedAt: new Date().toISOString(),
    };
  }
  const parsed = JSON.parse(readFileSync(statePath, "utf8")) as DeployState;
  return parsed;
}

function saveState(statePath: string, state: DeployState): void {
  state.updatedAt = new Date().toISOString();
  writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`, "utf8");
}

function appendEvent(eventsPath: string, level: "info" | "warn" | "error", message: string, data?: unknown): void {
  const line = JSON.stringify({
    at: new Date().toISOString(),
    level,
    message,
    data,
  });
  appendFileSync(eventsPath, `${line}\n`, "utf8");
}

function acquireLock(lockPath: string, lockTimeoutSec: number): LockHandle {
  mkdirSync(path.dirname(lockPath), { recursive: true });
  if (existsSync(lockPath)) {
    const stat = JSON.parse(readFileSync(lockPath, "utf8")) as { pid: number; at: string };
    const ageMs = Date.now() - new Date(stat.at).getTime();
    if (Number.isFinite(ageMs) && ageMs > lockTimeoutSec * 1000) {
      rmSync(lockPath, { force: true });
    } else {
      throw new Error(`deployment lock exists (pid=${stat.pid}, at=${stat.at})`);
    }
  }
  const fd = openSync(lockPath, "wx");
  writeFileSync(fd, JSON.stringify({ pid: process.pid, at: new Date().toISOString() }));
  return { lockPath, fd };
}

function releaseLock(lock: LockHandle): void {
  closeSync(lock.fd);
  rmSync(lock.lockPath, { force: true });
}

function buildFileStateStore(
  stateDir: string,
): StateStore {
  return {
    async init() {
      mkdirSync(path.resolve(stateDir), { recursive: true });
    },
    async loadState(tenantId, appName) {
      const p = tenantPaths(stateDir, tenantId);
      return loadState(p.state, tenantId, appName);
    },
    async saveState(state) {
      const p = tenantPaths(stateDir, state.tenantId);
      mkdirSync(p.root, { recursive: true });
      saveState(p.state, state);
    },
    async appendEvent(tenantId, level, message, data) {
      const p = tenantPaths(stateDir, tenantId);
      mkdirSync(p.root, { recursive: true });
      appendEvent(p.events, level, message, data);
    },
    async getEventsTail(tenantId, limit) {
      const p = tenantPaths(stateDir, tenantId);
      if (!existsSync(p.events)) return [];
      return readFileSync(p.events, "utf8")
        .trim()
        .split(/\r?\n/)
        .filter(Boolean)
        .slice(-limit)
        .map((line) => JSON.parse(line));
    },
    async withTenantLock(tenantId, lockTimeoutSec, fn) {
      const p = tenantPaths(stateDir, tenantId);
      const lock = acquireLock(p.lock, lockTimeoutSec);
      try {
        return await fn();
      } finally {
        releaseLock(lock);
      }
    },
    async close() {},
  };
}

class PostgresStateStore implements StateStore {
  constructor(private client: any, private schema: string) {}

  async init(): Promise<void> {
    await this.client.connect();
    await this.client.query(`CREATE SCHEMA IF NOT EXISTS ${this.schema}`);
    await this.client.query(`
      CREATE TABLE IF NOT EXISTS ${this.schema}.deploy_state (
        tenant_id TEXT PRIMARY KEY,
        app_name TEXT NOT NULL,
        status TEXT NOT NULL,
        current_image TEXT,
        last_stable_image TEXT,
        last_error TEXT,
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
      )
    `);
    await this.client.query(`
      CREATE TABLE IF NOT EXISTS ${this.schema}.deploy_events (
        id BIGSERIAL PRIMARY KEY,
        tenant_id TEXT NOT NULL,
        at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        level TEXT NOT NULL,
        message TEXT NOT NULL,
        data JSONB
      )
    `);
  }

  async loadState(tenantId: string, appName: string): Promise<DeployState> {
    const q = await this.client.query(
      `SELECT tenant_id, app_name, status, current_image, last_stable_image, last_error, updated_at
       FROM ${this.schema}.deploy_state
       WHERE tenant_id = $1`,
      [tenantId],
    );
    if (q.rowCount === 0) {
      return {
        tenantId,
        appName,
        status: "draft",
        updatedAt: new Date().toISOString(),
      };
    }
    const row = q.rows[0];
    return {
      tenantId: row.tenant_id,
      appName: row.app_name,
      status: row.status,
      currentImage: row.current_image ?? undefined,
      lastStableImage: row.last_stable_image ?? undefined,
      lastError: row.last_error ?? undefined,
      updatedAt: new Date(row.updated_at).toISOString(),
    };
  }

  async saveState(state: DeployState): Promise<void> {
    state.updatedAt = new Date().toISOString();
    await this.client.query(
      `INSERT INTO ${this.schema}.deploy_state
       (tenant_id, app_name, status, current_image, last_stable_image, last_error, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7)
       ON CONFLICT (tenant_id) DO UPDATE SET
         app_name = EXCLUDED.app_name,
         status = EXCLUDED.status,
         current_image = EXCLUDED.current_image,
         last_stable_image = EXCLUDED.last_stable_image,
         last_error = EXCLUDED.last_error,
         updated_at = EXCLUDED.updated_at`,
      [
        state.tenantId,
        state.appName,
        state.status,
        state.currentImage ?? null,
        state.lastStableImage ?? null,
        state.lastError ?? null,
        state.updatedAt,
      ],
    );
  }

  async appendEvent(
    tenantId: string,
    level: "info" | "warn" | "error",
    message: string,
    data?: unknown,
  ): Promise<void> {
    await this.client.query(
      `INSERT INTO ${this.schema}.deploy_events (tenant_id, level, message, data)
       VALUES ($1, $2, $3, $4::jsonb)`,
      [tenantId, level, message, JSON.stringify(data ?? null)],
    );
  }

  async getEventsTail(tenantId: string, limit: number): Promise<unknown[]> {
    const q = await this.client.query(
      `SELECT at, level, message, data
       FROM ${this.schema}.deploy_events
       WHERE tenant_id = $1
       ORDER BY id DESC
       LIMIT $2`,
      [tenantId, limit],
    );
    return q.rows
      .reverse()
      .map((r: { at: string; level: string; message: string; data: unknown }) => ({
        at: new Date(r.at).toISOString(),
        level: r.level,
        message: r.message,
        data: r.data,
      }));
  }

  async withTenantLock<T>(tenantId: string, lockTimeoutSec: number, fn: () => Promise<T>): Promise<T> {
    const deadline = Date.now() + lockTimeoutSec * 1000;
    let locked = false;
    while (Date.now() < deadline) {
      const q = await this.client.query("SELECT pg_try_advisory_lock(hashtext($1)) AS locked", [tenantId]);
      if (q.rows[0]?.locked) {
        locked = true;
        break;
      }
      await new Promise((r) => setTimeout(r, 1000));
    }
    if (!locked) {
      throw new Error(`failed to acquire postgres deploy lock for tenant ${tenantId}`);
    }
    try {
      return await fn();
    } finally {
      await this.client.query("SELECT pg_advisory_unlock(hashtext($1))", [tenantId]);
    }
  }

  async close(): Promise<void> {
    await this.client.end();
  }
}

function buildStateStore(options: CliOptions): StateStore {
  if (options.stateBackend === "postgres") {
    return new PostgresStateStore(
      new Client({ connectionString: options.pgConnectionString! }),
      options.pgSchema,
    );
  }
  return buildFileStateStore(options.stateDir);
}

function renderFlyToml(templatePath: string, outputPath: string, spec: TenantSpec): void {
  const tpl = readFileSync(templatePath, "utf8");
  const d = spec.spec.deployment;
  const r = spec.spec.runtime;
  const replacements: Record<string, string> = {
    app_name: d.appName,
    region: d.region,
    image_repository: d.image.repository,
    image_tag: d.image.tag,
    log_level: r.logLevel || "info",
    hermes_home: r.hermesHome,
    public_base_url: r.publicBaseUrl,
    tenant_id: spec.metadata.tenantId,
    tenant_name: spec.metadata.displayName,
    environment: spec.metadata.environment,
    volume_name: spec.spec.knowledgeBase.volume.name,
    internal_port: String(d.internalPort),
    min_machines: String(d.minMachines),
    vm_size: d.vmSize,
  };
  let out = tpl;
  for (const [k, v] of Object.entries(replacements)) {
    out = out.replaceAll(`{{ ${k} }}`, v);
  }
  mkdirSync(path.dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, out, "utf8");
}

async function ensureAppAndVolume(spec: TenantSpec, store: StateStore): Promise<void> {
  const app = spec.spec.deployment.appName;
  const region = spec.spec.deployment.region;
  const volume = spec.spec.knowledgeBase.volume;
  const appShow = runFly(["apps", "show", app], { allowFail: true, capture: true });
  if (appShow.code !== 0) {
    await store.appendEvent(spec.metadata.tenantId, "info", "creating app", { app });
    runFly(["apps", "create", app]);
  } else {
    await store.appendEvent(spec.metadata.tenantId, "info", "app exists", { app });
  }

  const volList = runFly(["volumes", "list", "-a", app], { capture: true });
  const hasVolume = volList.stdout.split(/\r?\n/).some((line) => line.startsWith(`${volume.name} `));
  if (!hasVolume) {
    await store.appendEvent(spec.metadata.tenantId, "info", "creating volume", {
      app,
      volume: volume.name,
      sizeGb: volume.sizeGb,
    });
    runFly(["volumes", "create", volume.name, "--region", region, "--size", String(volume.sizeGb), "-a", app]);
  } else {
    await store.appendEvent(spec.metadata.tenantId, "info", "volume exists", { app, volume: volume.name });
  }
}

async function setFlySecrets(
  tenantId: string,
  app: string,
  secrets: Map<string, string>,
  store: StateStore,
): Promise<void> {
  if (secrets.size === 0) {
    await store.appendEvent(tenantId, "warn", "no secrets resolved, skipping fly secrets set");
    return;
  }
  const pairs = [...secrets.entries()].map(([k, v]) => `${k}=${v}`);
  await store.appendEvent(tenantId, "info", "setting fly secrets", { app, keys: [...secrets.keys()] });
  runFly(["secrets", "set", "-a", app, ...pairs]);
}

async function verifyHealth(
  tenantId: string,
  baseUrl: string,
  timeoutSec: number,
  store: StateStore,
): Promise<void> {
  const deadline = Date.now() + timeoutSec * 1000;
  let lastErr = "unknown";
  while (Date.now() < deadline) {
    let allOk = true;
    for (const p of HEALTH_PATHS) {
      const url = new URL(p, baseUrl).toString();
      try {
        const res = await fetch(url, { method: "GET", redirect: "follow" });
        if (!res.ok) {
          allOk = false;
          lastErr = `${url} -> ${res.status}`;
          break;
        }
      } catch (e) {
        allOk = false;
        lastErr = `${url} -> ${e instanceof Error ? e.message : String(e)}`;
        break;
      }
    }
    if (allOk) {
      await store.appendEvent(tenantId, "info", "health checks passed", { baseUrl, paths: HEALTH_PATHS });
      return;
    }
    await new Promise((r) => setTimeout(r, 3000));
  }
  throw new Error(`health gate failed: ${lastErr}`);
}

async function deployFlow(spec: TenantSpec, options: CliOptions, store: StateStore): Promise<void> {
  const image = `${spec.spec.deployment.image.repository}:${spec.spec.deployment.image.tag}`;
  const outputDir = path.resolve(options.outputDir || `.fly-generated/${spec.metadata.tenantId}`);
  const renderedToml = path.join(outputDir, "fly.toml");
  await store.withTenantLock(spec.metadata.tenantId, options.lockTimeoutSec, async () => {
    let state = await store.loadState(spec.metadata.tenantId, spec.spec.deployment.appName);
    try {
      await store.appendEvent(spec.metadata.tenantId, "info", "deploy requested", {
        tenantId: spec.metadata.tenantId,
        image,
      });
      state.status = "validating";
      await store.saveState(state);

      ensureFlyReady();
      const secrets = await resolveSecrets(spec, options);

      state.status = "provisioning";
      await store.saveState(state);
      await ensureAppAndVolume(spec, store);

      state.status = "deploying";
      await store.saveState(state);
      await setFlySecrets(spec.metadata.tenantId, spec.spec.deployment.appName, secrets, store);
      renderFlyToml(options.templatePath, renderedToml, spec);
      runFly(
        ["deploy", "-a", spec.spec.deployment.appName, "--config", renderedToml, "--image", image],
        { retries: 3 },
      );

      if (spec.spec.domain?.hostname && spec.spec.domain.enableTls !== false) {
        const host = spec.spec.domain.hostname;
        const cert = runFly(["certs", "show", host, "-a", spec.spec.deployment.appName], {
          allowFail: true,
          capture: true,
        });
        if (cert.code !== 0) {
          await store.appendEvent(spec.metadata.tenantId, "info", "adding domain cert", { host });
          runFly(["certs", "add", host, "-a", spec.spec.deployment.appName]);
        }
      }

      state.status = "verifying";
      await store.saveState(state);
      await verifyHealth(spec.metadata.tenantId, spec.spec.runtime.publicBaseUrl, options.healthTimeoutSec, store);

      state.currentImage = image;
      state.lastStableImage = image;
      state.lastError = undefined;
      state.status = "running";
      await store.saveState(state);
      await store.appendEvent(spec.metadata.tenantId, "info", "deploy success", { image });
    } catch (e) {
      const err = e instanceof Error ? e.message : String(e);
      await store.appendEvent(spec.metadata.tenantId, "error", "deploy failed", { error: err });
      state.status = "failed";
      state.lastError = err;
      await store.saveState(state);

      if (options.autoRollback && state.lastStableImage && state.lastStableImage !== image) {
        await store.appendEvent(spec.metadata.tenantId, "warn", "auto rollback started", {
          image: state.lastStableImage,
        });
        try {
          state.status = "rollback_in_progress";
          await store.saveState(state);
          runFly(
            [
              "deploy",
              "-a",
              spec.spec.deployment.appName,
              "--config",
              renderedToml,
              "--image",
              state.lastStableImage,
            ],
            { retries: 2 },
          );
          await verifyHealth(
            spec.metadata.tenantId,
            spec.spec.runtime.publicBaseUrl,
            Math.min(options.healthTimeoutSec, 90),
            store,
          );
          state.currentImage = state.lastStableImage;
          state.status = "rolled_back";
          state.lastError = undefined;
          await store.saveState(state);
          await store.appendEvent(spec.metadata.tenantId, "warn", "auto rollback success", {
            image: state.lastStableImage,
          });
        } catch (rb) {
          const rbErr = rb instanceof Error ? rb.message : String(rb);
          state.status = "failed";
          state.lastError = `rollback failed: ${rbErr}`;
          await store.saveState(state);
          await store.appendEvent(spec.metadata.tenantId, "error", "auto rollback failed", { error: rbErr });
        }
      }

      throw e;
    }
  });
}

async function rollbackFlow(spec: TenantSpec, options: CliOptions, store: StateStore): Promise<void> {
  const outputDir = path.resolve(options.outputDir || `.fly-generated/${spec.metadata.tenantId}`);
  const renderedToml = path.join(outputDir, "fly.toml");
  await store.withTenantLock(spec.metadata.tenantId, options.lockTimeoutSec, async () => {
    const state = await store.loadState(spec.metadata.tenantId, spec.spec.deployment.appName);
    if (!state.lastStableImage) throw new Error("no lastStableImage recorded, cannot rollback");
    ensureFlyReady();
    renderFlyToml(options.templatePath, renderedToml, spec);
    await store.appendEvent(spec.metadata.tenantId, "warn", "manual rollback requested", {
      image: state.lastStableImage,
    });
    state.status = "rollback_in_progress";
    await store.saveState(state);
    runFly(
      ["deploy", "-a", spec.spec.deployment.appName, "--config", renderedToml, "--image", state.lastStableImage],
      { retries: 2 },
    );
    await verifyHealth(
      spec.metadata.tenantId,
      spec.spec.runtime.publicBaseUrl,
      Math.min(options.healthTimeoutSec, 90),
      store,
    );
    state.currentImage = state.lastStableImage;
    state.status = "rolled_back";
    state.lastError = undefined;
    await store.saveState(state);
    await store.appendEvent(spec.metadata.tenantId, "warn", "manual rollback success", {
      image: state.lastStableImage,
    });
  });
}

async function statusFlow(spec: TenantSpec, options: CliOptions, store: StateStore): Promise<void> {
  const state = await store.loadState(spec.metadata.tenantId, spec.spec.deployment.appName);
  const eventsTail = await store.getEventsTail(spec.metadata.tenantId, 10);
  console.log(
    JSON.stringify(
      {
        state,
        eventsTail,
      },
      null,
      2,
    ),
  );
}

async function main(): Promise<void> {
  let store: StateStore | null = null;
  try {
    const options = parseArgs(process.argv.slice(2));
    const spec = loadSpec(options.specPath);
    if (!existsSync(options.templatePath)) throw new Error(`template not found: ${options.templatePath}`);

    store = buildStateStore(options);
    await store.init();

    if (options.action === "status") {
      await statusFlow(spec, options, store);
      return;
    }
    if (options.action === "rollback") {
      await rollbackFlow(spec, options, store);
      return;
    }
    await deployFlow(spec, options, store);
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    process.exit(1);
  } finally {
    if (store) {
      await store.close();
    }
  }
}

main();
