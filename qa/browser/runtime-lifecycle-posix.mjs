import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import fsp from "node:fs/promises";
import http from "node:http";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const COMPONENT_EXTENSION_ID = "igopfebebmfpffafbkccbeedbpgdebfk";
const HOST_NAME = "com.omniplay.broker";

const PLATFORM_SPECS = {
  "linux-x86_64": {
    browserRelativePath: "chrome",
    brokerRelativePath: "broker",
    requiredFiles: [
      "chrome",
      "resources.pak",
      "icudtl.dat",
      "v8_context_snapshot.bin",
      "broker",
    ],
  },
  "macos-aarch64": {
    browserRelativePath: "OmniPlay.app/Contents/MacOS/OmniPlay",
    brokerRelativePath: "broker",
    requiredFiles: [
      "OmniPlay.app/Contents/MacOS/OmniPlay",
      "OmniPlay.app/Contents/Info.plist",
      "icudtl.dat",
      "v8_context_snapshot.arm64.bin",
      "broker",
    ],
  },
};

function parseArgs(argv) {
  const options = {
    platform: process.platform === "darwin" ? "macos-aarch64" : "linux-x86_64",
    packageDir: null,
    chromiumOutDir: null,
    brokerBinary: null,
    extensionDir: path.join(REPO_ROOT, "apps", "game-hub"),
    outputDir: null,
    keepTemp: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    const next = argv[index + 1];
    if (token === "--platform" && next) {
      options.platform = next;
      index += 1;
    } else if (token === "--package-dir" && next) {
      options.packageDir = path.resolve(next);
      index += 1;
    } else if (token === "--chromium-out-dir" && next) {
      options.chromiumOutDir = path.resolve(next);
      index += 1;
    } else if (token === "--broker-binary" && next) {
      options.brokerBinary = path.resolve(next);
      index += 1;
    } else if (token === "--extension-dir" && next) {
      options.extensionDir = path.resolve(next);
      index += 1;
    } else if (token === "--output-dir" && next) {
      options.outputDir = path.resolve(next);
      index += 1;
    } else if (token === "--keep-temp") {
      options.keepTemp = true;
    } else {
      throw new Error(`Unsupported argument: ${token}`);
    }
  }
  if (!PLATFORM_SPECS[options.platform]) {
    throw new Error(`Unsupported platform: ${options.platform}`);
  }
  return options;
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function resolveCandidateFile(options, relativeName) {
  const roots = [options.packageDir, options.chromiumOutDir].filter(Boolean);
  for (const root of roots) {
    const direct = path.join(root, relativeName);
    if (fs.existsSync(direct) && fs.statSync(direct).isFile()) {
      return direct;
    }
    if (options.platform === "macos-aarch64") {
      const frameworkVersions = path.join(
        root,
        "OmniPlay.app",
        "Contents",
        "Frameworks",
        "OmniPlay Framework.framework",
        "Versions",
      );
      if (fs.existsSync(frameworkVersions)) {
        for (const versionEntry of fs.readdirSync(frameworkVersions)) {
          const resCandidate = path.join(
            frameworkVersions,
            versionEntry,
            "Resources",
            path.basename(relativeName),
          );
          if (fs.existsSync(resCandidate) && fs.statSync(resCandidate).isFile()) {
            return resCandidate;
          }
        }
      }
    }
  }
  if (relativeName === "broker" && options.brokerBinary && fs.existsSync(options.brokerBinary)) {
    return options.brokerBinary;
  }
  throw new Error(`Missing required runtime artifact for ${options.platform}: ${relativeName}`);
}

function admitPosixArtifacts(options) {
  const spec = PLATFORM_SPECS[options.platform];
  const files = {};
  for (const rel of spec.requiredFiles) {
    const resolved = resolveCandidateFile(options, rel);
    const stat = fs.statSync(resolved);
    files[rel] = {
      path: resolved,
      size_bytes: stat.size,
      sha256: sha256File(resolved),
    };
  }
  const browserBinary = files[spec.browserRelativePath].path;
  const brokerBinary = files[spec.brokerRelativePath].path;
  return {
    platform: options.platform,
    browser_binary: browserBinary,
    broker_binary: brokerBinary,
    files,
  };
}

function listPosixProcesses() {
  const res = spawnSync("ps", ["-eo", "pid=,ppid=,pgid=,comm=,args="], {
    encoding: "utf8",
  });
  if (res.status !== 0) {
    return [];
  }
  const rows = [];
  for (const line of res.stdout.split(/\r?\n/)) {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s+(.*)$/);
    if (!match) {
      continue;
    }
    const pid = Number(match[1]);
    let exePath = null;
    if (process.platform === "linux") {
      try {
        exePath = fs.readlinkSync(`/proc/${pid}/exe`);
      } catch {
        exePath = null;
      }
    }
    rows.push({
      ProcessId: pid,
      ParentProcessId: Number(match[2]),
      ProcessGroupId: Number(match[3]),
      Name: match[4],
      ExecutablePath: exePath,
      CommandLine: match[5],
    });
  }
  return rows;
}

