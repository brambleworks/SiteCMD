import ts from "typescript";

const GUIDE_DIRS = ["apps/desktop/src/lib/fix-guides", "apps/desktop/src/lib/code-fix-guides"];
const NON_CONTENT_FILES = new Set(["index.ts", "types.ts"]);

// Shared free-baseline content bounds.
const MAX_BASELINE_STEPS = 2;
const MAX_BASELINE_STEP_CHARS = 600;

function staticPropertyName(name) {
  if (name && (ts.isIdentifier(name) || ts.isStringLiteral(name))) {
    return name.text;
  }
  return null;
}

function isGuideRecordType(type) {
  return (
    type &&
    ts.isTypeReferenceNode(type) &&
    ts.isIdentifier(type.typeName) &&
    type.typeName.text === "Record" &&
    type.typeArguments?.length === 2 &&
    type.typeArguments[0].kind === ts.SyntaxKind.StringKeyword
  );
}

function parseGuideModule(source, file, failures) {
  const sourceFile = ts.createSourceFile(
    file,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  if (sourceFile.parseDiagnostics.length > 0) {
    const detail = ts.flattenDiagnosticMessageText(sourceFile.parseDiagnostics[0].messageText, " ");
    failures.push(`${file}: baseline guide module has invalid TypeScript: ${detail}`);
    return null;
  }

  const runtimeImports = sourceFile.statements.filter(
    (statement) => ts.isImportDeclaration(statement) && !statement.importClause?.isTypeOnly,
  );
  const declarations = sourceFile.statements.filter(
    (statement) => !ts.isImportDeclaration(statement),
  );
  const statement = declarations[0];
  const exported = statement?.modifiers?.some(
    (modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword,
  );
  const variableStatement = statement && ts.isVariableStatement(statement) ? statement : null;
  const isConst =
    variableStatement && (variableStatement.declarationList.flags & ts.NodeFlags.Const) !== 0;
  const declaration = variableStatement?.declarationList.declarations[0] ?? null;

  if (
    runtimeImports.length > 0 ||
    declarations.length !== 1 ||
    !isConst ||
    !exported ||
    variableStatement.declarationList.declarations.length !== 1 ||
    !declaration ||
    !ts.isIdentifier(declaration.name) ||
    !isGuideRecordType(declaration.type) ||
    !declaration.initializer ||
    !ts.isObjectLiteralExpression(declaration.initializer)
  ) {
    failures.push(
      `${file}: baseline guide module must be a single typed exported record initialized with an object literal and using type-only imports`,
    );
    return null;
  }

  const guides = [];
  for (const property of declaration.initializer.properties) {
    if (!ts.isPropertyAssignment(property)) {
      failures.push(`${file}: baseline guide entries must use static property assignments`);
      continue;
    }
    const checkId = staticPropertyName(property.name);
    if (!checkId) {
      failures.push(`${file}: baseline guide entries must use static property assignments`);
      continue;
    }
    if (!ts.isObjectLiteralExpression(property.initializer)) {
      failures.push(`${file}: baseline ${checkId} must be an object literal`);
      continue;
    }

    const defaults = property.initializer.properties.filter(
      (entryProperty) => staticPropertyName(entryProperty.name) === "default",
    );
    if (
      defaults.length !== 1 ||
      !ts.isPropertyAssignment(defaults[0]) ||
      !ts.isArrayLiteralExpression(defaults[0].initializer)
    ) {
      failures.push(`${file}: baseline ${checkId} must have one literal default step array`);
      continue;
    }

    const steps = [];
    for (const element of defaults[0].initializer.elements) {
      if (ts.isStringLiteral(element) || ts.isNoSubstitutionTemplateLiteral(element)) {
        steps.push(element.text);
      } else {
        failures.push(`${file}: baseline ${checkId} steps must be string literals`);
      }
    }
    guides.push([checkId, steps]);
  }
  return guides;
}

export function baselineGuideShapeFailures(read, listFiles) {
  const failures = [];

  for (const dir of GUIDE_DIRS) {
    const contentFiles = listFiles(dir, (file) => file.endsWith(".ts")).filter(
      (file) => !NON_CONTENT_FILES.has(file.split("/").pop()),
    );
    for (const file of contentFiles) {
      const source = read(file);

      // Belt: the literal key never appears, even in a shape the evaluator
      // might miss (a spread, a helper, a renamed const).
      if (/\bframeworks\s*:/.test(source)) {
        failures.push(
          `${file}: baseline guides must not carry framework variants; stack-specific depth belongs to the catalog corpus in SiteCMD-Web`,
        );
      }

      const guides = parseGuideModule(source, file, failures);
      if (!guides) continue;

      for (const [checkId, steps] of guides) {
        if (steps.length === 0) {
          failures.push(`${file}: baseline ${checkId} has no steps`);
        }
        if (steps.length > MAX_BASELINE_STEPS) {
          failures.push(
            `${file}: baseline ${checkId} has ${steps.length} steps; more than ${MAX_BASELINE_STEPS} is deep-guide content and belongs to the catalog corpus in SiteCMD-Web`,
          );
        }
        for (const step of steps) {
          if (typeof step !== "string") continue;
          if (step.length > MAX_BASELINE_STEP_CHARS) {
            failures.push(
              `${file}: baseline ${checkId} has a ${step.length}-character step; over ${MAX_BASELINE_STEP_CHARS} is deep-guide content and belongs to the catalog corpus in SiteCMD-Web`,
            );
          }
          if (step.includes("```")) {
            failures.push(
              `${file}: baseline ${checkId} carries a fenced code block; baselines use inline code only, worked examples belong to the catalog corpus in SiteCMD-Web`,
            );
          }
        }
      }
    }
  }

  return failures;
}
