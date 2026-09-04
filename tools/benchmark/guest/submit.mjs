import { bridgeRequest } from "./bridge-client.mjs";

const result = await bridgeRequest(process.argv[2], "/submit", {
  summary: process.argv.slice(3).join(" "),
});
console.log(JSON.stringify(result));