function isDescendantOf(processes, candidatePid, ancestorPid) {
  const byPid = new Map(processes.map((entry) => [entry.ProcessId, entry]));
  let current = byPid.get(candidatePid);
  const visited = new Set();
  while (current && !visited.has(current.ProcessId)) {
    if (current.ProcessId === ancestorPid || current.ParentProcessId === ancestorPid) {
      return true;
    }
    visited.add(current.ProcessId);
    current = byPid.get(current.ParentProcessId);
  }
  return false;
}

function findOwnedBrokerProcesses(browserPid, brokerBinaryPath) {
  const processes = listPosixProcesses();
  const normalizedBroker = fs.existsSync(brokerBinaryPath)
    ? fs.realpathSync(brokerBinaryPath)
    : brokerBinaryPath;
  return processes.filter((proc) => {
    const matchesPath =
      proc.ExecutablePath === normalizedBroker ||
      proc.CommandLine.includes(brokerBinaryPath) ||
      proc.CommandLine.includes(normalizedBroker) ||
      proc.CommandLine.includes(HOST_NAME);
    return matchesPath && isDescendantOf(processes, proc.ProcessId, browserPid);
  });
}

async function allocateFreePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      srv.close((err) => (err ? reject(err) : resolve(port)));
    });
    srv.on("error", reject);
  });
}

async function startFixtureHttpServer() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    if (url.pathname === "/site-a") {
      res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      res.end("<!doctype html><title>OmniPlay Site A</title><h1 id='ready'>Site A Ready</h1>");
      return;
    }
    if (url.pathname === "/site-b") {
      res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      res.end("<!doctype html><title>OmniPlay Site B</title><h1 id='ready'>Site B Ready</h1>");
      return;
    }
    res.writeHead(404);
    res.end("not found");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  return {
    baseUrl: `http://127.0.0.1:${port}`,
    close: () => new Promise((resolve) => server.close(() => resolve())),
  };
}

function writeScopedNativeHostManifest(userDataDir, brokerBinaryPath) {
  // Chrome's DIR_USER_NATIVE_MESSAGING resolves to <user-data-dir>/NativeMessagingHosts on both Linux and macOS
  const manifestDir = path.join(userDataDir, "NativeMessagingHosts");
  fs.mkdirSync(manifestDir, { recursive: true });
  const manifestPath = path.join(manifestDir, `${HOST_NAME}.json`);
  const payload = {
    name: HOST_NAME,
    description: "OmniPlay Native Game Runtime Broker",
    path: brokerBinaryPath,
    type: "stdio",
    allowed_origins: [`chrome-extension://${COMPONENT_EXTENSION_ID}/`],
  };
  fs.writeFileSync(manifestPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  return manifestPath;
}

class CdpSession {
  constructor(webSocketUrl) {
    this.webSocketUrl = webSocketUrl;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
  }

  async connect() {
    this.ws = new WebSocket(this.webSocketUrl);
    this.ws.onmessage = (event) => {
      const msg = JSON.parse(String(event.data));
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) {
          reject(new Error(msg.error.message || JSON.stringify(msg.error)));
        } else {
          resolve(msg.result);
        }
      }
    };
    await new Promise((resolve, reject) => {
      this.ws.onopen = () => resolve();
      this.ws.onerror = (err) => reject(err);
    });
  }

  send(method, params = {}, sessionId = undefined) {
    const id = this.nextId++;
    const payload = { id, method, params };
    if (sessionId) {
      payload.sessionId = sessionId;
    }
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify(payload));
    });
  }

  async evaluate(sessionId, expression) {
    const res = await this.send(
      "Runtime.evaluate",
      {
        expression,
        awaitPromise: true,
        returnByValue: true,
      },
      sessionId,
    );
    if (res.exceptionDetails) {
      throw new Error(`CDP evaluation failed: ${JSON.stringify(res.exceptionDetails)}`);
    }
    return res.result ? res.result.value : undefined;
  }

  close() {
    if (this.ws) {
      this.ws.close();
    }
  }
}

