use super::super::*;
use crate::checks::IssueConfidence;

fn issue_ids(report: &CodeScanReport) -> Vec<String> {
    report.issues.iter().map(|issue| issue.id.clone()).collect()
}

fn has_issue(report: &CodeScanReport, prefix: &str) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with(prefix))
}

#[test]
fn python_command_from_request_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/run.py",
        r#"import os
import json
from flask import request

@app.post("/run")
def run():
    body = json.loads(request.data)
    os.system("ping -c 1 " + request.args["host"])
    return {"ok": True}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-command-injection:"))
        .unwrap_or_else(|| {
            panic!(
                "expected python-command-injection, got: {:?}",
                issue_ids(&report)
            )
        });
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("os.system") && !evidence.contains("shell=True"),
        "os.system evidence should name the matched sink family: {evidence}"
    );
    let fix = issue.likely_fix.as_deref().unwrap_or_default();
    assert!(
        fix.contains("Replace the call") && fix.contains("os.system"),
        "os.system fix should say to replace the call: {fix}"
    );
    assert!(
        !fix.contains("shlex.quote"),
        "quoting is not the primary safe boundary: {fix}"
    );
    assert!(
        fix.contains("leading-option"),
        "option injection must be covered: {fix}"
    );
    assert_eq!(issue.confidence, IssueConfidence::NeedsReview);
    assert!(issue.title.contains("Request accessor"));
    assert!(issue.description.contains("Static analysis matched"));
    assert!(issue.description.contains("does not establish"));
    let verify = issue.verify_hint.as_deref().unwrap_or_default();
    assert!(verify.contains("mock") || verify.contains("test harness"));
    assert!(!verify.contains("marker command"));
    // The precise Python sink owns this: the fuzzy generic shell-injection must
    // not also fire on the same route file.
    assert!(
        !has_issue(&report, "shell-injection:"),
        "Python shell sinks should not double-flag via generic shell-injection, got: {:?}",
        issue_ids(&report)
    );

    // subprocess with shell=True and a request accessor is the same bug.
    let subprocess = TempDir::new().unwrap();
    write_file(
        subprocess.path(),
        "app/api/run.py",
        r#"import subprocess
from flask import request

@app.post("/run")
def run():
    subprocess.run("echo " + request.form["msg"], shell=True)
"#,
    );

    let report = audit_project(subprocess.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-command-injection:"))
        .unwrap_or_else(|| {
            panic!(
                "expected python-command-injection for subprocess shell=True, got: {:?}",
                issue_ids(&report)
            )
        });
    // The subprocess branch's copy matches its sink: the remedy is dropping
    // shell=True, and the evidence must not blame os.system.
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("shell=True") && !evidence.contains("os.system"),
        "subprocess evidence should name the matched sink family: {evidence}"
    );
    let fix = issue.likely_fix.as_deref().unwrap_or_default();
    assert!(
        fix.contains("Drop shell=True"),
        "subprocess fix should say to drop shell=True: {fix}"
    );
    assert!(!fix.contains("shlex.quote"));
    assert!(fix.contains("leading-option"));

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/run.py",
        r#"import subprocess
import shlex
from flask import request

@app.post("/run")
def run():
    # No shell: arguments are passed directly to the program.
    subprocess.run(["ls", request.args["dir"]])
    # Escaped argument.
    subprocess.run("ls " + shlex.quote(request.args["dir"]), shell=True)
    # Constant command.
    subprocess.run("systemctl restart nginx", shell=True)
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-command-injection:"))
        .expect("shlex.quote command argument should produce a scoped review finding");
    assert_eq!(issue.severity, Severity::Medium);
    assert!(issue.title.contains("Shell-quoted request argument"));
    assert!(issue.description.contains("does not by itself constrain"));
}

#[test]
fn python_unsafe_deserialization_of_request_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/restore.py",
        r#"import base64
import pickle
from flask import request

@app.post("/restore")
def restore():
    state = pickle.loads(base64.b64decode(request.data))
    return render(state)
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-unsafe-deserialization:"))
        .expect("expected python-unsafe-deserialization");
    assert_scoped_static_sink_copy(issue);

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/restore.py",
        r#"import pickle
import yaml
from flask import request

@app.post("/restore")
def restore():
    # SafeLoader disables arbitrary object construction.
    config = yaml.load(request.data, Loader=yaml.SafeLoader)
    # safe_load never constructs arbitrary types.
    prefs = yaml.safe_load(request.data)
    # A cache-sourced value is not request input.
    session = pickle.loads(cache.get("session"))
    return render(config, prefs, session)
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-unsafe-deserialization:"),
        "SafeLoader, safe_load, and cache-sourced pickle must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn python_eval_of_request_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/calc.py",
        r#"import json
