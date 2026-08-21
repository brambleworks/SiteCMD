use super::super::*;

#[test]
fn detects_public_route_missing_rate_limit() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ email: body.email });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("public-endpoint-rate-limit:")));
}

#[test]
fn detects_public_external_call_without_timeout_and_retry_policy() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  const res = await fetch("https://api.resend.com/emails");
                  return Response.json({ ok: res.ok, email: body.email });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("external-call-timeout:")));
    let retry_issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("external-call-retry:"))
        .expect("external-call-retry issue"); // allow-expect: test assertion
    assert_eq!(retry_issue.severity, Severity::Low);
    assert!(retry_issue.title.contains("No explicit"));
    assert!(retry_issue.description.contains("fail-fast"));
    assert!(retry_issue.description.contains("does not prove"));
}

#[test]
fn detects_env_template_gaps_and_runtime_env_drift() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                export async function POST() {
                  return Response.json({
                    openai: process.env.OPENAI_API_KEY,
                    stripe: process.env.STRIPE_WEBHOOK_SECRET,
                  });
                }
            "#,
    );
    write_file(temp.path(), ".env.example", "OPENAI_API_KEY=\n");
    write_file(temp.path(), ".env.development", "OPENAI_API_KEY=dev\n");
    write_file(
        temp.path(),
        ".env.production",
        "STRIPE_WEBHOOK_SECRET=prod\n",
    );
    write_file(
        temp.path(),
        "vercel.json",
        "{ \"framework\": \"nextjs\" }\n",
    );

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    let incomplete = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("env-example-incomplete:"))
        .expect("incomplete env template");
    assert_eq!(incomplete.severity, Severity::Medium);
    assert_eq!(
        incomplete.confidence,
        crate::checks::IssueConfidence::Confirmed
    );
    assert_eq!(incomplete.relative_path, ".env.example");
    assert_eq!(incomplete.line, None);
    assert!(incomplete.description.contains("optional"));

    let drift = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("env-drift:"))
        .expect("parallel env-file key drift");
    assert_eq!(drift.severity, Severity::Medium);
    assert_eq!(drift.relative_path, ".env.development");
    assert_eq!(drift.line, None);
    assert!(drift.description.contains("does not establish"));
}

#[test]
fn default_audit_does_not_inspect_local_env_values() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/users/route.ts",
        "export async function GET() { return Response.json({ ok: true }); }",
    );
    write_file(
        temp.path(),
        ".env.local",
        "DATABASE_URL=postgresql://app:secret@database.example.com:5432/app\n",
    );
    write_file(temp.path(), ".env.production", "APP_DEBUG=true\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report.issues.iter().any(|issue| {
            issue.id.starts_with("local-db-target-remote:")
                || (issue.id.starts_with("framework-debug-enabled:")
                    && issue.relative_path.starts_with(".env"))
        }),
        "ordinary Code Scan must not derive findings from local dotenv values"
    );
}

#[test]
fn monorepo_env_templates_and_runtime_files_are_scoped_to_their_package() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/root-config.ts",
        "export const rootConfig = process.env.ROOT_CONFIG;\n",
    );
    write_file(temp.path(), ".env.example", "ROOT_CONFIG=\n");

    write_file(
        temp.path(),
        "apps/api/src/config.ts",
        "export const apiKey = process.env.API_KEY;\n",
    );
    write_file(temp.path(), "apps/api/.env.example", "API_KEY=\n");
    write_file(
        temp.path(),
        "apps/api/.env.production",
        "API_KEY=production\n",
    );

    write_file(
        temp.path(),
        "apps/web/src/config.ts",
        "export const publicUrl = process.env.PUBLIC_URL;\n",
    );
    write_file(temp.path(), "apps/web/.env.example", "PUBLIC_URL=\n");
    write_file(
        temp.path(),
        "apps/web/.env.production",
        "PUBLIC_URL=https://example.test\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report.issues.iter().any(|issue| {
            issue.id.starts_with("env-example-missing:")
                || issue.id.starts_with("env-example-incomplete:")
                || issue.id.starts_with("env-drift:")
        }),
        "package-scoped env files were compared across the monorepo: {:?}",
        report
            .issues
            .iter()
            .map(|issue| &issue.id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn platform_injected_env_usage_does_not_require_an_example_file() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/status/route.ts",
        r#"
                export async function GET() {
                  return Response.json({ environment: process.env.NODE_ENV });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("env-example-missing:")),
        "runtime-provided variables should not create a developer env-template task"
    );
}

#[test]
fn nextjs_layered_env_convention_is_not_drift() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                export async function POST() {
                  return Response.json({
                    url: process.env.NEXT_PUBLIC_APP_URL,
                    openai: process.env.OPENAI_API_KEY,
                  });
                }
            "#,
    );
    write_file(
        temp.path(),
        ".env",
        "NEXT_PUBLIC_APP_URL=http://localhost:3000\n",
    );
    write_file(temp.path(), ".env.local", "OPENAI_API_KEY=sk-local\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("env-drift:")),
        "the .env + .env.local layering must not read as drift"
    );
}

