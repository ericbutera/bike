import { spawn } from "node:child_process";
import { once } from "node:events";
import { setTimeout } from "node:timers/promises";

export async function run(command, args, options) {
  const child = spawn(command, args, { stdio: "inherit", ...options });
  const [code, signal] = await once(child, "exit");
  if (code !== 0) {
    throw new Error(`${command} failed: ${signal ?? code}`);
  }
}

export async function waitForService(url, child) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(`Service exited before ${url} became ready`);
    }
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(1000) });
      if (response.ok) return;
    } catch (error) {
      if (!["TimeoutError", "TypeError"].includes(error.name)) throw error;
    }
    await setTimeout(100);
  }
  throw new Error(`Service did not become ready: ${url}`);
}

export async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  child.kill("SIGTERM");
  const deadline = setTimeout(5000).then(() => child.kill("SIGKILL"));
  await Promise.race([exited, deadline]);
  await exited;
}
