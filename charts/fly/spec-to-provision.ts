#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import YAML from "yaml";

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

type CliOptions = {
  specPath: string;
  templatePath: string;
  secretsFile: string;
  outputDir?: string;
  domain?: string;
  exec: boolean;
  format: "shell" | "json";
  noSecretValidation: boolean;
};

function usage(): void {
  console.log(`Usage:
  pnpm tsx spec-to-provision.ts --spec tenant-spec.yaml --secrets-file /tmp/acme.env [options]

Required:
  --spec <path>           Tenant spec yaml path
  --secrets-file <path>   KEY=VALUE secrets file path

Optional:
  --template <path>       fly.toml template path (default: ./fly.toml.tmpl)
  --output-dir <path>     deploy output dir (default: .fly-generated/<tenantId>)
  --domain <hostname>     override domain
  --format <shell|json>   output format (default: shell)
  --no-secret-validation  skip checking required secret refs in secrets file
  --exec                  execute provision-tenant.sh directly
  -h, --help              show help
`);
}

function parseArgs(argv: string[]): CliOptions {
  let specPath = "";
  let templatePath = "./fly.toml.tmpl";
  let secretsFile = "";
  let outputDir: string | undefined;
  let domain: string | undefined;
  let exec = false;
  let format: "shell" | "json" = "shell";
  let noSecretValidation = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case "--spec":
        specPath = argv[++i] ?? "";
        break;
      case "--template":
        templatePath = argv[++i] ?? "";
        break;
      case "--secrets-file":
        secretsFile = argv[++i] ?? "";
        break;
      case "--output-dir":
        outputDir = argv[++i] ?? "";
        break;
      case "--domain":
        domain = argv[++i] ?? "";
        break;
      case "--format": {
        const v = argv[++i] ?? "";
        if (v !== "shell" && v !== "json") {
          throw new Error(`Invalid --format: ${v}`);
        }
        format = v;
        break;
      }
      case "--exec":
        exec = true;
        break;
      case "--no-secret-validation":
        noSecretValidation = true;
        break;
      case "-h":
      case "--help":
        usage();
        process.exit(0);
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!specPath || !secretsFile) {
    throw new Error("Both --spec and --secrets-file are required.");
  }

  return {
    specPath,
    templatePath,
    secretsFile,
    outputDir,
    domain,
    exec,
    format,
    noSecretValidation,
  };
}

