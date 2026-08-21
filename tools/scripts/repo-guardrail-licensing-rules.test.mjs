import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { ROOT, rules } from "./guardrail-test-support.mjs";

const {
  licenseActivationErrorFailures,
  licenseCodeUnionFailures,
  licenseSurfaceFailures,
  licenseValidationBranchFailures,
  stripComments,
  stripNonCode,
} = rules;

describe("the licensing guardrails survive a decoy left in a comment", () => {
  const real = (file) => fs.readFileSync(path.join(ROOT, file), "utf8");
  const withFile = (file, contents) => (asked) => (asked === file ? contents : real(asked));

  const TS = "apps/desktop/src/lib/license-activation-error.ts";
  const LIFECYCLE_VALIDATION =
    "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle_validation.rs";
  const LIFECYCLE_DEACTIVATION =
    "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle_deactivation.rs";
  const PANEL = "apps/desktop/src/components/settings/AccountSettings.tsx";

  const mutate = (file, from, to) => {
    const source = real(file);
    const next = source.replace(from, to);
    expect(next, `mutation of ${file} did not apply; the anchor moved`).not.toBe(source);
    return withFile(file, next);
  };

  it("passes on the real tree", () => {
    expect(licenseCodeUnionFailures(real)).toEqual([]);
    expect(licenseValidationBranchFailures(real)).toEqual([]);
  });

  it("catches an error code commented out of KNOWN_CODES", () => {
    const read = mutate(TS, /^(\s*)"store_mismatch",$/m, '$1// "store_mismatch",');
    expect(licenseCodeUnionFailures(read)).not.toEqual([]);
  });

  it("catches the validation arm replaced with a downgrade, its text left in a comment", () => {
    const read = mutate(
      LIFECYCLE_VALIDATION,
      /^(\s*)let row_answer = offline_validation_or_downgrade\(&row\)\?;/m,
      "$1// offline_validation_or_downgrade(&row) used to answer here\n$1let row_answer = free_info();",
    );
    expect(licenseValidationBranchFailures(read)).not.toEqual([]);
  });

  it("catches the deactivation marker renamed with the old literal left in a comment", () => {
    const read = mutate(
      LIFECYCLE_DEACTIVATION,
      'pub(crate) const DEACTIVATION_KEYCHAIN_REMNANT: &str = "unlinked_with_keychain_remnant: ";',
      '// was: "unlinked_with_keychain_remnant: "\npub(crate) const DEACTIVATION_KEYCHAIN_REMNANT: &str = "RENAMED: ";',
    );
    expect(licenseCodeUnionFailures(read)).not.toEqual([]);
  });

  it("catches the marker being declared but never carried by the error", () => {
    const read = mutate(
      LIFECYCLE_DEACTIVATION,
      '"{DEACTIVATION_KEYCHAIN_REMNANT}This machine was unlinked',
      '"This machine was unlinked',
    );
    expect(licenseCodeUnionFailures(read)).not.toEqual([]);
  });

  it("catches the panel's marker branch being deleted", () => {
    const read = mutate(PANEL, "message.startsWith(DEACTIVATION_KEYCHAIN_REMNANT)", "false");
    expect(licenseCodeUnionFailures(read)).not.toEqual([]);
  });

  it("does not mistake a // inside a string literal for a comment", () => {
    const code = 'const a = "https://example.com"; ';
    const comment = "// gone";
    const stripped = stripComments(`${code}${comment}\nconst b = 1;`, TS);
    expect(stripped).toBe(`${code}${" ".repeat(comment.length)}\nconst b = 1;`);
    expect(stripped).toContain('"https://example.com"');
  });

  it("keeps byte offsets stable so a raw anchor indexes stripped text", () => {
    const source = real(LIFECYCLE_VALIDATION);
    expect(stripNonCode(source, LIFECYCLE_VALIDATION)).toHaveLength(source.length);
    expect(stripComments(source, LIFECYCLE_VALIDATION)).toHaveLength(source.length);
  });

  it("does not let a lifetime open a span that hides the comments after it", () => {
    const read = mutate(
      LIFECYCLE_DEACTIVATION,
      "pub async fn deactivate_license(",
      '// was: format!("{DEACTIVATION_KEYCHAIN_REMNANT}{msg}")\npub async fn deactivate_license(',
    );
    const withMarkerUnhooked = (asked) => {
      const source = read(asked);
      return asked === LIFECYCLE_DEACTIVATION
        ? source.replace(/^(\s*)(.*\{DEACTIVATION_KEYCHAIN_REMNANT\}.*)$/gm, "$1// $2")
        : source;
    };
    expect(licenseCodeUnionFailures(withMarkerUnhooked)).not.toEqual([]);
  });

  it("does not end a nested Rust block comment at the first close", () => {
    const nested =
      "/*\n  /* why this went */\n  Ok(Some(_)) => offline_validation_or_downgrade(&state)\n*/\nOk(Some(_)) => Ok(free_info())";
    const stripped = stripNonCode(nested, "x.rs");
    expect(stripped).not.toContain("offline_validation_or_downgrade");
    expect(stripped).toContain("Ok(free_info())");
    expect(stripNonCode("/* a /* b */ live();", "x.ts")).toContain("live();");
  });

  it("still blanks real char literals and ordinary TypeScript strings", () => {
    const rust = stripNonCode("let c = 'a'; let n = '\\n'; let s = \"gone\";", "x.rs");
    expect(rust).not.toContain("gone");
    expect(rust).toMatch(/let c = {4};/);
    expect(stripNonCode("const a = 'hello world';", "x.ts")).not.toContain("hello world");
  });

  it("catches the activation command leaking the raw upstream error", () => {
    const ACTIVATION =
      "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle_activation.rs";
    const leaked = mutate(
      ACTIVATION,
      "return Err(activation_error_from_raw(&error));",
      "return Err(error);",
    );
    expect(licenseActivationErrorFailures(leaked)).not.toEqual([]);
    const commented = mutate(
      ACTIVATION,
      "return Err(activation_error_from_raw(&error));",
      "// return Err(activation_error_from_raw(&error));\n            return Err(error);",
    );
    expect(licenseActivationErrorFailures(commented)).not.toEqual([]);
  });

  it("refuses to strip without being told which language it is reading", () => {
    expect(() => stripNonCode("fn f() {}")).toThrow(/need the path/);
    expect(() => stripComments("fn f() {}")).toThrow(/need the path/);
  });

  it("recognizes Rust raw BYTE strings, which have no escapes either", () => {
    const swallow =
      'const WIN: &[u8] = br"C:\\";\n// Ok(Some(_)) => offline_validation_or_downgrade(&state)\nfn live() { let s = "marker"; }';
    expect(stripComments(swallow, "x.rs")).not.toContain("offline_validation_or_downgrade");
    expect(stripNonCode(swallow, "x.rs")).toContain("fn live()");
    expect(stripNonCode('let d = br#"say "LIVE" done"#;', "x.rs")).not.toContain("LIVE");
    const wide = `let s = r${"#".repeat(48)}"DECOY"${"#".repeat(48)};\nlive();`;
    expect(stripNonCode(wide, "x.rs")).not.toContain("DECOY");
    expect(stripNonCode('let b = b"gone"; live();', "x.rs")).not.toContain("gone");
    expect(stripNonCode('let charr = "gone"; live();', "x.rs")).toContain("live()");
  });

  it("recognizes Rust raw C STRINGS, the prefix the byte-string fix left out", () => {
    const swallow =
      'const P: &CStr = cr"C:\\";\n// Ok(Some(_)) => offline_validation_or_downgrade(&state)\nfn live() { let s = "marker"; }';
    expect(stripComments(swallow, "x.rs")).not.toContain("offline_validation_or_downgrade");
    expect(stripNonCode(swallow, "x.rs")).toContain("fn live()");
    expect(stripNonCode('let d = cr#"say "LIVE" done"#;', "x.rs")).not.toContain("LIVE");
    expect(stripNonCode('let c = c"gone"; live();', "x.rs")).not.toContain("gone");
  });

  it("does not read an apostrophe in JSX text as a string delimiter", () => {
    const jsx = [
      "<p>This machine's license was unlinked</p>",
      "{/* message.startsWith(DEACTIVATION_KEYCHAIN_REMNANT) */}",
      "<button onClick={handler}>the subscriber's seat</button>",
    ].join("\n");
    expect(stripComments(jsx, "Panel.tsx")).not.toContain("startsWith(DEACTIVATION");
    expect(stripNonCode(jsx, "Panel.tsx")).toContain("handler");
    expect(stripNonCode("const a = 'gone'; live();", "x.ts")).not.toContain("gone");
    expect(stripNonCode("const a = 'go\\\nne'; live();", "x.ts")).not.toContain("ne'");
  });

  it("reads a regex literal as a literal rather than as quotes or a comment", () => {
    expect(stripNonCode("const re = /don't/; live();", "x.ts")).toContain("live()");
    expect(stripNonCode('const re = /say "hi/; live();', "x.ts")).toContain("live()");
    expect(stripNonCode("const re = /`/; live();", "x.ts")).toContain("live()");
    expect(stripNonCode("const re = /[//]/; live();", "x.ts")).toContain("live()");
    expect(stripNonCode("const re = /a}b/; live();", "x.ts")).toContain("live()");
    expect(stripNonCode("const t = `x ${/a}b/.test(live)} y`;", "x.ts")).toContain("live");
    expect(stripNonCode("const p = /offline_validation_or_downgrade/;", "x.ts")).not.toContain(
      "offline_validation",
    );
    expect(stripNonCode("const half = total / 2; const s = 'gone';", "x.ts")).toContain(
      "total / 2",
    );
    expect(stripNonCode("const r = (a) / (b); live();", "x.ts")).toContain("live()");
  });

  it("does not read a keyword-named PROPERTY as the keyword before it", () => {
    const swallow =
      "void ({ of: 2 }.of / 2); // if (message.startsWith(DEACTIVATION_KEYCHAIN_REMNANT)) {\nif (false) {";
    expect(stripComments(swallow, "Panel.tsx")).not.toContain("startsWith(DEACTIVATION");
    for (const word of ["of", "in", "new", "delete", "case", "do", "await", "void"]) {
      const line = `const q = obj.${word} / 2; // ${word} decoy\n`;
      expect(stripComments(line, "x.ts"), `${word} as a property`).not.toContain("decoy");
      expect(stripNonCode(line, "x.ts"), `${word} as a property`).toContain(`obj.${word} / 2`);
    }
    expect(stripNonCode("for (const a of /ab/.exec(s)) { live(); }", "x.ts")).toContain("live()");
    expect(stripNonCode("return /ab/.test(x);", "x.ts")).not.toContain("ab");
    expect(stripComments("const r = a / b;// decoy\n", "x.ts")).not.toContain("decoy");
  });

  it("does not read a postfix ++ or -- as an operator a regex may follow", () => {
    for (const op of ["++", "--"]) {
      const swallow = `let n = 0; n${op} / 2; /* if (m.startsWith(DEACTIVATION_KEYCHAIN_REMNANT)) { */ n${op} / 2;`;
      expect(stripComments(swallow, "Panel.tsx"), op).not.toContain("startsWith(DEACTIVATION");
      expect(stripNonCode(swallow, "Panel.tsx"), op).toContain(`n${op} / 2`);
    }
    expect(stripNonCode("const n = 1 + /ab/.source.length;", "x.ts")).not.toContain("ab");
  });

  it("does not read a TypeScript non-null assertion as an operator a regex may follow", () => {
    const swallow = "const r = maybe! / (liveMarker(), 2) / 3;";
    expect(stripNonCode(swallow, "x.ts")).toContain("liveMarker()");
    for (const value of ["f()", "arr[0]", '"s"']) {
      expect(stripNonCode(`const r = ${value}! / (liveMarker(), 2) / 3;`, "x.ts"), value).toContain(
        "liveMarker()",
      );
    }
    const prefix = "if (!/don't/.test(s)) { live(); } // decoy\n";
    expect(stripNonCode(prefix, "x.ts")).toContain("live()");
    expect(stripComments(prefix, "x.ts")).not.toContain("decoy");
  });

  it("knows the keywords that do not end a value, and where a `)` does not either", () => {
    for (const decoyed of [
      "throw /don't/; /* if (m.startsWith(DEACTIVATION_KEYCHAIN_REMNANT)) { */ throw /won't/;",
      "if (ok) /don't/.test(s); /* if (m.startsWith(DEACTIVATION_KEYCHAIN_REMNANT)) { */ if (ok) /won't/.test(s);",
    ]) {
      expect(stripComments(decoyed, "Panel.tsx"), decoyed).not.toContain("startsWith(DEACTIVATION");
    }
    expect(
      stripComments("for await (const x of xs) /don't/.test(x); // decoy '\n", "x.ts"),
    ).not.toContain("decoy");
    expect(stripNonCode("const half = total(a) / 2; // decoy\n", "x.ts")).toContain("total(a) / 2");
    expect(stripComments("const q = obj.throw / 2; // decoy\n", "x.ts")).not.toContain("decoy");
  });

  it("scans inside a template literal's interpolations", () => {
    const decoy =
      "const t = `x ${ /* message.startsWith(DEACTIVATION_KEYCHAIN_REMNANT) */ y } z`;\nconst live = 1;";
    expect(stripComments(decoy, "x.ts")).not.toContain("startsWith(DEACTIVATION");
    const blanked = stripNonCode("const t = `gone ${live} gone`;", "x.ts");
    expect(blanked).not.toContain("gone");
    expect(blanked).toContain("live");
    const braces = (text, ch) => text.split(ch).length - 1;
    expect(braces(blanked, "{")).toBe(braces(blanked, "}"));
    const nestedHole = stripNonCode("const t = `a ${ f({ k: 1 }) } b`;", "x.ts");
    expect(braces(nestedHole, "{")).toBe(braces(nestedHole, "}"));
    expect(stripNonCode("const t = `a ${ `b ${inner} c` } d`;", "x.ts")).toContain("inner");
    for (const source of [decoy, "const t = `a ${ `b ${c} d` } e`;"]) {
      expect(stripComments(source, "x.ts")).toHaveLength(source.length);
      expect(stripNonCode(source, "x.ts")).toHaveLength(source.length);
    }
  });

  it("catches the completed-unlink marker moved out of the prefix position", () => {
    const displaced = mutate(
      LIFECYCLE_DEACTIVATION,
      '"{DEACTIVATION_KEYCHAIN_REMNANT}This machine was unlinked {}.{}"',
      '"This machine was unlinked {DEACTIVATION_KEYCHAIN_REMNANT}{}.{}"',
    );
    expect(licenseCodeUnionFailures(displaced)).not.toEqual([]);
  });

  it("catches an arm that mints its error somewhere the count cannot reach", () => {
    const delegated = mutate(
      LIFECYCLE_DEACTIVATION,
      "None if leaves_a_stranded_seat(upstream_release) => Err(format!(",
      "None if leaves_a_stranded_seat(upstream_release) => unmarked_lost_result(format!(",
    );
    expect(
      licenseCodeUnionFailures(delegated).some((f) => f.includes("unmarked_lost_result")),
    ).toBe(true);
  });

  it("catches an unmarked error built a line before it is returned", () => {
    const indirect = mutate(
      LIFECYCLE_DEACTIVATION,
      `        None if leaves_a_stranded_seat(upstream_release) => Err(format!(
            "{DEACTIVATION_KEYCHAIN_REMNANT}This machine was unlinked {}.{}",
            released_clause(upstream_release),
            remaining_work(upstream_release)
        )),`,
      `        None if leaves_a_stranded_seat(upstream_release) => {
            let message = format!(
                "This machine was unlinked {}.{}",
                released_clause(upstream_release),
                remaining_work(upstream_release)
            );
            Err(message)
        }`,
    );
    expect(licenseCodeUnionFailures(indirect)).not.toEqual([]);
  });
});

