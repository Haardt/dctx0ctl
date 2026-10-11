import {Application} from './app.ts';
export const application = new Application(Number(process.env.PORT ?? '4180'));
console.log('Counter API: ' + application.server.url);