function must(value: unknown, field: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${field} is required`);
  }
  return value.trim();
}

function mustNumber(value: unknown, field: string): number {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) throw new Error(`${field} must be a number`);
  return n;
}

function isValidHostname(hostname: string): boolean {
  if (hostname.length > 253) return false;
  if (hostname.includes("://")) return false;
  const labels = hostname.split(".");
  return labels.every((label) => /^[a-zA-Z0-9-]{1,63}$/.test(label) && !label.startsWith("-") && !label.endsWith("-"));
}

function isValidAppName(name: string): boolean {
  return /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/.test(name);
}

function isValidRegion(region: string): boolean {
  return /^[a-z]{3}$/.test(region);
}

function ensureHttpUrl(url: string, field: string): void {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`${field} must be a valid URL`);
  }
  if (!["http:", "https:"].includes(parsed.protocol)) {
    throw new Error(`${field} must use http or https`);
  }
}

function collectSecretRefs(node: unknown, found: Set<string> = new Set()): Set<string> {
  if (Array.isArray(node)) {
    for (const item of node) collectSecretRefs(item, found);
    return found;
  }
  if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
      if (key.endsWith("SecretRef") && typeof value === "string" && value.trim()) {
        found.add(value.trim());
      } else {
        collectSecretRefs(value, found);
      }
    }
  }
  return found;
}

function parseSecretsFile(secretsFile: string): Set<string> {
  if (!existsSync(secretsFile)) {
    throw new Error(`secrets file not found: ${secretsFile}`);
  }
  const lines = readFileSync(secretsFile, "utf8").split(/\r?\n/);
  const keys = new Set<string>();
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const match = trimmed.match(/^([A-Z][A-Z0-9_]*)=/);
    if (!match) {
      throw new Error(`invalid secrets line format: "${line}"`);
    }
    keys.add(match[1]);
  }
  return keys;
}

function loadSpec(specPath: string): TenantSpec {
  const raw = readFileSync(specPath, "utf8");
  const parsed = YAML.parse(raw) as TenantSpec;

  const tenantId = must(parsed?.metadata?.tenantId, "metadata.tenantId");
  const displayName = must(parsed?.metadata?.displayName, "metadata.displayName");
  const environment = must(parsed?.metadata?.environment, "metadata.environment");
  const appName = must(parsed?.spec?.deployment?.appName, "spec.deployment.appName");
  const region = must(parsed?.spec?.deployment?.region, "spec.deployment.region");
  const imageRepository = must(parsed?.spec?.deployment?.image?.repository, "spec.deployment.image.repository");
  const imageTag = must(parsed?.spec?.deployment?.image?.tag, "spec.deployment.image.tag");
  const vmSize = must(parsed?.spec?.deployment?.vmSize, "spec.deployment.vmSize");
  const hermesHome = must(parsed?.spec?.runtime?.hermesHome, "spec.runtime.hermesHome");
  const publicBaseUrl = must(parsed?.spec?.runtime?.publicBaseUrl, "spec.runtime.publicBaseUrl");
  const volumeName = must(parsed?.spec?.knowledgeBase?.volume?.name, "spec.knowledgeBase.volume.name");
  const minMachines = mustNumber(parsed?.spec?.deployment?.minMachines, "spec.deployment.minMachines");
  const internalPort = mustNumber(parsed?.spec?.deployment?.internalPort, "spec.deployment.internalPort");
  const volumeSizeGb = mustNumber(parsed?.spec?.knowledgeBase?.volume?.sizeGb, "spec.knowledgeBase.volume.sizeGb");
  const domain = parsed?.spec?.domain?.hostname;

  if (!isValidAppName(appName)) {
    throw new Error("spec.deployment.appName must match Fly app format: lowercase alnum/hyphen, 3-63 chars");
  }
  if (!isValidRegion(region)) {
    throw new Error("spec.deployment.region must be Fly 3-letter region code, e.g. hkg, sin, lax");
  }
  if (tenantId.length < 2 || tenantId.length > 64) {
    throw new Error("metadata.tenantId length must be between 2 and 64");
  }
  if (displayName.length > 120) {
    throw new Error("metadata.displayName must be <= 120 chars");
  }
  if (!/^[a-zA-Z0-9._/-]+$/.test(imageRepository)) {
    throw new Error("spec.deployment.image.repository contains invalid characters");
  }
  if (!/^[a-zA-Z0-9._-]+$/.test(imageTag)) {
    throw new Error("spec.deployment.image.tag contains invalid characters");
  }
  if (!/^[a-zA-Z0-9-]{3,63}$/.test(volumeName)) {
    throw new Error("spec.knowledgeBase.volume.name must match Fly volume naming");
  }
  if (!Number.isInteger(minMachines) || minMachines < 1 || minMachines > 20) {
    throw new Error("spec.deployment.minMachines must be integer between 1 and 20");
  }
  if (!Number.isInteger(internalPort) || internalPort < 1 || internalPort > 65535) {
    throw new Error("spec.deployment.internalPort must be integer between 1 and 65535");
  }
  if (!Number.isInteger(volumeSizeGb) || volumeSizeGb < 1 || volumeSizeGb > 500) {
    throw new Error("spec.knowledgeBase.volume.sizeGb must be integer between 1 and 500");
  }
  ensureHttpUrl(publicBaseUrl, "spec.runtime.publicBaseUrl");
  if (domain && !isValidHostname(domain)) {
    throw new Error("spec.domain.hostname must be a valid hostname without scheme");
  }
  if (!vmSize.trim()) {
    throw new Error("spec.deployment.vmSize cannot be empty");
  }
  if (!hermesHome.startsWith("/")) {
    throw new Error("spec.runtime.hermesHome must be an absolute unix path");
  }

  return parsed;
}

function toProvisionArgs(spec: TenantSpec, options: CliOptions): string[] {
  const d = spec.spec.deployment;
  const r = spec.spec.runtime;
  const kb = spec.spec.knowledgeBase;

  const image = `${d.image.repository}:${d.image.tag}`;
  const finalOutputDir = options.outputDir || `.fly-generated/${spec.metadata.tenantId}`;
  const finalDomain = options.domain || spec.spec.domain?.hostname;

  const args = [
    "--template", options.templatePath,
    "--app-name", d.appName,
    "--region", d.region,
    "--image", image,
    "--tenant-id", spec.metadata.tenantId,
    "--tenant-name", spec.metadata.displayName,
    "--environment", spec.metadata.environment,
    "--log-level", r.logLevel || "info",
    "--hermes-home", r.hermesHome,
    "--public-base-url", r.publicBaseUrl,
    "--volume-name", kb.volume.name,
    "--volume-size-gb", String(kb.volume.sizeGb),
    "--internal-port", String(d.internalPort),
    "--min-machines", String(d.minMachines),
    "--vm-size", d.vmSize,
    "--secrets-file", options.secretsFile,
    "--output-dir", finalOutputDir,
  ];

  if (finalDomain) {
    args.push("--domain", finalDomain);
  }

  return args;
}

function shellQuote(value: string): string {
  if (/^[a-zA-Z0-9._/:=-]+$/.test(value)) return value;
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

function main(): void {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (!existsSync(options.templatePath)) {
      throw new Error(`template file not found: ${options.templatePath}`);
    }
    const spec = loadSpec(options.specPath);
    if (!options.noSecretValidation) {
      const declared = Array.isArray(spec.spec?.secrets)
        ? spec.spec.secrets.filter((s): s is string => typeof s === "string" && s.trim().length > 0)
        : [];
      const discoveredRefs = collectSecretRefs(spec);
      for (const key of declared) discoveredRefs.add(key);
      const provided = parseSecretsFile(options.secretsFile);
      const missing = [...discoveredRefs].filter((s) => !provided.has(s));
      if (missing.length > 0) {
        throw new Error(`secrets file is missing required keys: ${missing.join(", ")}`);
      }
    }
    const args = toProvisionArgs(spec, options);
    const provisionScript = path.resolve(path.dirname(options.templatePath), "provision-tenant.sh");
    if (!existsSync(provisionScript)) {
      throw new Error(`provision script not found: ${provisionScript}`);
    }

    if (options.format === "json") {
      console.log(
        JSON.stringify(
          {
            script: provisionScript,
            args,
            command: [provisionScript, ...args],
          },
          null,
          2,
        ),
      );
    } else {
      const cmd = [shellQuote(provisionScript), ...args.map(shellQuote)].join(" ");
      console.log(cmd);
    }

    if (options.exec) {
      const run = spawnSync(provisionScript, args, { stdio: "inherit" });
      process.exit(run.status ?? 1);
    }
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    console.error(`Error: ${msg}`);
    usage();
    process.exit(1);
  }
}

main();
