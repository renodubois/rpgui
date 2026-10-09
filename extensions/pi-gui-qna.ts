import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

/** RPC-compatible replacement, not an addition to the global ask_user extension.
 * Uses only standard dialogs. A richer single native Q+A panel can come later.
 */
export default function guiQna(pi: ExtensionAPI) {
	pi.registerTool({
		name: "ask_user",
		label: "Q+A",
		description: "Ask the user a clarifying question with suggested options and an optional custom answer.",
		promptGuidelines: ["Use ask_user when user input is needed. Prefer 2–6 concise options; allow custom answers unless a listed choice is required."],
		executionMode: "sequential",
		parameters: Type.Object({
			question: Type.String(),
			options: Type.Array(Type.Object({ label: Type.String(), description: Type.Optional(Type.String()) })),
			allowCustom: Type.Optional(Type.Boolean()),
			customLabel: Type.Optional(Type.String()),
		}),
		async execute(_id, params, signal, _onUpdate, ctx) {
			const details = { question: params.question, options: params.options.map((o) => o.label), answer: null as string | null, cancelled: true };
			const cancelled = (text = "User cancelled the Q+A prompt.") => ({ content: [{ type: "text" as const, text }], details });
			if (!ctx.hasUI) return cancelled("Q+A requires an interactive client.");
			const allowCustom = params.allowCustom !== false;
			if (!params.options.length && !allowCustom) return cancelled("No answer options were provided and custom answers are disabled.");
			// Numbering makes duplicate labels and custom-label collisions unambiguous.
			const choices = params.options.map((o, i) => `${i + 1}. ${o.label}${o.description ? ` — ${o.description}` : ""}`);
			const custom = `${choices.length + 1}. ${params.customLabel?.trim() || "Write my own answer"}`;
			const choice = params.options.length
				? await ctx.ui.select(params.question, allowCustom ? [...choices, custom] : choices, { signal })
				: custom;
			if (signal?.aborted || choice === undefined) return cancelled();
			if (allowCustom && choice === custom) {
				const answer = (await ctx.ui.input(params.question, "Your answer", { signal }))?.trim();
				if (signal?.aborted || !answer) return cancelled();
				return { content: [{ type: "text" as const, text: `User wrote: ${answer}` }], details: { ...details, answer, cancelled: false, wasCustom: true } };
			}
			const index = choices.indexOf(choice);
			if (index < 0) return cancelled("Invalid Q+A response; no answer accepted.");
			const answer = params.options[index].label;
			return { content: [{ type: "text" as const, text: `User selected: ${index + 1}. ${answer}` }], details: { ...details, answer, cancelled: false, wasCustom: false, selectedIndex: index + 1 } };
		},
	});
}
