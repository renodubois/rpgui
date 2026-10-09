import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

/** Probe-only adapter. No stdout writes, terminal scraping, internal executors,
 * injected child factories or package test hooks. Notifications are a supported
 * RPC envelope, NOT a proposed final GUI wire protocol. */
export default function bridge(pi: ExtensionAPI) {
	let context: ExtensionContext | undefined;
	const disposers: (() => void)[] = [];
	const publish = (kind: string, data: unknown) => context?.ui.notify(JSON.stringify({ bridge: "pi-gui-subagents-probe", version: 1, kind, data }), "info");
	for (const event of ["subagents:rpc:v1:ready", "subagent:async-started", "subagent:async-complete", "subagent:process-terminal", "subagent:child-status", "subagent:foreground-complete"])
		disposers.push(pi.events.on(event, (data) => publish(event, data)));
	pi.on("session_start", (_event, ctx) => { context = ctx; });
	pi.registerCommand("fixture-subagents", {
		handler: async (args, ctx) => {
			context = ctx;
			const request = JSON.parse(args);
			if (request.version !== 1 || typeof request.requestId !== "string") throw new Error("Invalid probe request");
			await new Promise<void>((resolve, reject) => {
				const event = `subagents:rpc:v1:reply:${request.requestId}`;
				const timer = setTimeout(() => { off(); reject(new Error("Subagents RPC deadline")); }, 10000);
				const off = pi.events.on(event, (reply) => { clearTimeout(timer); off(); publish("reply", reply); resolve(); });
				pi.events.emit("subagents:rpc:v1:request", request);
			});
		},
	});
	pi.on("session_shutdown", () => { for (const dispose of disposers) dispose(); context = undefined; });
}
