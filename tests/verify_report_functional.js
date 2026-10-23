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
  'failure-nav-counter',
  'kpi-clusters-container',
  'kpi-clusters',
  'cluster-filter-container',
  'cluster-select',
  'btn-tab-clusters',
  'tab-clusters',
  'entry-cluster-banner',
  'cluster-cards-container',
  'btn-export-har',
  'footer-export-har'
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

// 5. Verify Failure Clustering & Root-Cause Grouping
console.log('\n✓ 5. Verifying Failure Clustering & Root-Cause Grouping:');
assert(Array.isArray(runData.failureClusters), 'runData.failureClusters must be an array');
assert(runData.failureClusters.length > 0, 'At least 1 failure cluster must be identified');
const cluster1 = runData.failureClusters[0];
assert(cluster1.id, 'Cluster must have an id');
assert(cluster1.title, 'Cluster must have a title');
assert(cluster1.rootCauseCategory, 'Cluster must have a rootCauseCategory');
assert(cluster1.entryCount >= 1, 'Cluster entryCount must be >= 1');
assert(cluster1.entryIds.includes(entry2.id), 'Cluster entryIds must contain entry #2');
assert.strictEqual(entry2.clusterId, cluster1.id, 'Entry #2 clusterId must link to cluster1');
console.log(`     - Identified ${runData.failureClusters.length} failure cluster(s):`);
console.log(`       * [${cluster1.id}] ${cluster1.title} (${cluster1.entryCount} entries)`);
console.log(`       * Root Cause Category: ${cluster1.rootCauseCategory}`);
console.log(`       * Remediation: ${cluster1.suggestedRemediation || 'None'}`);

// 6. Verify HAR (HTTP Archive 1.2) Export
console.log('\n✓ 6. Verifying HAR (HTTP Archive 1.2) Export:');
const harPath = reportPath.replace('index.html', 'trace.har');
assert(fs.existsSync(harPath), 'trace.har must be generated alongside index.html');
const harJson = JSON.parse(fs.readFileSync(harPath, 'utf8'));
assert.strictEqual(harJson.log.version, '1.2', 'HAR version must be 1.2');
assert.strictEqual(harJson.log.creator.name, 'TraceHurl', 'Creator name must be TraceHurl');
assert.strictEqual(harJson.log.entries.length, runData.summary.totalEntries, 'HAR entries count must match total entries');
console.log(`     - Generated HAR file verified: ${harPath}`);
console.log(`     - HAR Spec: Version 1.2, Creator: ${harJson.log.creator.name} v${harJson.log.creator.version}`);
console.log(`     - Total Archive HTTP Transactions: ${harJson.log.entries.length}`);

console.log('\n======================================================');
console.log('✓ ALL 6 VERIFICATION STAGES PASSED FUNCTIONALLY');
console.log('======================================================\n');
