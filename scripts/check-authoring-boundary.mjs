import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { pathToFileURL } from 'node:url';
import * as ts from 'typescript';

const root = process.cwd();
const authoringRoot = join(root, 'packages', 'authoring', 'src');
const irRoot = join(root, 'packages', 'ir', 'src');

const semanticCallNames = new Set([
  'advanceeffecttiming',
  'applyeffect',
  'applystacking',
  'evaluateformula',
  'evaluatepredicate',
  'mutateauthority',
  'resolveattack',
  'resolvecheck',
  'rolldice',
  'rolldie',
  'testlegality',
]);
const semanticCallbackNames = new Set([
  'apply',
  'execute',
  'evaluate',
  'mutate',
  'onhit',
  'resolve',
]);
const authorityObjectNames = new Set([
  'authority',
  'authoritystate',
  'capabilitystore',
  'gameplaycontext',
  'mutationcontext',
  'resolutioncontext',
]);
const privateIrLayoutNames = new Set([
  'capabilitystore',
  'compiledoperation',
  'compiledprogram',
  'compiledruleset',
  'mutationworkspace',
  'resolutionworkspace',
  'stagedstate',
  'storelayout',
]);
const browserGlobalNames = new Set([
  'document',
  'fetch',
  'localstorage',
  'navigator',
  'sessionstorage',
  'websocket',
  'window',
]);

export function inspectAuthoringBoundary(
  source,
  fileName = 'fixture.ts',
  options = {},
) {
  const sourceFile = ts.createSourceFile(
    fileName,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const diagnostics = [];
  const seen = new Set();
  const report = (node, message) => {
    const position = sourceFile.getLineAndCharacterOfPosition(
      node.getStart(sourceFile),
    );
    const diagnostic = `${fileName}:${position.line + 1}:${position.character + 1}: ${message}`;
    if (!seen.has(diagnostic)) {
      seen.add(diagnostic);
      diagnostics.push(diagnostic);
    }
  };

  const visit = (node) => {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
      const specifier = node.moduleSpecifier.text.toLowerCase();
      if (
        specifier.startsWith('@angular/') ||
        specifier.includes('rulebench') ||
        specifier.includes('transport') ||
        specifier.includes('process-host')
      ) {
        report(
          node,
          `portable TypeScript may not import product, host, transport, or Angular surface ${node.moduleSpecifier.text}`,
        );
      }
    }

    if (ts.isCallExpression(node)) {
      const name = normalizedCallName(node.expression);
      if (semanticCallNames.has(name)) {
        report(
          node,
          `TypeScript authoring may compose published operations but may not execute ${node.expression.getText(sourceFile)}`,
        );
      }
      if (
        ts.isPropertyAccessExpression(node.expression) &&
        authorityObjectNames.has(normalizedName(node.expression.expression))
      ) {
        report(
          node,
          `TypeScript authoring may not call private authority or capability-store surface ${node.expression.getText(sourceFile)}`,
        );
      }
      if (browserGlobalNames.has(name)) {
        report(node, `portable TypeScript may not call browser global ${name}`);
      }
    }

    if (
      ts.isNewExpression(node) &&
      browserGlobalNames.has(normalizedName(node.expression))
    ) {
      report(
        node,
        `portable TypeScript may not construct browser global ${node.expression.getText(sourceFile)}`,
      );
    }

    if (ts.isPropertyAccessExpression(node)) {
      const rootName = normalizedName(node.expression);
      if (browserGlobalNames.has(rootName)) {
        report(
          node,
          `portable TypeScript may not access browser global ${node.expression.getText(sourceFile)}`,
        );
      }
      if (authorityObjectNames.has(rootName)) {
        report(
          node,
          `TypeScript authoring may not inspect private authority or capability-store surface ${node.expression.getText(sourceFile)}`,
        );
      }
    }

    if (
      ts.isPropertyAssignment(node) &&
      semanticCallbackNames.has(normalizedName(node.name)) &&
      (ts.isArrowFunction(node.initializer) ||
        ts.isFunctionExpression(node.initializer))
    ) {
      report(
        node,
        `normalized authoring data may not contain executable semantic callback ${node.name.getText(sourceFile)}`,
      );
    }

    if (
      ts.isBinaryExpression(node) &&
      isAssignmentOperator(node.operatorToken.kind) &&
      containsAuthorityObject(node.left)
    ) {
      report(
        node,
        `TypeScript authoring may not mutate private authority context ${node.left.getText(sourceFile)}`,
      );
    }

    if (
      options.normalizedIr === true &&
      (ts.isPropertySignature(node) || ts.isPropertyDeclaration(node)) &&
      privateIrLayoutNames.has(normalizedName(node.name))
    ) {
      report(
        node,
        `normalized IR may not mirror private Rust runtime layout ${node.name.getText(sourceFile)}`,
      );
    }

    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return diagnostics;
}

function normalizedCallName(expression) {
  if (ts.isIdentifier(expression)) return normalizedName(expression);
  if (ts.isPropertyAccessExpression(expression)) {
    return normalizedName(expression.name);
  }
  return '';
}

function normalizedName(node) {
  if (ts.isIdentifier(node)) return node.text.toLowerCase();
  if (ts.isStringLiteral(node) || ts.isNumericLiteral(node)) {
    return node.text.toLowerCase();
  }
  return node
    .getText()
    .replace(/[^A-Za-z0-9]/g, '')
    .toLowerCase();
}

function containsAuthorityObject(node) {
  if (ts.isIdentifier(node)) return authorityObjectNames.has(normalizedName(node));
  if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
    return containsAuthorityObject(node.expression);
  }
  return false;
}

function isAssignmentOperator(kind) {
  return (
    kind >= ts.SyntaxKind.FirstAssignment &&
    kind <= ts.SyntaxKind.LastAssignment
  );
}

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

function run() {
  const inputs = [
    ...filesBelow(authoringRoot)
      .filter((path) => path.endsWith('.ts'))
      .map((path) => ({ path, normalizedIr: false })),
    ...filesBelow(irRoot)
      .filter(
        (path) => path.endsWith('.ts') && !path.endsWith('generated-vocabulary.ts'),
      )
      .map((path) => ({ path, normalizedIr: true })),
  ];
  const diagnostics = inputs.flatMap(({ path, normalizedIr }) =>
    inspectAuthoringBoundary(
      readFileSync(path, 'utf8'),
      relative(root, path),
      { normalizedIr },
    ),
  );
  if (diagnostics.length > 0) {
    console.error(diagnostics.join('\n'));
    process.exit(1);
  }
  console.log(
    `authoring boundary check ok (${inputs.length} portable TypeScript files)`,
  );
}

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  run();
}
