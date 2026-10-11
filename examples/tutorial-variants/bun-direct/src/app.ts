import {increment} from './board.ts';

export class Application {
  readonly server: ReturnType<typeof Bun.serve>;
  constructor(port = 0) {
    this.server = Bun.serve({hostname: '127.0.0.1', port, fetch: this.handle.bind(this)});
  }
  async handle(request: Request): Promise<Response> {
    if (new URL(request.url).pathname !== '/increment') return new Response('Not found', {status: 404});
    if (request.method !== 'POST') return new Response('Method not allowed', {status: 405});
    try {
      const body = await request.json() as {count?: unknown};
      if (typeof body.count !== 'string' || !/^-?\d+$/.test(body.count)) throw new TypeError('Expected an integer string');
      return Response.json({result: String(increment(BigInt(body.count)))});
    } catch (error) {
      if (error instanceof TypeError || error instanceof RangeError || error instanceof SyntaxError) {
        return Response.json({error: error.message}, {status: 400});
      }
      throw error;
    }
  }
  async close() { await this.server.stop(true); }
}
