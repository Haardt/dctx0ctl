import { count, nextValue } from "../../../src/counter.ts";
import { invokeApplication, type ProjectAdapter, type Value } from "./contract.ts";

function decode(value: Value): unknown {
  switch (value.kind) {
    case "unit": return undefined;
    case "integer": {
      const number = Number(value.value);
      if (!Number.isSafeInteger(number) || BigInt(value.value) !== BigInt(number)) throw new Error("Integer cannot be represented losslessly");
      return number;
    }
    case "float": return Number(value.value);
    case "decimal": throw new Error("Application has no exact Decimal representation");
    case "boolean": case "string": return value.value;
    case "list": return value.value.map(decode);
    case "record": return Object.fromEntries(Object.entries(value.value).map(([key, item]) => [key, decode(item)]));
    case "option": return value.value === null ? undefined : decode(value.value);
    case "variant": case "failure": return value.value;
  }
}
function integer(value: number): Value {
  if (!Number.isSafeInteger(value)) throw new Error("Application returned an unrepresentable integer");
  return { kind: "integer", value: String(value) };
}
export const adapter: ProjectAdapter = {
  async describe() { return {
    protocol_version: 1,
    subjects: [
      { subject: "Adaptive.Count", role: "contract", artifact: "src/counter.ts", entry_point: "count" },
      { subject: "Adaptive.Counter.next", role: "operation", artifact: "src/counter.ts", entry_point: "nextValue" },
    ],
    artifacts: ["src/counter.ts"], capabilities: ["sessions", "values", "invocation", "events"],
    representations: { Int: "lossless safe JavaScript integer" },
  }; },
  async begin() { return {
    async validate(subject, value) {
      if (subject !== "Adaptive.Count") return { kind: "unavailable", reason: "No production factory" };
      let input: unknown;
      try { input = decode(value); } catch (error) { return { kind: "unavailable", reason: String(error) }; }
      try { return { kind: "accepted", value: integer(count(input)) }; }
      catch (error) { if (error instanceof RangeError) return { kind: "rejected", reason: error.message }; throw error; }
    },
    async invoke(invocation) {
      if (invocation.subject !== "Adaptive.Counter.next") return { kind: "unavailable", reason: "No application mapping" };
      const argument = decode(invocation.arguments[0]);
      if (typeof argument !== "number") throw new Error("Application argument representation mismatch");
      return invokeApplication(() => nextValue(argument), integer);
    },
    async close() {},
  }; },
};
