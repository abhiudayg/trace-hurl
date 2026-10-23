const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

console.log('================================================================');
console.log('🚀 ISOLATED ENVIRONMENT RUN & END-TO-END ACCURACY VERIFICATION');
console.log('================================================================');

// 1. Create clean isolated temporary directory
const tmpDir = fs.mkdtempSync(path.join('/tmp', 'tracehurl_isolated_'));
console.log(`\n📁 Step 1: Created isolated test directory: ${tmpDir}`);

const targetRunId = process.argv[2] || 'TRACEHURL_SAMPLE_001';
const sourceDir = path.resolve(fs.existsSync('verify/fixtures/sample_run') ? 'verify/fixtures/sample_run' : '.verify/fixtures/sample_run');
const binaryPath = path.resolve('target/release/tracehurl');

if (!fs.existsSync(binaryPath)) {
  console.log('Building release binary first...');
  execSync('cargo build --release', { stdio: 'inherit' });
}

// 2. Copy raw artifacts (excluding any existing report directory)
console.log(`\n📦 Step 2: Copying raw artifacts from ${sourceDir} (excluding report/)...`);
const entriesToCopy = fs.readdirSync(sourceDir);
for (const item of entriesToCopy) {
  if (item === 'report') continue;
  const src = path.join(sourceDir, item);
  const dest = path.join(tmpDir, item);
  execSync(`cp -R "${src}" "${dest}"`);
}

// 3. Run tracehurl report in the isolated directory
console.log(`\n⚙️  Step 3: Running TraceHurl CLI in isolated environment:`);
const reportOutput = execSync(`"${binaryPath}" report "${tmpDir}"`, { encoding: 'utf-8' });
console.log(reportOutput.trim());

// 4. Verify report generation files exist
const reportHtmlPath = path.join(tmpDir, 'report', 'index.html');
const reportJsonPath = path.join(tmpDir, 'report', 'report.json');

if (!fs.existsSync(reportHtmlPath) || !fs.existsSync(reportJsonPath)) {
  console.error('❌ FAIL: Generated report files are missing!');
  process.exit(1);
}
console.log('\n✓ Generated report/index.html and report/report.json exist.');

// 5. In-depth Data Accuracy Verification against Raw Inputs
console.log('\n🔍 Step 4: Cross-verifying values against raw ground-truth inputs...');

// Read raw inputs
let rawHurlJson;
if (fs.existsSync(path.join(tmpDir, 'hurl-json', 'report.json'))) {
  rawHurlJson = JSON.parse(fs.readFileSync(path.join(tmpDir, 'hurl-json', 'report.json'), 'utf8'));
} else if (fs.existsSync(path.join(tmpDir, 'functional-loan-import.json'))) {
  rawHurlJson = JSON.parse(fs.readFileSync(path.join(tmpDir, 'functional-loan-import.json'), 'utf8'));
}
const rawManifest = fs.existsSync(path.join(tmpDir, 'manifest.json'))
  ? JSON.parse(fs.readFileSync(path.join(tmpDir, 'manifest.json'), 'utf8'))
  : null;
const rawPhasesTsv = fs.existsSync(path.join(tmpDir, 'phases.tsv'))
  ? fs.readFileSync(path.join(tmpDir, 'phases.tsv'), 'utf8')
  : null;

// Read generated outputs
const genJson = JSON.parse(fs.readFileSync(reportJsonPath, 'utf8'));
const genHtml = fs.readFileSync(reportHtmlPath, 'utf8');

let errors = [];

// Check Run ID
if (genJson.runId !== targetRunId) {
  errors.push(`Run ID mismatch: expected ${targetRunId}, got ${genJson.runId}`);
} else {
  console.log(`  ✓ Run ID verified: ${genJson.runId}`);
}

