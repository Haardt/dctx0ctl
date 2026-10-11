import {expect,test} from 'bun:test';
import {copyTitle} from './copy.ts';
test('fragment operation preserves the actual validated title and inherits every refinement',()=>{
  expect(copyTitle('Read')).toBe('Read');expect(copyTitle('🧩')).toBe('🧩');
  for(const invalid of ['', ' '.repeat(3), 'x'.repeat(17), null, 1])expect(()=>copyTitle(invalid)).toThrow();
});
