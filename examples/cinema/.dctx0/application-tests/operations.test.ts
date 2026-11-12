import {test} from 'bun:test';
import {operationTest,stateChangeTest,assertionTest} from './library/index.ts';
import {winnerFixture,filmSaveFixture} from './operation-fixtures.ts';
import {applicationTest} from './session.ts';
test('CinemaWinner',()=>applicationTest((client,model)=>operationTest(client,'CinemaWinner',()=>winnerFixture(model))),120000);
test('CinemaFilmSave',()=>applicationTest((client,model)=>stateChangeTest(client,'CinemaFilmSave',()=>filmSaveFixture(model))),120000);

test('CinemaWinnerAssertions',()=>applicationTest((client,model)=>assertionTest(client,'CinemaWinnerAssertions',()=>winnerFixture(model))),120000);