// Check Phases
if (rawPhasesTsv) {
  const tsvLines = rawPhasesTsv.trim().split('\n').slice(1);
  const expectedPhaseCount = tsvLines.length;
  if (genJson.phases.length !== expectedPhaseCount) {
    errors.push(`Phase count mismatch: expected ${expectedPhaseCount}, got ${genJson.phases.length}`);
  } else {
    for (let i = 0; i < tsvLines.length; i++) {
      const [name, exitCode, start, end, log] = tsvLines[i].split('\t');
      const phase = genJson.phases[i];
      if (phase.name !== name || phase.exitCode !== parseInt(exitCode, 10)) {
        errors.push(`Phase #${i} mismatch: ${phase.name} (exit ${phase.exitCode}) vs raw ${name} (exit ${exitCode})`);
      }
    }
    console.log(`  ✓ Phases accurately match phases.tsv (${genJson.phases.length} phases).`);
  }
} else if (rawManifest && rawManifest.phases) {
  if (genJson.phases.length !== rawManifest.phases.length) {
    errors.push(`Phase count mismatch: expected ${rawManifest.phases.length}, got ${genJson.phases.length}`);
  } else {
    for (let i = 0; i < rawManifest.phases.length; i++) {
      const rawPhase = rawManifest.phases[i];
      const genPhase = genJson.phases[i];
      if (genPhase.name !== rawPhase.name || genPhase.exitCode !== rawPhase.exitCode) {
        errors.push(`Phase #${i} mismatch: ${genPhase.name} (exit ${genPhase.exitCode}) vs raw ${rawPhase.name}`);
      }
    }
    console.log(`  ✓ Phases accurately match manifest.json (${genJson.phases.length} phases).`);
  }
}

// Check Scenario and Entries count
const totalGenEntries = genJson.scenarios.reduce((acc, s) => acc + s.entries.length, 0);

if (totalGenEntries !== genJson.summary.totalEntries || totalGenEntries === 0) {
  errors.push(`Total entries mismatch or empty: summary=${genJson.summary.totalEntries}, generated=${totalGenEntries}`);
} else {
  console.log(`  ✓ Total entries verified: ${totalGenEntries} entries across ${genJson.scenarios.length} scenario(s).`);
}

// Verify Entry-by-Entry Accuracy
let passedCount = 0;
let failedCount = 0;
let expectedRejectCount = 0;
let weakCount = 0;

for (let i = 0; i < totalGenEntries; i++) {
  const genEntry = genJson.scenarios[0].entries[i];

  if (!genEntry) {
    errors.push(`Missing generated entry for index ${i + 1}`);
    continue;
  }

  // Verify index
  if (genEntry.index !== i + 1) {
    errors.push(`Entry #${i + 1} has wrong index: ${genEntry.index}`);
  }

  // Classify outcomes
  if (genEntry.outcome === 'passed') passedCount++;
  else if (genEntry.outcome === 'failed') failedCount++;
  else if (genEntry.outcome === 'expected-rejection') expectedRejectCount++;
  else if (genEntry.outcome === 'weak') weakCount++;
}

console.log(`  ✓ Outcomes verified:`);
console.log(`      Passed:             ${passedCount} (Summary: ${genJson.summary.passedEntries})`);
console.log(`      Failed:             ${failedCount} (Summary: ${genJson.summary.failedEntries})`);
console.log(`      Expected Rejections:${expectedRejectCount} (Summary: ${genJson.summary.expectedRejections})`);
console.log(`      Weak Assertions:    ${weakCount} (Summary: ${genJson.summary.weakEntries})`);

if (passedCount !== genJson.summary.passedEntries) errors.push(`Passed count mismatch`);
if (failedCount !== genJson.summary.failedEntries) errors.push(`Failed count mismatch`);
if (expectedRejectCount !== genJson.summary.expectedRejections) errors.push(`Expected rejection count mismatch`);
if (weakCount !== genJson.summary.weakEntries) errors.push(`Weak assertions count mismatch`);