describe("the licensing surface rules check the thing they are named for", () => {
  const real = (file) => fs.readFileSync(path.join(ROOT, file), "utf8");
  const ACTIVATION =
    "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle_activation.rs";
  const LIFECYCLE_ROOT = "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle.rs";
  const COMMANDS_DIR = "apps/desktop/src-tauri/src/licensing/commands";
  const mutate = (file, from, to) => {
    const source = real(file);
    const next = source.replace(from, to);
    expect(next, `mutation of ${file} did not apply; the anchor moved`).not.toBe(source);
    return (asked) => (asked === file ? next : real(asked));
  };

  const realList = (dir, predicate, files = []) => {
    for (const entry of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
      const relative = path.join(dir, entry.name);
      if (entry.isDirectory()) realList(relative, predicate, files);
      else if (predicate(relative)) files.push(relative);
    }
    return files;
  };
  const surface = (read, listFiles = realList) => licenseSurfaceFailures(read, listFiles);

  it("passes on the real tree", () => {
    expect(surface(real)).toEqual([]);
  });

  it("catches a lifecycle module that compiles but is in neither enumeration", () => {
    const declared = mutate(
      LIFECYCLE_ROOT,
      '#[path = "license_lifecycle_validation.rs"]',
      '#[path = "license_lifecycle_v2.rs"]\nmod v2;\n#[path = "license_lifecycle_validation.rs"]',
    );
    const failures = surface(declared);
    expect(failures.some((f) => f.includes("LIFECYCLE_SOURCES"))).toBe(true);
  });

  it("catches the raw license key reaching an audit row", () => {
    const leaked = mutate(
      ACTIVATION,
      "let audit_detail = license_activation_audit_detail(&key_fingerprint);",
      'let audit_detail = serde_json::json!({ "license_key": &key });',
    );
    expect(surface(leaked)).not.toEqual([]);
    const inlineLeak = mutate(
      ACTIVATION,
      '            "key_fingerprint": key_fingerprint,',
      '            "key": key.clone(),',
    );
    expect(surface(inlineLeak)).not.toEqual([]);
  });

  it("catches one activation rejection route losing its typed constructor", () => {
    const raw = mutate(
      ACTIVATION,
      "return Err(provider_refusal_error(",
      "return Err(String::from(",
    );
    expect(surface(raw)).not.toEqual([]);
    const bare = mutate(
      ACTIVATION,
      "return Err(activation_error(LicenseActivationErrorCode::VariantUnknown));",
      'return Err("Activation failed".to_string());',
    );
    expect(surface(bare)).not.toEqual([]);
  });

  it("catches a raw provider body concatenated onto a typed refusal", () => {
    const concatenated = mutate(
      ACTIVATION,
      "return Err(activation_error_from_raw(&error));",
      "return Err(activation_error(LicenseActivationErrorCode::Incomplete) + &error);",
    );
    expect(surface(concatenated)).not.toEqual([]);
  });

  it("catches a raw error propagated out of the command with `?`", () => {
    const propagated = mutate(
      ACTIVATION,
      "    let key = normalize_license_key(&key);",
      "    let raw: Result<(), String> = api::preflight(&key).await;\n    raw?;\n    let key = normalize_license_key(&key);",
    );
    expect(surface(propagated).some((f) => f.includes("propagates"))).toBe(true);
  });

  it("catches an audit_detail shadowed after the safe binding", () => {
    const shadowed = mutate(
      ACTIVATION,
      "let audit_detail = license_activation_audit_detail(&key_fingerprint);",
      'let audit_detail = license_activation_audit_detail(&key_fingerprint);\n    let audit_detail = serde_json::json!({ "license_key": key.clone() });',
    );
    expect(surface(shadowed).some((f) => f.includes("binds it again"))).toBe(true);
  });

  it("catches a lifecycle module on disk that no #[path] in the root declares", () => {
    const NEW = `${COMMANDS_DIR}/license_lifecycle_catalog.rs`;
    const withModule = (dir, predicate) => [
      ...realList(dir, predicate),
      ...(dir === COMMANDS_DIR ? [NEW].filter(predicate) : []),
    ];
    const failures = surface(
      (file) => (file === NEW ? "pub fn mint_catalog_credential() {}\n" : real(file)),
      withModule,
    );
    expect(failures.some((f) => f.includes("LIFECYCLE_SOURCES"))).toBe(true);
  });

  it("catches audit_detail shadowed through a PATTERN binding", () => {
    const shadowed = mutate(
      ACTIVATION,
      "let audit_detail = license_activation_audit_detail(&key_fingerprint);",
      "let audit_detail = license_activation_audit_detail(&key_fingerprint);\n" +
        '    let (audit_detail,) = (serde_json::json!({ "key_fingerprint": key.to_string() }),);',
    );
    expect(surface(shadowed).some((f) => f.includes("binds it again"))).toBe(true);
  });

  for (const [label, spelling] of [
    ["key.as_str()", "key.as_str()"],
    ["key.to_string()", "key.to_string()"],
    ['format!("{key}")', 'format!("{key}")'],
  ]) {
    it(`catches the raw key spelled ${label} in an audit detail`, () => {
      const leaked = mutate(
        ACTIVATION,
        'ports.audit(audit_detail.clone(), "fail");',
        `ports.audit(serde_json::json!({ "key_fingerprint": key_fingerprint, "k": ${spelling} }), "fail");`,
      );
      expect(surface(leaked).some((f) => f.includes("beside"))).toBe(true);
    });
  }

  it("catches the raw key in a later argument of a record call split over lines", () => {
    const spread = mutate(
      ACTIVATION,
      'ports.audit(audit_detail.clone(), "fail");',
      'ports.audit(\n            audit_detail,\n            &format!("{}", key.clone()),\n        );',
    );
    expect(surface(spread).some((f) => f.includes("beside"))).toBe(true);
  });

  for (const [label, shadow] of [
    [
      "a match arm",
      'Err(error) => match serde_json::json!({ "license_key": key.clone() }) {\n            audit_detail => {\n                ports.audit(audit_detail.clone(), "fail");\n                return Err(activation_error_from_raw(&error));\n            }\n        },',
    ],
    [
      "a `for` pattern",
      'Err(error) => {\n            for audit_detail in [serde_json::json!({ "license_key": key.clone() })] {\n                ports.audit(audit_detail, "fail");\n            }\n            return Err(activation_error_from_raw(&error));\n        }',
    ],
  ]) {
    it(`catches audit_detail rebound by ${label}, which writes no \`let\` at all`, () => {
      const rebound = mutate(
        ACTIVATION,
        'Err(error) => {\n            ports.audit(audit_detail.clone(), "fail");\n            return Err(activation_error_from_raw(&error));\n        }',
        shadow,
      );
      expect(surface(rebound).some((f) => f.includes("binds it again"))).toBe(true);
    });
  }

  it("catches a `?` excused by a map_err that types someone else's error", () => {
    const nested = mutate(
      ACTIVATION,
      "    let tier = ports.tier_for_variant(result.variant_id);",
      "    let raw: Result<(), String> = Err(key.clone());\n" +
        "    let _nested = raw.map(|()| {\n" +
        "        Result::<(), String>::Ok(()).map_err(|error| activation_error_from_raw(&error))\n" +
        "    })?;\n" +
        "    let tier = ports.tier_for_variant(result.variant_id);",
    );
    expect(surface(nested).some((f) => f.includes("propagates"))).toBe(true);
  });

  it("catches the raw key renamed into a fresh local", () => {
    const renamed = mutate(
      ACTIVATION,
      '            ports.audit(audit_detail.clone(), "fail");',
      "        let k = key.clone();\n" +
        '            ports.audit(serde_json::json!({ "key_fingerprint": key_fingerprint, "trace": k }), "fail");',
    );
    expect(surface(renamed).some((f) => f.includes("not allowed to carry"))).toBe(true);
  });

  it("catches a `?` excused by a map_err that converts nothing", () => {
    const identity = mutate(
      ACTIVATION,
      "    let tier = ports.tier_for_variant(result.variant_id);",
      "    let raw: Result<(), String> = Err(key.clone());\n" +
        "    raw.map_err(|error| error)?;\n" +
        "    let tier = ports.tier_for_variant(result.variant_id);",
    );
    expect(surface(identity).some((f) => f.includes("propagates"))).toBe(true);
  });

  it("does not let an earlier `?` promote a nested map_err to the outermost one", () => {
    const negative = mutate(
      ACTIVATION,
      "    let tier = ports.tier_for_variant(result.variant_id);",
      "    let inner: Result<(), String> = Ok(());\n" +
        "    let _missed = (\n" +
        "        Result::<(), String>::Ok(())?,\n" +
        "        Result::<(), String>::Err(key.clone()),\n" +
        "    )\n" +
        "        .1\n" +
        "        .map(|()| inner.map_err(|error| activation_error_from_raw(&error)))?;\n" +
        "    let tier = ports.tier_for_variant(result.variant_id);",
    );
    expect(surface(negative).some((f) => f.includes("propagates"))).toBe(true);
  });

  it("catches the raw key riding along BESIDE the fingerprint", () => {
    const beside = mutate(
      ACTIVATION,
      '            "key_fingerprint": key_fingerprint,',
      '            "key_fingerprint": key_fingerprint,\n            "license_key": key.clone(),',
    );
    expect(surface(beside).some((f) => f.includes("beside"))).toBe(true);
  });
});
