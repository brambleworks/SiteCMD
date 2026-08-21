# MCP Server Recovery Runbook

## What state matters

- The MCP server reads the main SiteCMD SQLite database and may update existing
  `fix_attempts` rows only: it records the first brief fetch and requests
  verification with the agent's summary.
- These writes never create an attempt, mark an issue fixed, or update another
  table. SiteCMD remains the authority that verifies the result.
- The database file is the critical artifact to protect and recover.

## Backup

1. Stop any process that may still be writing to the SiteCMD desktop database.
2. Copy the SQLite file to a safe backup location.
3. Keep at least one dated backup before schema changes or app upgrades.

## Restore

1. Stop the MCP server and the SiteCMD desktop app.
2. Replace the damaged database file with the most recent known-good backup.
3. Restart SiteCMD first, then restart the MCP server.
4. Validate that projects, scans, and issue lookups load correctly.

## Safe first move during an incident

1. Preserve the current broken database file before changing anything.
2. Restore from the latest backup copy.
3. Re-run the MCP server against the restored database by setting `SITECMD_DB_PATH` if needed.

## Notes

- If the default database path changes, update `.env.example` and this runbook together.
- If a fix attempt advances incorrectly, cancel it in SiteCMD and start a new
  attempt instead of editing SQLite directly.
- Keep all MCP writes confined to the existing fix-attempt workflow. Document
  recovery behavior before changing its allowed fields or transitions.
