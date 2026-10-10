import { Hono } from "hono";
import { asClass, asValue, createContainer, InjectionMode, Lifetime } from "awilix";
import { boundedAmount, SequenceService } from "./service";

export function createHttpApplication() {
  const container = createContainer({ injectionMode: InjectionMode.CLASSIC, strict: true });
  // Register before resolution; the request scope owns its scoped service.
  container.register({
    increment: asValue(1),
    sequenceCalculator: asClass(SequenceService, { lifetime: Lifetime.SCOPED }),
  });
  const app = new Hono();
  app.post("/advance", async (context) => {
    const scope = container.createScope();
    try {
      const body = await context.req.json<{ amount: unknown }>();
      const amount = boundedAmount(body.amount);
      const calculator = scope.resolve<SequenceService>("sequenceCalculator");
      return context.json({ next: calculator.calculateNext(amount) });
    } catch (error) {
      if (error instanceof RangeError) return context.json({ error: error.message }, 400);
      throw error;
    } finally {
      await scope.dispose();
    }
  });
  return { app, container, close: () => container.dispose() };
}
