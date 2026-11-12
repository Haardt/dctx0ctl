#!/usr/bin/env python3
"""Generate test-only typed Cinema calls from the centrally published model."""
import argparse
import json
from pathlib import Path

OWNERS = {
    'Catalog.FilmCatalogStore': 'store', 'Catalog.FilmCatalogApi': 'service',
    'Community.FilmPresence': 'store', 'Community.CommentStore': 'store',
    'Community.RatingStore': 'store', 'Community.FilmCommunityApi': 'service',
    'ScreeningPlanning.VotingStore': 'store', 'ScreeningPlanning.ScreeningPlanningApi': 'service',
    'ScreeningPlanning.VotingRules': 'winner',
}
ROOT = 'HomeCinema.MovieNight.'

def generate(bundle):
    data = bundle['model']['data']
    def native(ty):
        if ty.get('symbol_id'):
            definition = data[ty['symbol_id']]
            if definition.get('alias'): return native(definition['alias'])
            return 'string' if definition.get('variants') else 'domain.Record'
        return {'String':'string','UUID':'string','Timestamp':'string','Boolean':'bool','Int':'*big.Int','Float':'float64','List':'[]any','Option':'any','Unit':'any'}[ty['name']]
    rows = ['// Code generated from the central application bundle; DO NOT EDIT.', 'package contracttests', 'import ("cinema/domain"; "fmt")', 'func (fixture *cinemaFixture) production(invocation appInvocation, arguments []any)(any,error){', 'switch invocation.Subject {']
    methods = []
    uses_big = False
    for index, (subject, operation) in enumerate(sorted(bundle['model']['operations'].items())):
        if not subject.startswith(ROOT): raise ValueError('Foreign operation '+subject)
        owner, method = subject[len(ROOT):].rsplit('.',1)
        binding = OWNERS[owner]
        types = [native(p['ty']) for p in operation['parameters']]
        uses_big |= '*big.Int' in types
        rows += [f'case {json.dumps(subject)}:', f'if len(arguments)!={len(types)} {{return nil,fmt.Errorf("Incorrect generated arity")}}']
        for i, ty in enumerate(types):
            rows += [f'arg{i}, ok := arguments[{i}].({ty}); if !ok {{return nil,fmt.Errorf("Native argument {i} differs from generated {ty}")}}'] if ty != 'any' else [f'arg{i} := arguments[{i}]']
        params = ','.join(f'arg{i}' for i in range(len(types)))
        rows += [f'return fixture.operation{index}({params})']
        methods += [f'func (fixture *cinemaFixture) operation{index}({",".join(f"arg{i} {ty}" for i,ty in enumerate(types))})(any,error){{']
        args = '[]any{'+params+'}'
        if binding == 'winner':
            if method != 'candidateOrderWinner': raise ValueError('Unsupported winner method')
            methods += [f'return domain.CandidateOrderWinner({params}),nil']
        elif binding == 'store': methods += [f'return fixture.store.Invoke({json.dumps(subject)},{args})']
        else: methods += [f'if fixture.server!=nil {{return fixture.invokeHTTP({json.dumps(method)},{args})}}',f'return fixture.service.Invoke({json.dumps(method)},{args})']
        methods += ['}']
    if uses_big: rows[2] = rows[2].replace('"fmt"','"fmt"; "math/big"')
    rows += ['default: return nil,fmt.Errorf("Operation has no generated application binding: %s",invocation.Subject)', '}', '}']
    return '\n'.join(rows+methods)+'\n'

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle',type=Path,required=True)
    parser.add_argument('--project',type=Path,default=Path(__file__).resolve().parents[1])
    args=parser.parse_args()
    target=args.project/'contracttests/generated_facade_test.go'
    target.write_text(generate(json.loads(args.bundle.read_text())))
if __name__=='__main__': main()