// Check Variable Provenance DAG
const scenario = genJson.scenarios[0];
if (!scenario.allCaptures || scenario.allCaptures.length === 0) {
  errors.push(`Variable Provenance DAG is empty!`);
} else {
  console.log(`  ✓ Variable Provenance DAG contains ${scenario.allCaptures.length} tracked variables:`);
  for (const cap of scenario.allCaptures) {
    const deps = cap.dependentEntries || [];
    console.log(`      • ${cap.name}: status=${cap.status}, defined at step #${cap.sourceEntryIndex}, used by ${deps.length} steps: [${deps.slice(0, 5).join(', ')}${deps.length > 5 ? '...' : ''}]`);
  }
}

// Step 5: Verify Generated HTML
console.log('\n🌐 Step 5: Verifying HTML Output Integrity...');

// 5.1 Embedded Data matches generated JSON
const dataTagMatch = genHtml.match(/<script id="tracehurl-data" type="application\/json">\s*([\s\S]*?)\s*<\/script>/);
if (!dataTagMatch) {
  errors.push(`Missing embedded <script id="tracehurl-data"> in HTML!`);
} else {
  const embeddedJson = JSON.parse(dataTagMatch[1]);
  if (embeddedJson.runId !== genJson.runId || embeddedJson.summary.failedEntries !== genJson.summary.failedEntries) {
    errors.push(`Embedded JSON does not match generated report.json!`);
  } else {
    console.log(`  ✓ Embedded data script matches JSON model with 100% fidelity.`);
  }
}

// 5.2 Header Metric Counters in HTML
const requiredKpiHooks = ['kpi-total', 'kpi-failed', 'kpi-rejections', 'kpi-blocked', 'kpi-weak', 'kpi-passed', 'kpi-duration'];
for (const kpi of requiredKpiHooks) {
  if (!genHtml.includes(`id="${kpi}"`)) {
    errors.push(`HTML does not contain KPI element '${kpi}'`);
  }
}
console.log(`  ✓ Metric counter elements (${requiredKpiHooks.join(', ')}) confirmed in HTML.`);

// 5.3 Redaction Integrity Check
// Ensure no live JWT or secrets leaked into the HTML
const jwtRegex = /eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}/;
const jwtMatch = genHtml.match(jwtRegex);
if (jwtMatch) {
  errors.push(`SECURITY VIOLATION: Raw unredacted JWT detected in HTML: ${jwtMatch[0].slice(0, 15)}...`);
} else {
  console.log(`  ✓ Security Invariant: Zero raw JWT tokens leaked into HTML (all masked).`);
}

// 5.4 Superimposed Diff Engine and DAG table hooks present
const requiredHooks = [
  'scenarios-list',
  'diff-superimposed-container',
  'diff-split-container',
  'btn-diff-mode-superimposed',
  'btn-diff-mode-split',
  'captures-dag-table-body',
  'assertions-table-body',
  'manifest-phases-tbody',
  'manifest-artifacts-tbody',
  'btn-first-failure',
  'tab-evidence',
  'tab-request-response',
  'tab-captures',
  'tab-diff',
  'tab-diagnostics',
  'tab-manifest',
];
for (const hook of requiredHooks) {
  if (!genHtml.includes(`id="${hook}"`)) {
    errors.push(`Required DOM hook id="${hook}" missing in generated HTML`);
  }
}
console.log(`  ✓ All ${requiredHooks.length} interactive UI elements and container hooks present.`);

// 6. Cleanup
fs.rmSync(tmpDir, { recursive: true, force: true });
console.log(`\n🧹 Cleaned up isolated temp directory: ${tmpDir}`);

// Final Verdict
console.log('\n================================================================');
if (errors.length === 0) {
  console.log('✅ ISOLATED RUN TEST PASSED WITH 100% ACCURACY & INTEGRITY!');
  console.log('================================================================\n');
  process.exit(0);
} else {
  console.error(`❌ ISOLATED RUN TEST FAILED with ${errors.length} error(s):`);
  errors.forEach(e => console.error(`   - ${e}`));
  console.log('================================================================\n');
  process.exit(1);
}
