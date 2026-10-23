const fs = require('fs');
const assert = require('assert');

console.log('=== TraceHurl Report Functional Verification ===\n');

const reportPath = fs.existsSync('.verify/fixtures/sample_run/report/index.html')
  ? '.verify/fixtures/sample_run/report/index.html'
  : (fs.existsSync('.verify/artifacts/VERIFY1792726500/report/index.html')
    ? '.verify/artifacts/VERIFY1792726500/report/index.html'
    : 'verify/artifacts/VERIFY1792726500/report/index.html');
const htmlContent = fs.readFileSync(reportPath, 'utf8');

// 1. Verify JSON Data embedding
const dataMatch = htmlContent.match(/<script id="tracehurl-data" type="application\/json">([\s\S]*?)<\/script>/);
assert(dataMatch, 'tracehurl-data script must exist in HTML');
const runData = JSON.parse(dataMatch[1]);
console.log('✓ 1. Embedded JSON Model Validated:');
console.log(`     - Run ID: ${runData.runId}`);
console.log(`     - Scenarios: ${runData.scenarios.length}`);
console.log(`     - Total Entries: ${runData.summary.totalEntries}`);
console.log(`     - Failed Entries: ${runData.summary.failedEntries}`);
console.log(`     - Scenario Captures in DAG: ${runData.scenarios[0].allCaptures.length}`);

// 2. Verify Tab Structure and Responsive Elements
const requiredElementIds = [
  'scenario-sidebar',
  'scenarios-list',
  'tab-evidence',
  'tab-request-response',
  'tab-captures',
  'tab-diff',
  'tab-diagnostics',
  'tab-manifest',
  'tab-response-body',
  'tab-response-provenance-badge',
  'diff-superimposed-container',
  'diff-superimposed-code',
  'diff-split-container',
  'captures-dag-table-body',
  'captures-used-container',
  'diagnostics-list-container',
  'btn-first-failure',
  'footer-first-failure',
  'failure-nav-counter'
];

console.log('\n✓ 2. Verifying DOM Elements & ID Hooks:');
for (const id of requiredElementIds) {
  assert(htmlContent.includes(`id="${id}"`), `Missing required element ID: ${id}`);
}
console.log(`     - All ${requiredElementIds.length} required interactive DOM hooks confirmed present.`);

// 3. Verify CSS Vertical Scrolling Constraints
console.log('\n✓ 3. Verifying Vertical Scrolling & Layout Constraints:');
assert(htmlContent.includes('h-screen overflow-hidden flex flex-col'), 'body must enforce h-screen overflow-hidden');
assert(htmlContent.includes('id="scenarios-list" class="flex-1 min-h-0 overflow-y-auto'), 'scenarios-list must have min-h-0 and overflow-y-auto');
assert(htmlContent.includes('id="diagnostics-list-container" class="p-3 space-y-2 overflow-y-auto'), 'diagnostics-list-container must have overflow-y-auto');
console.log('     - Body viewport height constrained (h-screen overflow-hidden).');
console.log('     - Scenario Explorer list has flex-1 min-h-0 overflow-y-auto for independent vertical scrolling.');
console.log('     - Diagnostics list has overflow-y-auto for independent scrolling.');

// 4. Functional Execution Simulation
console.log('\n✓ 4. Simulating Interactive State Logic:');

const entries = runData.scenarios[0].entries;
const failureIds = entries.filter(e => e.outcome === 'failed').map(e => e.id);
assert.strictEqual(failureIds.length, runData.summary.failedEntries, 'Failure IDs count must match summary');
console.log(`     - Identified ${failureIds.length} failure steps for navigation stepper.`);

// Test Entry 2 (First failure)
const entry2 = entries.find(e => e.index === 2);
assert(entry2, 'Entry #2 must exist');
assert.strictEqual(entry2.outcome, 'failed', 'Entry #2 must be marked failed');
assert(entry2.response, 'Entry #2 must have response data');
assert(entry2.response.bodySnippet, 'Entry #2 must have non-empty response bodySnippet');
console.log(`     - Entry #2 response body verified: ${entry2.response.bodySnippet.length} bytes (isDerived: ${entry2.response.isDerived})`);

// Verify Diff Inspector has diffs for Entry 2
assert(entry2.diffs.length > 0, 'Entry #2 must contain contract diffs');
console.log(`     - Entry #2 diffs verified: ${entry2.diffs.length} contractual modifications found.`);
const diffAuthCode = entry2.diffs.find(d => d.path.includes('authCode'));
assert(diffAuthCode, 'Missing authCode diff must be present in Entry #2');
console.log(`       * Verified missing contract field: ${diffAuthCode.path} (type: ${diffAuthCode.diffType})`);

// Verify Scenario DAG Captures
const allCaps = runData.scenarios[0].allCaptures;
assert(allCaps.length >= 1, 'Scenario must have captured variables in DAG');
const targetCap = allCaps.find(c => c.name === 'user_id' || c.name === 'account_number');
assert(targetCap, 'Captured variable must be tracked in DAG');
console.log(`     - Variable Provenance DAG verified: '${targetCap.name}' extracted in step #${targetCap.sourceEntryIndex}`);

// Verify First Failure Stepper
const firstFailId = failureIds[0];
assert.strictEqual(firstFailId, entry2.id, 'First failure ID must correspond to Entry #2');
console.log(`     - Jump to First Failure target: ${firstFailId}`);

console.log('\n======================================================');
console.log('✓ ALL 4 VERIFICATION STAGES PASSED FUNCTIONALLY');
console.log('======================================================\n');
