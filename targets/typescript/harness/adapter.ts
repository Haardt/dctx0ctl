import type { ProjectAdapter } from './contract.ts';
/** Implement this testing adapter with real project calls. Retain existing code. */
export const adapter:ProjectAdapter = {
 async describe(){return {protocol_version:1,subjects:[],artifacts:[],capabilities:[],representations:{}};},
 async begin(){throw new Error('Project testing adapter is not implemented');},
};
