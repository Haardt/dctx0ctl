import {Board} from './src/entities.ts';
import {serveBoard} from './src/http.ts';

const board = new Board([
  {id:'00000000-0000-0000-0000-000000000001',title:'Spec lesen',status:'Open'},
  {id:'00000000-0000-0000-0000-000000000002',title:'Code prüfen',status:'Open'},
  {id:'00000000-0000-0000-0000-000000000003',title:'Board ansehen',status:'Done'},
]);
export const server = serveBoard(board, Number(process.env.PORT ?? '4173'));
console.log('MiniBoard: ' + server.url.href);
