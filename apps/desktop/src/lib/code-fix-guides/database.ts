import type { CodeFixGuideEntry } from "./types";

export const DATABASE_FIX_GUIDES: Record<string, CodeFixGuideEntry> = {
  "db-in-route": {
    effort: "moderate",
    effortMinutes: 20,
    lead: "Database queries and business logic are written directly inside a route handler instead of a separate reusable layer.",
    default: [
      "Review the handler first; a small route-local query can be clearer than a generic repository, and the real concern is duplicated policy or a route that mixes transport, business rules, and complex persistence. Where a boundary helps, extract the smallest policy-bearing operation into a typed service or repository method, keep authorization visible at the boundary, and test through the public route.",
    ],
  },
  "db-scattered-across-routes": {
    effort: "involved",
    effortMinutes: 45,
    lead: "The same kind of database query is duplicated across many separate routes instead of going through one shared place.",
    default: [
      "Confirm the matched routes actually duplicate policy rather than call a consistent client through thin handlers before treating placement as a defect. Where behavior has drifted, centralize the smallest policy-bearing unit (a shared query helper, service, or repository method), starting with repeated or security-sensitive operations, and add cross-route tests for authorization and tenant isolation.",
    ],
  },
  "db-index-hints-missing": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A query shape that usually benefits from a database index does not appear to have one, which can slow down as data grows.",
    default: [
      "Treat the detected query shape as a review lead, not proof an index is missing. Confirm the slow access path with the database's EXPLAIN tooling on the real SQL, check for an existing equivalent index, then add the chosen index through a reviewed migration (using the engine's online method on busy tables) and compare the before/after plan.",
    ],
  },
  "local-db-target-remote": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A local command appears to point at a remote database instead of an isolated one, risking accidental changes to shared data.",
    default: [
      "Confirm whether the remote target is intentional; a dedicated cloud development database can be valid, but production or shared staging should not be the default for local commands, tests, or destructive migrations. Point local work at an isolated database with least-privilege credentials, keep the URL in an ignored secret file, and guard destructive operations with an explicit target allowlist.",
    ],
  },
  "local-drizzle-migration-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The Drizzle schema in your code and the actual local database no longer agree, which can cause confusing runtime errors.",
    default: [
      "Decide which side is authoritative first, then run `npx drizzle-kit generate` to capture the schema change as a migration, review the SQL, and apply it with `npx drizzle-kit migrate`. Use `drizzle-kit push` only on a backed-up, disposable development database; do not use it to erase unexplained drift.",
    ],
  },
  "local-drizzle-migration-history-missing": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "This project uses Drizzle but has no migration history, so schema changes are not tracked or repeatable.",
    default: [
      "Run `npx drizzle-kit generate` to create an initial migration from your current schema, commit the generated `drizzle/` files as the source of truth, and run migrations through one controlled deployment step with locking rather than letting every application instance race to migrate on startup.",
    ],
  },
  "local-postgres-column-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A column in the local Postgres database no longer matches what the ORM schema expects, which can cause runtime errors.",
    default: [
      "Compare the ORM schema against the real database with `\\d tablename` in psql, `npx prisma db pull`, or `npx drizzle-kit introspect`, then generate an aligning migration with `npx prisma migrate dev` or `npx drizzle-kit generate`. Review it before applying; column type changes can cause data loss, so back up first if the table has data.",
    ],
  },
  "local-postgres-drizzle-migration-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The Drizzle schema and the actual local Postgres database no longer agree with each other, which can break queries.",
    default: [
      "Run `npx drizzle-kit generate` to capture the drift, review the SQL, and apply it with `npx drizzle-kit migrate`. Rebuild from migrations only if the database is confirmed disposable (back up useful fixtures first); otherwise reconcile with a reviewed forward migration, and never drop a database merely because the scanner found a mismatch.",
    ],
  },
  "local-postgres-drizzle-migration-history-missing": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "This Postgres project uses Drizzle but has no migration history recording how the schema actually got here.",
    default: [
      "Initialize Drizzle migrations with `npx drizzle-kit generate`, apply them to your local Postgres with `npx drizzle-kit migrate`, then commit the `drizzle/` directory and note the migration step in your setup docs so other developers run it too.",
    ],
  },
  "local-postgres-missing-composite-unique": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A join table in the local Postgres database allows the same relationship pair to be inserted more than once.",
    default: [
      "Add a composite unique constraint to join tables through a migration generated by your ORM tool, for example `ALTER TABLE user_roles ADD CONSTRAINT uq_user_roles UNIQUE (user_id, role_id);`. This blocks duplicate relationships, such as assigning the same role to a user twice, at the database level.",
    ],
  },
  "local-postgres-missing-foreign-keys": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A relationship between tables in the local Postgres database is not enforced, so it can point at rows that do not exist.",
    default: [
      "Add foreign key constraints through migrations, for example `ALTER TABLE orders ADD CONSTRAINT fk_orders_user FOREIGN KEY (user_id) REFERENCES users(id);`, choosing `ON DELETE CASCADE`, `SET NULL`, or `RESTRICT` based on your business rules. Verify by inserting a row with a non-existent reference; it should fail.",
    ],
  },
  "local-postgres-missing-unique-constraints": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A column that should hold unique values, like email, has no unique constraint enforcing that in the local database.",
    default: [
      "Add unique constraints, as a migration, to columns that should be unique (email, username, slug, API key), for example `ALTER TABLE users ADD CONSTRAINT uq_users_email UNIQUE (email);`. Find and resolve existing duplicates first with `SELECT email, COUNT(*) FROM users GROUP BY email HAVING COUNT(*) > 1;`.",
    ],
  },
  "local-postgres-nullable-relations": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A relationship column that should always point to a related row is allowed to be empty in the local database.",
    default: [
      "If the relationship is required, check for existing nulls with `SELECT COUNT(*) FROM orders WHERE user_id IS NULL;`, then add a migration that handles them (backfill, set a default, or remove orphans) before applying `ALTER TABLE orders ALTER COLUMN user_id SET NOT NULL;`.",
    ],
  },
  "local-postgres-prisma-migration-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The Prisma schema and the actual local Postgres database no longer agree with each other, which can break queries.",
    default: [
      "Determine whether the schema, migration history, or database holds the intended change, then run `npx prisma migrate dev` to generate an aligning migration. Review the SQL for table rewrites, locks, and column drops before applying, and use `prisma migrate reset` only on a positively identified disposable development database, accepting its full data loss.",
    ],
  },
  "local-postgres-prisma-migration-history-missing": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "This Postgres project uses Prisma but has no migration history recording how the schema actually got here.",
    default: [
      "Run `npx prisma migrate dev --name init` to create an initial migration from your current Prisma schema, commit the generated `prisma/migrations/` directory, and make your deploy process run `npx prisma migrate deploy` so pending migrations are applied.",
    ],
  },
  "local-postgres-schema-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The local Postgres database's real structure no longer matches what your ORM or migrations say it should be.",
    default: [
      "Capture the real schema with `npx prisma db pull`, `npx drizzle-kit introspect`, or `pg_dump --schema-only` and compare it against your ORM or migration definitions, then reconcile with a generated migration (`npx prisma migrate dev` or `npx drizzle-kit generate`), reviewing the diff carefully. For manual SQL projects, a diff tool like `pgdiff` or `migra` can produce the ALTER statements.",
    ],
  },
  "local-postgres-unindexed-lookups": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A query filters on a column in the local Postgres database that has no index, which can force a slow full scan.",
    default: [
      "Run `EXPLAIN ANALYZE` to confirm the query performs a sequential scan, then add an index on the filtered column(s) as a migration, for example `CREATE INDEX idx_tablename_column ON tablename(column);`. For a composite index, derive column order from the real query shape (a common starting point is equality predicates before the range or ORDER BY column) and re-check the plan.",
    ],
  },
  "local-postgres-unmigrated": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The local Postgres database has schema changes that were never captured in a migration file, so they are not repeatable.",
    default: [
      "Capture the schema changes in a new migration: `npx prisma migrate dev --name describe_change` for Prisma, `npx drizzle-kit generate` for Drizzle, or a numbered SQL file for raw projects. Verify by running the migrations against a fresh database and confirming they produce the expected schema from scratch.",
    ],
  },
  "local-prisma-migration-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The Prisma schema and the actual local database no longer agree with each other, which can cause confusing errors.",
    default: [
      "Run `npx prisma migrate dev` to detect the drift and generate a corrective migration, then confirm with `npx prisma migrate status`. If Prisma reports a history conflict, inspect the exact database changes before using `prisma migrate resolve`; marking a migration applied is a bookkeeping assertion, appropriate only when the database already has that migration's intended effect.",
    ],
  },
  "local-prisma-migration-history-missing": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "This project uses Prisma but has no migration history, so its schema changes are not tracked or repeatable.",
    default: [
      "Run `npx prisma migrate dev --name init` to baseline your schema as an initial migration, commit the `prisma/migrations/` directory, and add `npx prisma migrate deploy` to your deployment script. From then on, every schema change should produce a new migration file.",
    ],
  },
  "local-sqlite-column-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A column in the local SQLite database no longer matches what the ORM schema expects, which can cause runtime errors.",
    default: [
      "Compare the ORM schema against the database with `sqlite3 db.sqlite '.schema tablename'`, then write a migration suited to the deployed SQLite version: rename, add, and DROP COLUMN work under documented restrictions, but many constraint or type changes still require the table-rebuild procedure. Test on a backup copy with `PRAGMA foreign_key_check`, preserving indexes, triggers, and foreign keys through any rebuild.",
    ],
  },
  "local-sqlite-missing-composite-unique": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A join table in the local SQLite database allows the same relationship pair to be inserted more than once.",
    default: [
      "Add a composite unique constraint as a migration; for existing tables create a unique index, for example `CREATE UNIQUE INDEX idx_user_roles ON user_roles(user_id, role_id);`. Verify by inserting a duplicate combination, which should fail with a UNIQUE constraint error.",
    ],
  },
  "local-sqlite-missing-foreign-keys": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A relationship between tables in the local SQLite database is not enforced, so it can point at rows that do not exist.",
    default: [
      "SQLite cannot add foreign keys to existing tables via ALTER TABLE, so create a migration that recreates the table with the constraints in its CREATE TABLE statement, and ensure `PRAGMA foreign_keys = ON` on every application connection (the library default is normally off, so verify the effective value). Confirm an insert with a non-existent reference fails.",
    ],
  },
  "local-sqlite-missing-unique-constraints": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A column that should hold unique values has no unique constraint enforcing that in the local SQLite database.",
    default: [
      "Check for existing duplicates with `SELECT email, COUNT(*) FROM users GROUP BY email HAVING COUNT(*) > 1;` and fix them, then add a unique index as a committed migration, for example `CREATE UNIQUE INDEX idx_users_email ON users(email);`.",
    ],
  },
  "local-sqlite-nullable-relations": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A relationship column that should always point to a related row is allowed to be empty in the local SQLite database.",
    default: [
      "SQLite does not support `ALTER TABLE ... SET NOT NULL`, so write a migration that recreates the table: CREATE the new table with NOT NULL on the foreign key, INSERT INTO new FROM old, DROP old, then RENAME. First check for null rows with `SELECT COUNT(*) FROM orders WHERE user_id IS NULL;` and fix or remove them.",
    ],
  },
  "local-sqlite-schema-drift": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The local SQLite database's real structure no longer matches what your ORM or migrations say it should be.",
    default: [
      "Inspect the database with `sqlite3 db.sqlite '.schema'`, compare against your ORM definitions, and generate a corrective migration with `npx drizzle-kit generate` or `npx prisma migrate dev`. If the file is positively identified as disposable development state, copy it aside and rebuild from migrations to verify reproducibility; otherwise preserve the data and reconcile with a reviewed forward migration.",
    ],
  },
  "local-sqlite-unindexed-lookups": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A query filters on a column in the local SQLite database that has no index, which can force a slow full table scan.",
    default: [
      "Run `EXPLAIN QUERY PLAN SELECT ...` to confirm the query does a full table scan, then add an index on the filtered column as a migration, for example `CREATE INDEX idx_tablename_column ON tablename(column);`. Base the decision on the measured plan, expected growth, and read/write frequency, not table size alone.",
    ],
  },
  "local-sqlite-unmigrated": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The local SQLite database has schema changes that were never captured in a migration file, so they are not repeatable.",
    default: [
      "Capture the schema changes in a new migration: `npx drizzle-kit generate`, `npx prisma migrate dev --name describe_change`, or a numbered SQL file for raw projects. Verify by running all migrations against a fresh temporary SQLite file and comparing the resulting schema; back up the working database before applying rather than deleting it.",
    ],
  },
  "migration-workflow-missing": {
    effort: "moderate",
    effortMinutes: 20,
    lead: "This project changes its database schema without any tracked, repeatable migration process behind those changes.",
    default: [
      "Inventory the existing migration or schema-sync path first, including provider dashboards, infrastructure repositories, and deployment tooling outside the scanned tree; if the application has no schema-changing datastore, document that and close the finding instead of adding a framework.",
      "For a real gap, adopt the database or ORM's supported versioned migration flow, reconcile the deployed schema before creating a baseline (never mark unknown production changes as applied), and run changes through one controlled owner with locking, failure visibility, and verification on both a fresh database and a representative upgrade with existing data.",
    ],
  },
  "multi-write-no-transaction": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "Several related database writes happen outside of a transaction, so a failure partway through can leave data half-updated.",
    default: [
      "Confirm the matched writes belong to one atomic business invariant and that a called service or database API does not already own the transaction; independent writes should not be coupled to satisfy the finding. Where they do belong together, wrap them with the database or ORM's transaction API, pass the transaction-scoped client through every operation, keep network calls outside the transaction, and verify rollback by injecting a failure against a disposable database.",
    ],
  },
  "schema-join-missing-composite-unique": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A join table's foreign key columns have no combined uniqueness constraint, so the same relationship can be duplicated.",
    default: [
      "Add a composite unique constraint on the join table's foreign key columns so the same relationship cannot be inserted twice: `@@unique([userId, roleId])` in Prisma or `uniqueIndex().on(table.userId, table.roleId)` in Drizzle. Generate the migration and test by attempting to create a duplicate pair.",
    ],
  },
  "schema-join-missing-delete-intent": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "The schema never specifies what should happen to related rows when their parent record gets deleted.",
    default: [
      "Choose and document the parent-deletion behavior for each foreign key: CASCADE is often appropriate for purely owned join rows, RESTRICT protects records that must block deletion, and SET NULL is valid only when the relationship is optional. Encode the decision in the schema (Prisma `@relation(onDelete: Cascade)`, Drizzle `{ onDelete: 'cascade' }`, or SQL `ON DELETE ...`), never applying CASCADE mechanically to audit or billing records, and test each behavior from both parent sides.",
    ],
  },
  "schema-join-nullable-relations": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A relationship that should always be required is defined as optional in the schema, which can hide missing data.",
    default: [
      "Confirm both sides of the relationship are actually required; nullable foreign keys can be intentional for staged imports, polymorphic associations, or historical records, and that model should be documented rather than changed only to clear the finding. If both sides are required, inspect existing null rows, decide how to backfill, repair, quarantine, or delete each class of data, then tighten the schema through a reviewed migration with a safe backfill sequence.",
    ],
  },
  "schema-relation-missing-index": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A foreign key column used by a relationship has no index, which can slow down related lookups as the table grows.",
    default: [
      "Confirm the workload with EXPLAIN before adding anything; the referenced key is normally already indexed, and the candidate is the foreign-key column on the referencing (child) table, which PostgreSQL does not create automatically. When the query and data shape justify it, add the child-side index (`@@index([userId])` in Prisma, `index().on(table.userId)` in Drizzle, or `CREATE INDEX idx_orders_user_id ON orders(user_id)`), then compare the plan and account for the added storage and write cost.",
    ],
  },
  "supabase-open-policy": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A Supabase row security policy allows an operation with no real condition, so it applies to every row unconditionally.",
    default: [
      "Inspect the policy's operation, `TO` roles, and table grants first; `FOR SELECT USING (true)` can be correct for deliberately public data, while unconditional INSERT, UPDATE, or DELETE is much riskier. For private rows, replace `true` with the real owner, tenant, or role predicate (for example `FOR INSERT WITH CHECK (auth.uid() = user_id)`), then test as two users to confirm one cannot read or modify the other's private rows.",
    ],
  },
  "supabase-policy-not-auth-scoped": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A Supabase write policy does not check who the caller is, so it may not actually restrict access to a person's own rows.",
    default: [
      "Review the write policy together with its `TO` roles and table grants; a restricted database role may make the missing caller boundary intentional. If writes must be isolated, bind both `USING` and `WITH CHECK` as appropriate to `auth.uid()`, a tenant claim, or a narrowly scoped role, and test as two different users so each succeeds only on its permitted rows.",
    ],
  },
  "supabase-policy-operation-missing": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A database operation the frontend performs has no matching Supabase policy, so it is likely being silently denied.",
    default: [
      "Confirm the flagged operation is meant to run from the browser at all; with RLS enabled, a missing policy normally means that operation is denied, not exposed. Add only the operations the frontend needs with the real ownership or tenant boundary (SELECT/DELETE use `USING`, INSERT uses `WITH CHECK`, UPDATE commonly needs both), move privileged operations to a trusted server path, and test each one as the role the frontend uses.",
    ],
  },
  "supabase-policy-set-empty": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A Supabase table has row security turned on but no policies at all, so every request to it is denied by default.",
    default: [
      "Confirm whether the table should be reachable from the browser; with RLS enabled and no policies, anon and authenticated roles are default-denied, so the detected client call will usually return no rows or an error. If client access is intended, add only the required operation policies with explicit owner or tenant boundaries; if the table is server-only, remove the browser query instead. Verify both the allowed case and a different-user or unauthenticated denial.",
    ],
  },
  "supabase-rls-missing": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A Supabase table is reachable from the browser with row security turned off, leaving its raw grants as the only boundary.",
    default: [
      "Inventory the table's anon/authenticated grants and every browser operation first; without RLS those grants are the database boundary, and enabling RLS before defining the intended policies can immediately deny ordinary clients. In one reviewed migration, enable RLS and add only the required policies with explicit owner, tenant, or role predicates (using `WITH CHECK` for new row values), then test as unauthenticated, as two different users, and as the trusted server role.",
    ],
  },
  "supabase-service-role-client": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A Supabase credential that bypasses all row security may be reachable from code that runs in the browser.",
    default: [
      "Inspect production configuration and built client assets for both the service-role variable name and its resolved value; the name alone is a serious warning but does not prove a live credential shipped. Rotate any credential confirmed in a bundle, public variable, log, or repository, because the service role bypasses RLS and moving an exposed string without rotation does not contain the exposure.",
      "Use the project's publishable/anon key in the browser only with tested RLS policies, keep service-role use in a trusted server process behind authorization, then confirm the rotated old credential fails and browser requests as two users still enforce the intended boundary.",
    ],
  },
  "n-plus-one-query": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A database lookup runs once per item in a loop instead of once for the whole batch, so it slows down as items grow.",
    default: [
      "Measure query count at 1, 10, and a representative maximum item count to confirm the lookup really executes per iteration and is not already served by a request-scoped cache. If the count grows with items, switch to an eager relation/include, a set-based `WHERE id IN (...)` query with a lookup map, or a request-scoped batching loader, preserving ordering, authorization, and tenant-scope semantics, then re-measure to confirm the query count is bounded.",
    ],
  },
};