#[test]
fn detects_remote_database_url_in_local_dev_env() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/users/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                const prisma = new PrismaClient();

                export async function GET() {
                  return Response.json(await prisma.user.findMany());
                }
            "#,
    );
    write_file(
        temp.path(),
        ".env.local",
        "DATABASE_URL=postgresql://postgres:postgres@db.supabase.co:5432/app\n",
    );

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("local-db-target-remote:"))
        .expect("remote local-development database target");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("may be intentional"));
    assert!(!issue.description.contains("production data"));
    assert!(issue
        .verify_hint
        .as_deref()
        .is_some_and(|text| text.contains("project or branch identifier")));
}

#[test]
fn docker_compose_service_hostname_is_not_a_remote_database() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/users/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";
                const prisma = new PrismaClient();
                export async function GET() {
                  return Response.json(await prisma.user.findMany());
                }
            "#,
    );
    write_file(
        temp.path(),
        ".env.local",
        "DATABASE_URL=postgresql://app:app@db:5432/app\n",
    );

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("local-db-target-remote:")),
        "a single-label container service hostname is local dev infrastructure"
    );
}

#[test]
fn detects_frontend_supabase_usage_without_rls_markers() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Profile.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Profile() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-rls-missing:"))
        .expect("frontend Supabase usage without local RLS evidence needs review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("scanned local artifacts"));
    assert!(issue
        .description
        .contains("deployed database was not inspected"));
    assert!(!issue.description.contains("database never got"));
}

#[test]
fn skips_supabase_rls_issue_when_local_policy_markers_exist() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Profile.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Profile() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "Profiles are readable by owner" on public.profiles
                  for select using (auth.uid() = user_id);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-rls-missing:")));
}

#[test]
fn rls_enabled_without_any_policy_is_a_default_deny_configuration_gap() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Profile.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Profile() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        "alter table public.profiles enable row level security;\n",
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-policy-set-empty:"))
        .expect("RLS with no policies should explain the default-deny configuration gap");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.category, "data");
    assert!(issue.description.to_ascii_lowercase().contains("deny"));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-rls-missing:")));
}

#[test]
fn detects_supabase_frontend_table_without_matching_local_policy() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Profile.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Profile() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_projects.sql",
        r#"
                alter table public.projects enable row level security;
                create policy "Projects are readable by owner" on public.projects
                  for select using (auth.uid() = owner_id);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-rls-missing:")));
}

#[test]
fn detects_supabase_permissive_policy_for_client_facing_table() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Profile.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Profile() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "Profiles are readable by everyone" on public.profiles
                  for select using (true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-open-policy:"))
        .expect("an unconditional public-read policy should be surfaced for review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.contains("unconditional"));
    assert!(issue.description.contains("restrictive policies"));
    assert!(!issue.title.contains("allows every row"));
}

#[test]
fn detects_supabase_write_operation_without_matching_local_policy() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/ProfileEditor.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function saveProfile(id: string, name: string) {
                  return await supabase
                    .from("profiles")
                    .update({ name })
                    .eq("id", id);
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "Profiles are readable by owner" on public.profiles
                  for select using (auth.uid() = user_id);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-policy-operation-missing:"))
        .expect("the unavailable frontend operation should be reported");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.category, "data");
    assert!(issue.description.to_ascii_lowercase().contains("denied"));
    assert!(issue.description.contains("scanned local policy set"));
    assert!(issue.description.contains("if applied"));
    assert!(!issue.description.contains("checked-in"));
}

#[test]
fn skips_supabase_write_operation_issue_when_local_policy_covers_write() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/ProfileEditor.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function saveProfile(id: string, name: string) {
                  return await supabase
                    .from("profiles")
                    .update({ name })
                    .eq("id", id);
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "Profiles are readable by owner" on public.profiles
                  for select using (auth.uid() = user_id);
                create policy "Profiles are writable by owner" on public.profiles
                  for update using (auth.uid() = user_id)
                  with check (auth.uid() = user_id);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-policy-operation-missing:")));
}

#[test]
fn detects_client_side_supabase_service_role_usage() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Admin.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient(
                  "https://example.supabase.co",
                  process.env.NEXT_PUBLIC_SUPABASE_SERVICE_ROLE_KEY!,
                );

                export async function Admin() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-service-role-client:"))
        .expect("supabase-service-role-client issue"); // allow-expect: test assertion
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not prove"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("First inspect"));
}

