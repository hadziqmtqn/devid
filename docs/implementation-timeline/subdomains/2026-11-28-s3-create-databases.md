# Phase 16 S3: accounts that may make databases

> Date: 2026-11-28 · Branch: `phase16-s3` · Plan and results: `docs/roadmap/phase-16.md`, "S3 results" · Decision: D-065

## What shipped

- **`db.rs`:**
  - `Admin::add_user` takes `create_databases`:
    - MySQL adds `GRANT ALL ON \`<name>\_%\`.*`;
    - PostgreSQL creates the role with `CREATEDB`;
    - MongoDB refuses before anything is made.
  - `Admin::allow_create` turns the right on or off for an existing account: on MySQL with `GRANT` or `REVOKE IF EXISTS` for each of its hosts, on PostgreSQL with `CREATEDB` or `NOCREATEDB`.
  - `Admin::can_create` reads it back from `mysql.db` or `pg_roles`.
  - `mysql_prefix` builds the grant's pattern, with the name's own `_` escaped.
- **`protocol.rs` and `server.rs`:**
  - `DbUserAdd` gets `create_databases`, which defaults to off;
  - the new `DbUserCreateDatabases` refuses the server's own accounts and names that have no account;
  - `DbUserList` reports `create_databases` for each account.
- **CLI:**
  - `devid db user add … --create-databases`;
  - `devid db user create-databases <engine> <name> on|off`;
  - `devid db user list` ends with a line for each account that has the right;
  - the output says what the account may make, and on MySQL the prefix for a tenancy package.
- **The TUI** sends `create_databases: false` until S5 adds its checkbox.
- **`scripts/db-e2e.sh`:** `tenants()` checks the right against the real MySQL and PostgreSQL builds:
  - the account makes, uses and drops `tenant_t1`;
  - it is refused `other` (MySQL) and another account's database;
  - it is refused again after `off`.

  On MongoDB, it checks the option is refused.
- **Tests:**
  - `crates/cli/tests/db.rs`: the statements, `off`, and the refusals;
  - `db.rs`: the escaped prefix;
  - the TUI's account test: the new field.

## Checked

See "S3 results" in `docs/roadmap/phase-16.md`.