from flask import request

@app.post("/calc")
def calc():
    payload = json.loads(request.data)
    return {"result": eval(request.form["expr"])}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-code-execution:"))
        .expect("expected python-code-execution");
    assert_scoped_static_sink_copy(issue);
    assert!(!issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("system('id')"));
    // The precise Python sink owns eval/exec: generic eval-exec-injection must
    // not also fire on the same route file.
    assert!(
        !has_issue(&report, "eval-exec-injection:"),
        "Python eval should not double-flag via generic eval-exec-injection, got: {:?}",
        issue_ids(&report)
    );

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/calc.py",
        r#"import ast
from flask import request

@app.post("/calc")
def calc():
    # literal_eval only evaluates literals - no code execution.
    value = ast.literal_eval(request.args["expr"])
    # ML eval-mode, not the builtin.
    model.eval()
    # SQL execution, not code execution.
    cursor.execute("SELECT 1")
    return {"value": value}
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-code-execution:"),
        "literal_eval, model.eval(), and cursor.execute must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn python_sql_from_request_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/users.py",
        r#"from flask import request

def get_user():
    cursor.execute(f"SELECT * FROM users WHERE name = '{request.args['name']}'")
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-sql-injection:"))
        .expect("expected python-sql-injection");
    assert_scoped_static_sink_copy(issue);
    assert!(!issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("DROP TABLE"));
    // The precise Python sink owns this: the fuzzy generic raw-sql-unsafe must
    // not also fire on the same.py file.
    assert!(
        !has_issue(&report, "raw-sql-unsafe:"),
        "Python raw SQL should not double-flag via generic raw-sql-unsafe, got: {:?}",
        issue_ids(&report)
    );

    // Concatenation into the query string is the same bug.
    let concat = TempDir::new().unwrap();
    write_file(
        concat.path(),
        "app/api/users.py",
        r#"from flask import request

def get_user():
    cursor.execute("SELECT * FROM users WHERE id = " + request.args["id"])
"#,
    );

    let report = audit_project(concat.path()).unwrap();
    assert!(
        has_issue(&report, "python-sql-injection:"),
        "expected python-sql-injection for concatenation, got: {:?}",
        issue_ids(&report)
    );

    // Safe forms: a bound parameter after the comma, psycopg2 identifier
    // composition, and a constant query whose SELECT list contains commas.
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/users.py",
        r#"from flask import request
from psycopg2 import sql

def get_user():
    cursor.execute("SELECT id, name, email FROM users WHERE id = %s", [request.args["id"]])
    cursor.execute(sql.SQL("SELECT * FROM {}").format(sql.Identifier(request.args["table"])))
    cursor.execute("SELECT id, name, created_at FROM users ORDER BY created_at")
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-sql-injection:"),
        "parameterized query, sql.Identifier, and constant SELECT must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn python_sink_keywords_in_comments_and_docstrings_do_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/notes.py",
        r#"from flask import request
import json

def get_note():
    """Never os.system(request.args['cmd']) or eval(request.args['x']) here.

    We could pickle.loads(request.data) but that would be unsafe.
    """
    # cursor.execute(f"SELECT {request.args['id']}") would be injection; we don't.
    return json.loads(request.data)
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    for prefix in [
        "python-command-injection:",
        "python-code-execution:",
        "python-unsafe-deserialization:",
        "python-sql-injection:",
    ] {
        assert!(
            !has_issue(&report, prefix),
            "sink call inside a comment/docstring must not flag {prefix}, got: {:?}",
            issue_ids(&report)
        );
    }
}