#[test]
fn two_uncovered_tables_in_one_file_produce_two_distinct_rls_ids() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Dashboard.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Dashboard() {
                  const { data: profiles } = await supabase.from("profiles").select("*");
                  const { data: orders } = await supabase.from("orders").select("*");
                  return <pre>{JSON.stringify({ profiles, orders })}</pre>;
                }
            "#,
    );
    // RLS markers exist so the per-table branch runs, but neither table is
    // covered.
    write_file(
        temp.path(),
        "supabase/migrations/20260409_other.sql",
        r#"
                alter table public.projects enable row level security;
                create policy "Projects are readable by owner" on public.projects
                  for select using (auth.uid() = owner_id);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let rls_ids: Vec<&str> = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("supabase-rls-missing:"))
        .map(|issue| issue.id.as_str())
        .collect();
    assert!(
        rls_ids.contains(&"supabase-rls-missing:src/components/Dashboard.tsx:profiles"),
        "profiles finding missing or wrongly keyed: {rls_ids:?}"
    );
    assert!(
        rls_ids.contains(&"supabase-rls-missing:src/components/Dashboard.tsx:orders"),
        "orders finding missing or wrongly keyed: {rls_ids:?}"
    );
}

#[test]
fn open_policy_emits_once_per_policy_table_and_is_needs_review() {
    let temp = TempDir::new().unwrap();
    for component in ["Profile", "Feed"] {
        write_file(
            temp.path(),
            &format!("src/components/{component}.tsx"),
            r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function View() {
                  const { data } = await supabase.from("profiles").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
        );
    }
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "Profiles are readable by everyone" on public.profiles
                  for select using (true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let open_policy: Vec<&CodeIssue> = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("supabase-open-policy:"))
        .collect();
    assert_eq!(
        open_policy.len(),
        1,
        "one policy/table pair must produce one finding, got {:?}",
        open_policy
            .iter()
            .map(|issue| &issue.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        open_policy[0].confidence,
        crate::checks::IssueConfidence::NeedsReview,
        "'appears to allow' is a heuristic SQL read and must grade NeedsReview"
    );
}

#[test]
fn distinct_open_policies_on_one_table_are_not_collapsed() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/ProfileEditor.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function loadAndSave(id: string) {
                  await supabase.from("profiles").select("*");
                  return supabase.from("profiles").update({ visible: true }).eq("id", id);
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "Profiles are public" on public.profiles
                  for select using (true);
                create policy "Profiles are globally writable" on public.profiles
                  for update using (true) with check (true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let open_policies = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("supabase-open-policy:"))
        .collect::<Vec<_>>();
    assert_eq!(
        open_policies.len(),
        2,
        "each distinct policy needs its own evidence"
    );
    assert!(open_policies
        .iter()
        .any(|issue| issue.severity == Severity::High && issue.title.contains("write")));
}

#[test]
fn public_select_predicate_is_not_mislabeled_as_missing_auth_scoping() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/Posts.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function Posts() {
                  const { data } = await supabase.from("posts").select("*");
                  return <pre>{JSON.stringify(data)}</pre>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_posts.sql",
        r#"
                alter table public.posts enable row level security;
                create policy "Published posts are readable" on public.posts
                  for select using (published = true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-policy-not-auth-scoped:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-open-policy:")));
}

#[test]
fn write_policy_without_caller_scoping_is_needs_review() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/PostEditor.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");

                export async function publishPost(id: string) {
                  return supabase.from("posts").update({ published: true }).eq("id", id);
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_posts.sql",
        r#"
                alter table public.posts enable row level security;
                create policy "Draft posts can be updated" on public.posts
                  for update using (published = false)
                  with check (published = true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-policy-not-auth-scoped:"))
        .expect("a client-facing write policy without caller scoping needs review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.to_ascii_lowercase().contains("write policy"));
}

#[test]
fn service_role_only_policy_does_not_masquerade_as_frontend_coverage() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/PublicPosts.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");
                export async function loadPosts() {
                  return supabase.from("posts").select("*");
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_posts.sql",
        r#"
                alter table public.posts enable row level security;
                create policy "backend reads" on public.posts
                  for select to service_role using (true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-policy-operation-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("supabase-open-policy:")));
}

#[test]
fn one_unconditional_update_clause_is_not_called_every_row_access() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "src/components/ProfileEditor.tsx",
        r#"
                "use client";
                import { createClient } from "@supabase/supabase-js";

                const supabase = createClient("https://example.supabase.co", "anon-key");
                export async function saveProfile(id: string) {
                  return supabase.from("profiles").update({ display_name: "Updated" }).eq("id", id);
                }
            "#,
    );
    write_file(
        temp.path(),
        "supabase/migrations/20260409_profiles.sql",
        r#"
                alter table public.profiles enable row level security;
                create policy "owners update profiles" on public.profiles
                  for update to authenticated
                  using (auth.uid() = user_id)
                  with check (true);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("supabase-open-policy:"))
        .expect("unconditional WITH CHECK finding");
    assert!(!issue
        .title
        .to_ascii_lowercase()
        .contains("allows every row"));
    assert!(issue
        .description
        .to_ascii_lowercase()
        .contains("other policy clause"));
}
