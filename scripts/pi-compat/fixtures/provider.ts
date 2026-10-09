import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { createAssistantMessageEventStream } from "@earendil-works/pi-ai";
import { existsSync, writeFileSync } from "node:fs";

/** Deterministic in-process provider. Never opens a socket or reads credentials. */
export default function fixture(pi: ExtensionAPI) {
	let release: (() => void) | undefined;
	let transientFailed = false;
	pi.registerCommand("fixture-release", { handler: async () => { release?.(); } });
	pi.registerCommand("fixture-cwd", { handler: async (_args, ctx) => { pi.appendEntry("fixture-cwd", { cwd: ctx.cwd }); } });
	pi.registerCommand("fixture-dialog", {
		handler: async (args, ctx) => {
			const answer = await ctx.ui.select("Timeout probe", ["yes"], args === "timeout" ? { timeout: 25 } : { signal: ctx.signal });
			pi.appendEntry("fixture-dialog", { answer: answer ?? null });
		},
	});
	pi.on("session_shutdown", () => { pi.appendEntry("fixture-shutdown", { closed: true }); });
	pi.registerProvider("gui-fixture", {
		api: "gui-fixture-api",
		apiKey: "disposable-fixture-not-a-credential",
		baseUrl: "http://127.0.0.1:1/never-used",
		models: [{ id: "controlled", name: "Controlled fixture", reasoning: false, input: ["text"], cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }, contextWindow: 200000, maxTokens: 4096 }],
		streamSimple(model, context, options) {
			const stream = createAssistantMessageEventStream();
			const output = { role: "assistant" as const, content: [] as any[], api: model.api, provider: model.provider, model: model.id, timestamp: Date.now(), stopReason: "pending" as any,
				usage: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0, totalTokens: 2, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } };
			void (async () => {
				try {
					const last = context.messages.at(-1);
					const rawText = typeof last?.content === "string" ? last.content : last?.content.filter((c) => c.type === "text").map((c) => c.text).join("");
					// Production foreground children wrap the assigned task; async children do not.
					const text = rawText?.replace(/^Task: /, "");
					stream.push({ type: "start", partial: output });
					const noncooperative = text?.startsWith("noncoop-gate:") === true;
					const gate = last?.role === "user" && /^(?:gate|noncoop-gate):(\/[^\s]+)/.exec(text ?? "")?.[1];
					if (gate) {
						writeFileSync(`${gate}.started`, JSON.stringify({ pid: process.pid, model: `${model.provider}/${model.id}` }));
						const deadline = Date.now() + 15000;
						const aborted = () => writeFileSync(`${gate}.aborted`, "observed");
						options?.signal?.addEventListener("abort", aborted, { once: true });
						while (!existsSync(gate) && (noncooperative || !options?.signal?.aborted) && Date.now() < deadline)
							await new Promise((resolve) => setTimeout(resolve, 20));
						options?.signal?.removeEventListener("abort", aborted);
						if (!existsSync(gate) && !options?.signal?.aborted) throw new Error("Fixture gate deadline");
					}
					if (last?.role === "user" && text?.includes("hold")) {
						await new Promise<void>((resolve) => {
							const finish = () => { options?.signal?.removeEventListener("abort", finish); release = undefined; resolve(); };
							release = finish;
							options?.signal?.addEventListener("abort", finish, { once: true });
							if (options?.signal?.aborted) finish();
						});
					}
					if (options?.signal?.aborted) throw new Error("Fixture aborted");
					if (last?.role === "user" && (text?.startsWith("qna:") || text?.startsWith("subagent:") || text?.startsWith("tool:"))) {
						const isChild = text.startsWith("subagent:");
						const requested = text.startsWith("tool:") ? JSON.parse(text.slice(5)) : undefined;
						const call = { type: "toolCall" as const, id: "fixture-ask", name: requested?.name ?? (isChild ? "subagent" : "ask_user"), arguments: requested?.arguments ?? JSON.parse(text.slice(isChild ? 9 : 4)) };
						output.content.push(call);
						stream.push({ type: "toolcall_start", contentIndex: 0, partial: output });
						stream.push({ type: "toolcall_end", contentIndex: 0, toolCall: call, partial: output });
						output.stopReason = "toolUse";
					} else {
						if (text === "provider-error") throw new Error("Fixture permanent failure");
						if (text === "retry-once" && !transientFailed) { transientFailed = true; throw new Error("503 service unavailable: controlled transient failure"); }
						output.content.push({ type: "text", text: "" });
						stream.push({ type: "text_start", contentIndex: 0, partial: output });
						output.content[0].text = "fixture\u2028answer\u2029🦀";
						stream.push({ type: "text_delta", contentIndex: 0, delta: output.content[0].text, partial: output });
						stream.push({ type: "text_end", contentIndex: 0, content: output.content[0].text, partial: output });
						output.stopReason = "stop";
					}
					stream.push({ type: "done", reason: output.stopReason, message: output });
				} catch (error) {
					output.stopReason = options?.signal?.aborted ? "aborted" : "error";
					stream.push({ type: "error", reason: output.stopReason, error: { ...output, errorMessage: String(error) } });
				} finally { stream.end(); }
			})();
			return stream;
		},
	});
}