#[test]
fn python_sqlalchemy_expression_language_is_not_sql_injection() {
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/users.py",
        r#"from flask import request
from app import db, User, session

def list_users():
    db.session.execute(db.select(User).filter_by(name=request.args["name"]))
    session.execute(select(User).where(User.name == request.args["name"]))
    session.query(User).filter(User.id == request.args["id"]).all()
    User.query.filter(User.email == request.args["email"]).all()
"#,
    );
    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-sql-injection:"),
        "SQLAlchemy expression/ORM builders must not flag as SQL injection, got: {:?}",
        issue_ids(&report)
    );

    // The.py gate hole: a raw query string through.query still flags.
    let raw = TempDir::new().unwrap();
    write_file(
        raw.path(),
        "app/api/search.py",
        r#"from flask import request

def search():
    db.query(f"SELECT * FROM items WHERE name = '{request.args['q']}'")
"#,
    );
    let report = audit_project(raw.path()).unwrap();
    assert!(
        has_issue(&report, "python-sql-injection:"),
        "raw f-string SQL through .query() must flag, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn python_template_from_request_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/render.py",
        r#"from flask import request, render_template_string

@app.post("/preview")
def preview():
    return render_template_string("<h1>Hello " + request.args["name"] + "</h1>")
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-template-injection:"))
        .expect("expected python-template-injection");
    assert_scoped_static_sink_copy(issue);

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/render.py",
        r#"from flask import request, render_template, render_template_string

TEMPLATE = "<h1>Hello {{ name }}</h1>"

@app.post("/preview")
def preview():
    # request data as a context value - Jinja escapes it, not executes it.
    a = render_template_string(TEMPLATE, name=request.args["name"])
    # File-based template: request never reaches the template source.
    b = render_template("preview.html", name=request.args["name"])
    return a + b
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-template-injection:"),
        "context-value render and render_template must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn python_redirect_from_request_is_open_redirect() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/login.py",
        r#"from flask import request, redirect

@app.get("/login")
def login():
    return redirect(request.args["next"])
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-open-redirect:"))
        .expect("expected python-open-redirect");
    assert_scoped_static_sink_copy(issue);
    // The precise Python sink owns this; the fuzzy generic open-redirect must
    // not also fire on the same route file.
    assert!(
        !has_issue(&report, "open-redirect:"),
        "Python redirect sinks should not double-flag via generic open-redirect, got: {:?}",
        issue_ids(&report)
    );

    // Django's HttpResponseRedirect straight from request.GET is the same bug.
    let django = TempDir::new().unwrap();
    write_file(
        django.path(),
        "app/api/views.py",
        r#"from django.http import HttpResponseRedirect

def next_view(request):
    return HttpResponseRedirect(request.GET["next"])
"#,
    );
    let report = audit_project(django.path()).unwrap();
    assert!(
        has_issue(&report, "python-open-redirect:"),
        "expected python-open-redirect for HttpResponseRedirect, got: {:?}",
        issue_ids(&report)
    );

    // Safe forms: url_for (server-owned), a relative literal, and a variable
    // validated against an allowlist before redirecting.
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/login.py",
        r#"from flask import request, redirect, url_for

ALLOWED = {"/dashboard", "/settings"}

@app.get("/login")
def login():
    target = request.args.get("next", "/")
    if target not in ALLOWED:
        target = "/"
    a = redirect(url_for("dashboard"))
    b = redirect("/home")
    c = redirect(target)
    return a or b or c
"#,
    );
    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-open-redirect:"),
        "url_for, relative literal, and allowlist-checked variable must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn python_file_path_from_request_is_path_traversal() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/download.py",
        r#"from flask import request, send_file

@app.get("/download")
def download():
    return send_file(request.args["path"])
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("python-path-traversal:"))
        .expect("expected python-path-traversal for send_file");
    assert_scoped_static_sink_copy(issue);
    assert!(!issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("/etc/passwd"));

    let remove = TempDir::new().unwrap();
    write_file(
        remove.path(),
        "app/api/delete.py",
        r#"import os
from flask import request

@app.post("/delete")
def delete():
    os.remove(request.form["name"])
    return {"ok": True}
"#,
    );
    let report = audit_project(remove.path()).unwrap();
    assert!(
        has_issue(&report, "python-path-traversal:"),
        "expected python-path-traversal for os.remove, got: {:?}",
        issue_ids(&report)
    );

    // Safe forms: secure_filename and os.path.basename confine the value, a
    // constant path is fixed, and send_from_directory guards traversal itself.
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "app/api/download.py",
        r#"import os
from flask import request, send_file, send_from_directory
from werkzeug.utils import secure_filename

UPLOADS = "/var/app/uploads"

@app.get("/download")
def download():
    a = send_file(os.path.join(UPLOADS, secure_filename(request.args["path"])))
    b = open(os.path.join(UPLOADS, os.path.basename(request.args["path"])))
    c = open("/etc/app/config.json")
    d = send_from_directory(UPLOADS, request.args["path"])
    return a
"#,
    );
    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "python-path-traversal:"),
        "secure_filename, basename, constant path, and send_from_directory must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

fn assert_scoped_static_sink_copy(issue: &CodeIssue) {
    assert_eq!(issue.confidence, IssueConfidence::NeedsReview);
    assert!(issue.title.contains("Request accessor"), "{}", issue.title);
    assert!(
        issue.description.contains("Static analysis matched"),
        "{}",
        issue.description
    );
    assert!(
        issue.description.contains("does not establish"),
        "{}",
        issue.description
    );
}