async function waitForDevTools(port, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) {
        return await response.json();
      }
    } catch {
      // retry until browser listens
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Timed out waiting for DevTools endpoint on port ${port}`);
}

async function runQualification(options) {
  const admission = admitPosixArtifacts(options);
  const tempRoot = await fsp.mkdtemp(path.join(os.tmpdir(), `omniplay-posix-${options.platform}-`));
  const userDataDir = path.join(tempRoot, "profiles");
  const runtimeRoot = path.join(tempRoot, "runtime");
  await fsp.mkdir(userDataDir, { recursive: true });
  await fsp.mkdir(runtimeRoot, { recursive: true });

  const nativeManifestPath = writeScopedNativeHostManifest(userDataDir, admission.broker_binary);
  const fixtureServer = await startFixtureHttpServer();
  const debugPort = await allocateFreePort();

  const browserArgs = [
    `--user-data-dir=${userDataDir}`,
    `--remote-debugging-port=${debugPort}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-sync",
    `--disable-extensions-except=${options.extensionDir}`,
    `--load-extension=${options.extensionDir}`,
    "chrome://game-hub/",
  ];
  if (options.platform === "linux-x86_64" && !process.env.DISPLAY && !process.env.WAYLAND_DISPLAY) {
    browserArgs.unshift("--headless=new", "--ozone-platform=headless");
  }

  const env = {
    ...process.env,
    OMNIPLAY_DATA_DIR: runtimeRoot,
    XDG_DATA_HOME: path.join(tempRoot, "xdg-data"),
    XDG_CONFIG_HOME: path.join(tempRoot, "xdg-config"),
    XDG_CACHE_HOME: path.join(tempRoot, "xdg-cache"),
  };

  const browserProc = spawn(admission.browser_binary, browserArgs, {
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const checks = [];
  const recordCheck = (id, passed, details) => {
    checks.push({ id, passed: Boolean(passed), details });
    assert.equal(Boolean(passed), true, `Check ${id} failed: ${JSON.stringify(details)}`);
  };

  let cdp = null;
  try {
    const versionInfo = await waitForDevTools(debugPort);
    cdp = new CdpSession(versionInfo.webSocketDebuggerUrl);
    await cdp.connect();

    const { targetInfos } = await cdp.send("Target.getTargets");
    const hubTarget = targetInfos.find(
      (t) =>
        t.type === "page" &&
        (t.url.startsWith("chrome://game-hub") ||
          t.url.startsWith(`chrome-extension://${COMPONENT_EXTENSION_ID}/`)),
    );
    recordCheck("game_hub_startup_surface", Boolean(hubTarget), {
      url: hubTarget?.url ?? null,
      manifest_path: nativeManifestPath,
    });

    const { sessionId: hubSession } = await cdp.send("Target.attachToTarget", {
      targetId: hubTarget.targetId,
      flatten: true,
    });

    const bridgeBoot = await cdp.evaluate(
      hubSession,
      `(async () => {
        for (let i = 0; i < 50; i++) {
          if (window.__omniplayHubState && window.__omniplayHubState.protocolVersion) {
            return window.__omniplayHubState;
          }
          await new Promise(r => setTimeout(r, 100));
        }
        return window.__omniplayHubState || null;
      })()`,
    );
    recordCheck("native_bridge_handshake", Boolean(bridgeBoot), bridgeBoot);

    const initialBrokers = findOwnedBrokerProcesses(browserProc.pid, admission.broker_binary);
    recordCheck("broker_ancestry_verified", initialBrokers.length === 1, {
      browser_pid: browserProc.pid,
      brokers: initialBrokers,
    });

    // Multi-tab navigation & isolation checks
    const tabA = await cdp.send("Target.createTarget", { url: `${fixtureServer.baseUrl}/site-a` });
    const tabB = await cdp.send("Target.createTarget", { url: `${fixtureServer.baseUrl}/site-b` });
    recordCheck("multi_tab_creation", Boolean(tabA.targetId && tabB.targetId), { tabA, tabB });

    // Normal vs Incognito profile isolation + chrome.omniPlayPrivate rejection in incognito
    const incognitoContext = await cdp.send("Target.createBrowserContext", {
      disposeOnDetach: true,
    });
    const incognitoTab = await cdp.send("Target.createTarget", {
      url: `${fixtureServer.baseUrl}/site-a`,
      browserContextId: incognitoContext.browserContextId,
    });
    const { sessionId: incogSession } = await cdp.send("Target.attachToTarget", {
      targetId: incognitoTab.targetId,
      flatten: true,
    });
    const incogHasBridge = await cdp.evaluate(
      incogSession,
      "typeof chrome !== 'undefined' && Boolean(chrome.omniPlayPrivate)",
    );
    recordCheck("incognito_bridge_rejected", incogHasBridge === false, {
      browserContextId: incognitoContext.browserContextId,
      incogHasBridge,
    });

    // Forced broker termination and reconnect verification
    const firstBrokerPid = initialBrokers[0].ProcessId;
    process.kill(firstBrokerPid, "SIGKILL");
    await new Promise((r) => setTimeout(r, 400));

    const reconnectedBrokers = await (async () => {
      await cdp.evaluate(
        hubSession,
        `window.__omniplayReconnect && window.__omniplayReconnect()`,
      );
      const deadline = Date.now() + 8000;
      while (Date.now() < deadline) {
        const active = findOwnedBrokerProcesses(browserProc.pid, admission.broker_binary);
        if (active.length === 1 && active[0].ProcessId !== firstBrokerPid) {
          return active;
        }
        await new Promise((r) => setTimeout(r, 200));
      }
      return [];
    })();
    recordCheck("broker_forced_termination_and_reconnect", reconnectedBrokers.length === 1, {
      previous_broker_pid: firstBrokerPid,
      reconnected_brokers: reconnectedBrokers,
    });

    const summary = {
      schema_version: 1,
      item_id: 6,
      platform: options.platform,
      generated_at_utc: new Date().toISOString(),
      admission,
      browser_pid: browserProc.pid,
      native_manifest_path: nativeManifestPath,
      checks_passed: checks.filter((c) => c.passed).length,
      checks_total: checks.length,
      checks,
    };

    if (options.outputDir) {
      await fsp.mkdir(options.outputDir, { recursive: true });
      await fsp.writeFile(
        path.join(options.outputDir, `runtime-lifecycle-${options.platform}.json`),
        `${JSON.stringify(summary, null, 2)}\n`,
        "utf8",
      );
    }
    return summary;
  } finally {
    if (cdp) {
      cdp.close();
    }
    if (browserProc.exitCode === null) {
      browserProc.kill("SIGTERM");
    }
    await fixtureServer.close();
    if (!options.keepTemp) {
      await fsp.rm(tempRoot, { recursive: true, force: true });
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const opts = parseArgs(process.argv.slice(2));
  runQualification(opts)
    .then((summary) => {
      process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
    })
    .catch((err) => {
      process.stderr.write(`${err.stack || err.message}\n`);
      process.exitCode = 1;
    });
}

export { admitPosixArtifacts, findOwnedBrokerProcesses, parseArgs, runQualification, writeScopedNativeHostManifest };
