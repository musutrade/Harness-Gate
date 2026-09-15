"use strict";
const test = require("node:test"),
  assert = require("node:assert/strict");
const fs = require("node:fs"),
  path = require("node:path"),
  os = require("node:os"),
  vm = require("node:vm");
const { spawnSync } = require("node:child_process");
const ts = require("typescript"),
  { createInstrumenter } = require("istanbul-lib-instrument");
const {
  COLLECTOR,
  canonical,
  discover,
  binding,
  collect,
} = require("./protocol.cjs");
const { sha } = require("./measure.cjs");
const binary = process.env.HARNESS_GATE_BINARY || "harness-gate";
let retainedCase = 0;
function fixture(cc, invoke, action) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "ts-core-"));
  try {
    fs.mkdirSync(path.join(root, "src"));
    fs.mkdirSync(path.join(root, "artifacts"));
    const source =
      "function subject(x: number) {\n" +
      Array.from(
        { length: cc - 1 },
        (_, i) => ` if (x === ${i}) {\n  return ${i};\n }\n`,
      ).join("") +
      " return -1;\n}\n";
    const sourcePath = "src/subject.ts",
      nativePath = "/fixture/" + sourcePath;
    fs.writeFileSync(path.join(root, sourcePath), source);
    const instrumenter = createInstrumenter({
      parserPlugins: ["typescript"],
      produceSourceMap: true,
    });
    const js = ts.transpileModule(
      instrumenter.instrumentSync(source, nativePath),
      { compilerOptions: { target: ts.ScriptTarget.ES2022 } },
    ).outputText;
    const sandbox = {};
    vm.runInNewContext(js + "\n" + invoke, sandbox, { timeout: 1000 });
    const coverage = JSON.stringify(sandbox.__coverage__);
    fs.writeFileSync(path.join(root, "coverage.json"), coverage);
    const request = {
      schema: "harness-collector-request/v1",
      project: "native-typescript",
      component: "frontend",
      collector: COLLECTOR,
      context: {
        commit: "1".repeat(40),
        base_commit: "0".repeat(40),
        target: "node",
        run: "fresh-original-instrumentation",
      },
      requested_capabilities: ["risk.crap"],
      workspace_root: root,
      output_root: path.join(root, "artifacts"),
      parameters: {
        source_root: "src",
        boundary: "production",
        coverage: "coverage.json",
      },
    };
    const inv = discover(request);
    request.parameters.subjects = inv.subjects;
    request.parameters.receipt = {
      schema: "typescript-original-coverage-receipt/v1",
      request: binding(request),
      sources: inv.sources.map(({ path, sha256 }) => ({ path, sha256 })),
      coverage_sha256: sha(coverage),
      coverage_root: "/fixture",
      toolchain: {
        typescript: "6.0.2",
        instrumenter: "istanbul-lib-instrument@6.0.3",
        instrumentation: "original-typescript-before-transpile/v1",
        node: process.versions.node,
      },
    };
    const response = process.env.HARNESS_GATE_TYPESCRIPT_CLI
      ? JSON.parse(spawnSync(process.execPath, [process.env.HARNESS_GATE_TYPESCRIPT_CLI], {input:canonical(request),encoding:'utf8'}).stdout)
      : collect(request);
    const project = {
      schema: "harness-project/v1",
      id: request.project,
      metadata: {},
      relationships: [],
      components: [
        {
          id: "frontend",
          path: "src",
          metadata: {},
          targets: [{ id: "node", boundaries: ["production"], metadata: {} }],
          source_boundaries: [
            { id: "production", path: "src", role: "production", metadata: {} },
          ],
        },
      ],
      subjects: inv.subjects,
    };
    const policy = {
      schema: "harness-policy/v1",
      rules: [
        {
          id: "frontend.crap",
          scope: { kind: "component", component: "frontend" },
          metric: "risk.crap",
          operator: "le",
          limit: { type: "rational", numerator: 10, denominator: 1 },
          required: true,
          on_violation: "fail",
          remediation_classes: ["reduce_risk"],
        },
      ],
    };
    for (const [name, data] of [
      ["project", project],
      ["policy", policy],
      ["evidence", response.evidence],
      ["expected", request.context],
    ])
      fs.writeFileSync(path.join(root, name + ".json"), canonical(data));
    action({ root, request, response });
    if (process.env.HARNESS_GATE_TYPESCRIPT_ACCEPTANCE) {
      const destination=path.join(process.env.HARNESS_GATE_TYPESCRIPT_ACCEPTANCE, `case-${++retainedCase}`);
      assert(!fs.existsSync(destination),'refusing stale acceptance output');
      fs.cpSync(root,destination,{recursive:true});
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}
function evaluate(root) {
  const args = [
    "quality",
    "evaluate",
    "--source-root",
    root,
    "--artifact-root",
    path.join(root, "artifacts"),
    "--output",
    path.join(root, "report.json"),
  ];
  for (const name of ["project", "policy", "evidence", "expected"])
    args.push("--" + name, path.join(root, name + ".json"));
  const result = spawnSync(binary, args, { encoding: "utf8" });
  const report = fs.existsSync(path.join(root, "report.json"))
    ? JSON.parse(fs.readFileSync(path.join(root, "report.json"), "utf8"))
    : null;
  return { result, report };
}
test("released Core alone decides the exact 10 boundary from fresh native evidence", () => {
  const version = spawnSync(binary, ["--version"], { encoding: "utf8" });
  assert.equal(version.stdout.trim(), "harness-gate 0.4.5");
  for (const [cc, invoke, state] of [
    [10, "for(let i=-1;i<10;i++) subject(i);", "pass"],
    [11, "for(let i=-1;i<11;i++) subject(i);", "fail"],
    [10, "subject(-1);", "fail"],
  ]) {
    fixture(cc, invoke, ({ root }) => {
      const { result, report } = evaluate(root);
      assert(report, result.stderr + result.stdout);
      assert.equal(report.aggregate.state, state, JSON.stringify(report));
      assert.equal(
        result.status,
        state === "pass" ? 0 : 1,
        result.stderr + result.stdout,
      );
    });
  }
});
test("Core rejects evidence whose retained bytes were changed after collection", () => {
  fixture(1, "subject(0);", ({ root, response, request }) => {
    fs.appendFileSync(
      path.join(request.output_root, response.artifacts[0].path),
      " ",
    );
    const { result, report } = evaluate(root);
    assert.notEqual(result.status, 0);
    assert.notEqual(report?.aggregate?.state, "pass");
  });
});
test("generic collector transport validates independent executable and artifact inventory", () => {
  fixture(2, "subject(0); subject(2);", ({ root, request }) => {
    request.output_root = path.join(root, "transport-artifacts");
    fs.mkdirSync(request.output_root);
    fs.writeFileSync(path.join(root, "request.json"), canonical(request));
    const code =
      'import json,sys; sys.path.insert(0,sys.argv[1]); from collector_runner import run_collector,SubprocessAdapter; from pathlib import Path; root=Path(sys.argv[2]); request=json.loads((root/"request.json").read_text()); project=json.loads((root/"project.json").read_text()); rows=run_collector(SubprocessAdapter((sys.argv[3],sys.argv[4])),request,project=project); assert len(rows)==1; print("generic transport validated")';
    const result = spawnSync(
      "python3",
      [
        "-c",
        code,
        path.resolve(__dirname, ".."),
        root,
        process.execPath,
        process.env.HARNESS_GATE_TYPESCRIPT_CLI || path.join(__dirname, "cli.cjs"),
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /validated/);
  });
});
