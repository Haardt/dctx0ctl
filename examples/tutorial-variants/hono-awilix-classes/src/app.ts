import {asClass, createContainer, InjectionMode, Lifetime, type AwilixContainer} from 'awilix';
import {Hono, type Context} from 'hono';
import {CounterService} from './board.ts';

export class CounterController {
  constructor(private readonly counter: CounterService) {}
  async increment(context: Context): Promise<Response> {
    try {
      const body = await context.req.json<{count?: unknown}>();
      if (typeof body.count !== 'string' || !/^-?\d+$/.test(body.count)) throw new TypeError('Expected an integer string');
      return context.json({result: String(this.counter.increment(BigInt(body.count)))});
    } catch (error) {
      if (error instanceof TypeError || error instanceof RangeError || error instanceof SyntaxError) {
        return context.json({error: error.message}, 400);
      }
      throw error;
    }
  }
}

export class Application {
  readonly container: AwilixContainer;
  readonly app: Hono;
  readonly server: ReturnType<typeof Bun.serve>;
  constructor(port = 0) {
    this.container = createContainer({injectionMode: InjectionMode.CLASSIC, strict: true});
    this.container.register({
      counter: asClass(CounterService, {lifetime: Lifetime.SCOPED}).disposer(CounterService.dispose),
      controller: asClass(CounterController, {lifetime: Lifetime.SCOPED}),
    });
    this.app = new Hono();
    this.app.post('/increment', this.increment.bind(this));
    this.server = Bun.serve({hostname: '127.0.0.1', port, fetch: this.app.fetch.bind(this.app)});
  }
  async increment(context: Context): Promise<Response> {
    const scope = this.container.createScope();
    try { return await scope.resolve<CounterController>('controller').increment(context); }
    finally { await scope.dispose(); }
  }
  async close() {
    try { await this.server.stop(true); }
    finally { await this.container.dispose(); }
  }
}
