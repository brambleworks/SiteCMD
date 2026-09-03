//! Dependency resolution: which declarations a lockfile actually covers,
//! which imports the project already provides, and which usage evidence lives
//! outside the issue-emitting source walk.

use super::*;

/// Collect the producer rule ids a scan emitted, for readable assertions.
fn issue_ids(report: &CodeScanReport) -> Vec<String> {
    report.issues.iter().map(|issue| issue.id.clone()).collect()
}

/// An npm v3 workspace: a peer dependency, a `"*"` reference to a sibling
/// package, a workspace link, and an install that lives under the declaring
/// member are all resolved, so none of them is manifest/lockfile drift.
#[test]
fn workspace_peers_links_and_member_local_installs_are_not_lockfile_mismatches() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "monorepo-root",
          "private": true,
          "workspaces": ["apps/*", "packages/*"]
        }"#,
    );
    write_file(
        temp.path(),
        "apps/docs/package.json",
        r#"{
          "name": "@acme/docs",
          "dependencies": {
            "next": "^15.0.0",
            "@acme/lib": "*"
          },
          "peerDependencies": {
            "react": "^19.0.0"
          },
          "optionalDependencies": {
            "fsevents": "^2.3.3"
          }
        }"#,
    );
    write_file(
        temp.path(),
        "packages/lib/package.json",
        r#"{ "name": "@acme/lib", "version": "0.0.0" }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{
          "lockfileVersion": 3,
          "packages": {
            "": { "name": "monorepo-root" },
            "packages/lib": { "name": "@acme/lib", "version": "0.0.0" },
            "node_modules/@acme/lib": { "resolved": "packages/lib", "link": true },
            "apps/docs/node_modules/next": { "version": "15.1.0" }
          }
        }"#,
    );
    write_file(
        temp.path(),
        "apps/docs/app/page.tsx",
        "import Link from \"next/link\";\nimport { helper } from \"@acme/lib\";\nexport default function Page() {\n  return <main>{helper(Link)}</main>;\n}\n",
    );

    let report = audit_project(temp.path()).unwrap();
    let ids = issue_ids(&report);
    assert!(
        !ids.iter().any(|id| id.starts_with("lockfile-mismatch:")),
        "peer, optional, link, and member-local installs all resolve: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id.starts_with("lockfile-missing:")),
        "a declared workspace member is covered by the root lockfile: {ids:?}"
    );
    // A bare "*" naming a workspace package is the npm workspaces convention,
    // not an unbounded registry range.
    assert!(
        !ids.iter()
            .any(|id| id.starts_with("unbounded-dependency-range:")),
        "workspace `*` references are local: {ids:?}"
    );
}

/// A registry dependency the lockfile never resolves is still drift.
#[test]
fn a_declaration_absent_from_the_lockfile_is_still_a_mismatch() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "demo-app",
          "dependencies": { "react": "^19.0.0", "zod": "^3.24.0" },
          "peerDependencies": { "rxjs": "^7.0.0" }
        }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{
          "lockfileVersion": 3,
          "packages": {
            "": { "name": "demo-app" },
            "node_modules/react": { "version": "19.0.0" }
          }
        }"#,
    );
    write_file(
        temp.path(),
        "app/page.tsx",
        "import React from \"react\";\nexport default function Page() {\n  return <main>Hello</main>;\n}\n",
    );

    let ids = issue_ids(&audit_project(temp.path()).unwrap());
    assert!(
        ids.iter()
            .any(|id| id == "lockfile-mismatch:package.json:zod"),
        "an unresolved runtime declaration must still fire: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id.contains(":rxjs")),
        "a peer dependency is the consuming app's install: {ids:?}"
    );
}

/// A nested project that is not a declared workspace member installs on its
/// own, so the monorepo lockfile above it describes nothing about it: no drift
/// can be measured against that lockfile, and the manifest's own missing
/// lockfile is what there is to say. A teaching sample is exempt from even
/// that, the way `unbounded-dependency-range` already exempts it.
#[test]
fn manifests_outside_the_workspace_globs_are_not_measured_against_the_root_lockfile() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "framework", "private": true, "workspaces": ["packages/*"] }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{
          "lockfileVersion": 3,
          "packages": {
            "": { "name": "framework" },
            "node_modules/rxjs": { "version": "7.8.2" }
          }
        }"#,
    );
    write_file(
        temp.path(),
        "sample/01-basic/package.json",
        r#"{ "name": "basic-sample", "dependencies": { "rxjs": "7.8.2", "zod": "^3.24.0" } }"#,
    );
    write_file(
        temp.path(),
        "tools/benchmarks/package.json",
        r#"{ "name": "benchmarks", "dependencies": { "rxjs": "7.8.2", "zod": "^3.24.0" } }"#,
    );

    let ids = issue_ids(&audit_project(temp.path()).unwrap());
    assert!(
        !ids.iter().any(|id| id.starts_with("lockfile-mismatch:")),
        "an ancestor lockfile that does not cover the manifest proves no drift: {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "lockfile-missing:tools/benchmarks/package.json"),
        "a non-member project with no lockfile of its own is still reported: {ids:?}"
    );
    assert!(
        !ids.iter()
            .any(|id| id == "lockfile-missing:sample/01-basic/package.json"),
        "a teaching sample is not the project's shipped dependency set: {ids:?}"
    );
}

