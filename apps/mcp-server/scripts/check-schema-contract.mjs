import { assertRepositorySchemaContract } from "./lib/schema-contract.mjs";

const { latest } = assertRepositorySchemaContract();
console.log(`MCP schema compatibility covers desktop migration ${latest}`);