/// A `"*"` spec on a real registry package is still unbounded.
#[test]
fn an_unbounded_range_on_a_registry_package_still_fires() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "demo-app",
          "workspaces": ["packages/*"],
          "dependencies": { "@snyk/protect": "latest", "@local/ui": "*" }
        }"#,
    );
    write_file(
        temp.path(),
        "packages/ui/package.json",
        r#"{ "name": "@local/ui", "version": "1.0.0" }"#,
    );

    let issues = audit_project(temp.path()).unwrap().issues;
    let issue = issues
        .iter()
        .find(|issue| issue.id == "unbounded-dependency-range:package.json")
        .expect("registry package with a `latest` spec"); // allow-expect: test assertion
    let evidence = issue.evidence.clone().unwrap_or_default();
    assert!(evidence.contains("@snyk/protect"), "{evidence}");
    assert!(!evidence.contains("@local/ui"), "{evidence}");
}

/// A `from "..."` inside ordinary source text is not an import, and a
/// `@types/*` package resolves a TypeScript import on its own.
#[test]
fn string_literals_and_type_packages_do_not_become_undeclared_imports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "demo-app",
          "dependencies": { "next": "^15.0.0" },
          "devDependencies": { "@types/express": "^4.17.21", "@types/mdx": "^2.0.13" }
        }"#,
    );
    // No semicolons anywhere, so a `[^;]*` scan runs off the declaration.
    write_file(
        temp.path(),
        "components/user-auth-form.tsx",
        "import { useSearchParams } from \"next/navigation\"\n\nexport function UserAuthForm() {\n  const searchParams = useSearchParams()\n  return signIn(\"email\", {\n    callbackUrl: searchParams?.get(\"from\") || \"/dashboard\",\n  })\n}\n",
    );
    write_file(
        temp.path(),
        "src/middleware.controller.ts",
        "export class MiddlewareController {\n  hello() {\n    return 'Hello from \"MiddlewareController\"!'\n  }\n}\n",
    );
    write_file(
        temp.path(),
        "src/main.ts",
        "import type { Request, Response } from \"express\"\nexport type Handler = (request: Request, response: Response) => void\n",
    );
    write_file(
        temp.path(),
        "src/mdx-components.tsx",
        "import type { MDXComponents } from \"mdx/types\"\nexport const components: MDXComponents = {}\n",
    );

    let ids = issue_ids(&audit_project(temp.path()).unwrap());
    let undeclared = ids
        .iter()
        .filter(|id| id.starts_with("undeclared-package:"))
        .collect::<Vec<_>>();
    assert!(
        undeclared.is_empty(),
        "string literals and @types packages are not undeclared imports: {undeclared:?}"
    );
}

/// A genuinely undeclared import is still reported, including from a
/// TypeScript file where no matching `@types/*` package is declared.
#[test]
fn a_phantom_dependency_is_still_reported() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "next": "^15.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "vite.config.ts",
        "import devServer from \"@hono/vite-dev-server\"\nexport default { plugins: [devServer()] }\n",
    );
    write_file(
        temp.path(),
        "scraper/find-asins.js",
        "const puppeteer = require('puppeteer');\nmodule.exports = puppeteer;\n",
    );

    let ids = issue_ids(&audit_project(temp.path()).unwrap());
    assert!(
        ids.iter()
            .any(|id| id == "undeclared-package:vite.config.ts:-hono-vite-dev-server"),
        "{ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "undeclared-package:scraper/find-asins.js:puppeteer"),
        "{ids:?}"
    );
}

/// `baseUrl` makes every directory under it importable by bare specifier, with
/// no `paths` entry involved.
#[test]
fn base_url_directories_resolve_as_internal_imports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "next": "^15.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "tsconfig.json",
        r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./*"]
    }
  }
}"#,
    );
    write_file(
        temp.path(),
        "types/index.d.ts",
        "export type SiteConfig = { name: string }\n",
    );
    write_file(
        temp.path(),
        "config/site.ts",
        "import { SiteConfig } from \"types\"\nexport const site: SiteConfig = { name: \"demo\" }\n",
    );

    // A second package's tsconfig must not silence an import elsewhere just
    // because it happens to own a directory of the same name.
    write_file(
        temp.path(),
        "apps/tool/tsconfig.json",
        r#"{
  "compilerOptions": {
    "baseUrl": "."
  }
}"#,
    );
    write_file(
        temp.path(),
        "apps/tool/playwright/helpers.ts",
        "export const helpers = {}\n",
    );
    write_file(
        temp.path(),
        "scripts/smoke.ts",
        "import { chromium } from \"playwright\"\nexport const browser = chromium\n",
    );

    let ids = issue_ids(&audit_project(temp.path()).unwrap());
    assert!(
        !ids.iter().any(|id| id.contains(":types")),
        "a baseUrl directory is an internal import: {ids:?}"
    );
    assert!(
        ids.iter()
            .any(|id| id == "undeclared-package:scripts/smoke.ts:playwright"),
        "a baseUrl root only governs the files under its own config: {ids:?}"
    );
}

/// Usage inside mocks, tool configuration, and script bin names counts: the
/// package is used, it just leaves no import in the emitted source walk.
#[test]
fn mock_config_and_bin_usage_are_not_unused_dependencies() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "demo-app",
          "scripts": { "build": "nest build", "typecheck": "tsc --noEmit" },
          "dependencies": { "next": "^15.0.0" },
          "devDependencies": {
            "msw": "^2.2.14",
            "@nestjs/cli": "^10.0.0",
            "@nestjs/schematics": "^10.0.0",
            "typescript": "^5.5.0",
            "ts-jest": "^29.1.0",
            "abandoned-widget": "^1.0.0"
          },
          "optionalDependencies": { "fsevents": "^2.3.3" },
          "peerDependencies": { "react": "^19.0.0" }
        }"#,
    );
    write_file(temp.path(), "package-lock.json", "{\"lockfileVersion\": 3}");
    write_file(
        temp.path(),
        "src/testing/mocks/handlers.ts",
        "import { http } from \"msw\"\nexport const handlers = [http.get(\"/api\", () => null)]\n",
    );
    write_file(
        temp.path(),
        "jest.config.ts",
        "export default { preset: \"ts-jest\" }\n",
    );
    // A NestJS project names its schematics collection here and nowhere else.
    write_file(
        temp.path(),
        "nest-cli.json",
        "{\n  \"language\": \"ts\",\n  \"collection\": \"@nestjs/schematics\",\n  \"sourceRoot\": \"src\"\n}\n",
    );
    write_file(
        temp.path(),
        "app/page.tsx",
        "import Link from \"next/link\"\nexport default function Page() {\n  return <Link href=\"/\">home</Link>\n}\n",
    );

    let ids = issue_ids(&audit_project(temp.path()).unwrap());
    let unused = ids
        .iter()
        .filter(|id| id.starts_with("unused-dependency:"))
        .collect::<Vec<_>>();
    assert_eq!(
        unused,
        vec!["unused-dependency:package.json:abandoned-widget"],
        "only the package with no usage anywhere is unused: {ids:?}"
    );
}

/// Dependency resolution reaches a verdict the static scan cannot fully
/// establish, so both findings ship for review rather than as strong claims.
#[test]
fn dependency_findings_ship_as_needs_review() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "demo-app",
          "dependencies": { "next": "^15.0.0" },
          "devDependencies": { "abandoned-widget": "^1.0.0" }
        }"#,
    );
    write_file(temp.path(), "package-lock.json", "{\"lockfileVersion\": 3}");
    write_file(
        temp.path(),
        "app/page.tsx",
        "import Link from \"next/link\"\nimport ghost from \"never-declared-pkg\"\nexport default function Page() {\n  return <Link href={ghost}>home</Link>\n}\n",
    );

    let issues = audit_project(temp.path()).unwrap().issues;
    for prefix in ["undeclared-package:", "unused-dependency:"] {
        let issue = issues
            .iter()
            .find(|issue| issue.id.starts_with(prefix))
            .unwrap_or_else(|| panic!("expected a {prefix} finding"));
        assert_eq!(
            issue.confidence,
            crate::checks::IssueConfidence::NeedsReview,
            "{}",
            issue.id
        );
        assert!(
            issue.confidence_reason.is_some(),
            "{} should explain the caveat",
            issue.id
        );
    }
}
