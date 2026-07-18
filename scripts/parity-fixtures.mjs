import { createHash } from "node:crypto";
import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { gunzipSync, gzipSync } from "node:zlib";
import { createServer } from "vite";

const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FIXTURES_ROOT = path.join(REPOSITORY_ROOT, "fixtures", "parity");
const TRACE_ARCHIVE_NAME = "expected-trace.json.gz.b64";

function parseArguments(argv) {
    const options = {
        mode: "check",
        fixture: "",
        outputDir: "",
    };
    for (let index = 0; index < argv.length; index++) {
        const argument = argv[index];
        const nextValue = argv[index + 1];
        if (argument === "--mode" && nextValue) {
            options.mode = nextValue;
            index += 1;
        } else if (argument === "--fixture" && nextValue) {
            options.fixture = nextValue;
            index += 1;
        } else if (argument === "--output-dir" && nextValue) {
            options.outputDir = path.resolve(REPOSITORY_ROOT, nextValue);
            index += 1;
        } else if (argument === "--help") {
            options.help = true;
        } else {
            throw new Error(`Unknown or incomplete argument: ${argument}`);
        }
    }
    if (!new Set(["generate", "check"]).has(options.mode)) {
        throw new Error(`Unsupported mode: ${options.mode}`);
    }
    return options;
}

function printHelp() {
    console.log(`Usage: node scripts/parity-fixtures.mjs [options]\n\nOptions:\n  --mode <generate|check>  Generate or verify golden files (default: check)\n  --fixture <id>           Process one fixture directory\n  --output-dir <path>      Write generated goldens outside fixtures/parity\n  --help                   Show this help`);
}

function canonicalize(value) {
    if (Array.isArray(value)) {
        return value.map(canonicalize);
    }
    if (value && typeof value === "object") {
        return Object.fromEntries(
            Object.keys(value)
                .sort()
                .map((key) => [key, canonicalize(value[key])]),
        );
    }
    return value;
}

function normalizeGoldenResult(result) {
    const normalized = JSON.parse(JSON.stringify(result));
    for (const wipeEvent of normalized.wipeEvents || []) {
        if (Object.prototype.hasOwnProperty.call(wipeEvent, "timestamp")) {
            wipeEvent.timestamp = "<wall-clock-omitted>";
        }
    }
    return canonicalize(normalized);
}

function normalizeGoldenTrace(trace) {
    return canonicalize(JSON.parse(JSON.stringify(trace)));
}

function formatJson(value) {
    return `${JSON.stringify(value, null, 2)}\n`;
}

function encodeTraceArchive(traceText) {
    return `${gzipSync(Buffer.from(traceText, "utf8"), { level: 9 }).toString("base64")}\n`;
}

function decodeTraceArchive(encodedText) {
    const compressed = Buffer.from(String(encodedText || "").trim(), "base64");
    return JSON.parse(gunzipSync(compressed).toString("utf8"));
}

function sha256(value) {
    return createHash("sha256").update(value).digest("hex");
}

function firstDifference(first, second, location = "root") {
    if (Object.is(first, second)) return null;
    if (typeof first !== typeof second || first == null || second == null) {
        return { location, first, second };
    }
    if (typeof first !== "object") {
        return { location, first, second };
    }
    if (Array.isArray(first) !== Array.isArray(second)) {
        return { location, firstType: Array.isArray(first) ? "array" : "object", secondType: Array.isArray(second) ? "array" : "object" };
    }
    if (Array.isArray(first)) {
        if (first.length !== second.length) {
            return { location: `${location}.length`, first: first.length, second: second.length };
        }
        for (let index = 0; index < first.length; index++) {
            const difference = firstDifference(first[index], second[index], `${location}[${index}]`);
            if (difference) return difference;
        }
        return null;
    }
    const firstKeys = Object.keys(first).sort();
    const secondKeys = Object.keys(second).sort();
    if (JSON.stringify(firstKeys) !== JSON.stringify(secondKeys)) {
        return { location: `${location}.__keys`, first: firstKeys, second: secondKeys };
    }
    for (const key of firstKeys) {
        const difference = firstDifference(first[key], second[key], `${location}.${key}`);
        if (difference) return difference;
    }
    return null;
}

function getPathValue(root, fieldPath) {
    const parts = String(fieldPath || "").split(".").filter(Boolean);
    let value = root;
    for (const part of parts) {
        if (part === "length") {
            value = value?.length;
        } else {
            value = value?.[part];
        }
    }
    return value;
}

function validateFixtureAssertions(fixtureId, metadata, execution) {
    const assertions = metadata?.assertions || {};

    for (const [fieldPath, expected] of Object.entries(assertions.resultEquals || {})) {
        const actual = getPathValue(execution.result, fieldPath);
        if (!Object.is(actual, expected)) {
            throw new Error(
                `Fixture ${fixtureId} assertion failed: result.${fieldPath} expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}.`,
            );
        }
    }

    for (const [fieldPath, minimum] of Object.entries(assertions.resultMinimums || {})) {
        const actual = Number(getPathValue(execution.result, fieldPath));
        if (!Number.isFinite(actual) || actual < Number(minimum)) {
            throw new Error(
                `Fixture ${fixtureId} assertion failed: result.${fieldPath} expected >= ${minimum}, received ${String(actual)}.`,
            );
        }
    }

    if (assertions.minimumRandomDraws != null && execution.randomDraws < Number(assertions.minimumRandomDraws)) {
        throw new Error(
            `Fixture ${fixtureId} assertion failed: expected at least ${assertions.minimumRandomDraws} random draws, received ${execution.randomDraws}.`,
        );
    }

    if (assertions.minimumTraceEvents != null && execution.trace.events.length < Number(assertions.minimumTraceEvents)) {
        throw new Error(
            `Fixture ${fixtureId} assertion failed: expected at least ${assertions.minimumTraceEvents} trace events, received ${execution.trace.events.length}.`,
        );
    }

    const traceEventTypes = new Set(execution.trace.events.map((entry) => entry?.event?.type).filter(Boolean));
    for (const eventType of assertions.requiredTraceEventTypes || []) {
        if (!traceEventTypes.has(eventType)) {
            throw new Error(
                `Fixture ${fixtureId} assertion failed: required trace event type ${eventType} was not observed.`,
            );
        }
    }
}

async function discoverFixtures(requestedFixture) {
    if (requestedFixture) return [requestedFixture];
    const entries = await readdir(FIXTURES_ROOT, { withFileTypes: true });
    return entries.filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort();
}

async function loadModules() {
    const vite = await createServer({
        appType: "custom",
        logLevel: "error",
        server: { middlewareMode: true },
        ssr: { noExternal: ["heap-js"] },
    });
    try {
        const runnerModule = await vite.ssrLoadModule("/src/services/referenceSimulationRunner.js");
        return {
            vite,
            runReferenceSimulation: runnerModule.runReferenceSimulation,
        };
    } catch (error) {
        await vite.close();
        throw error;
    }
}

async function executeFixture(modules, fixtureId) {
    const fixtureDir = path.join(FIXTURES_ROOT, fixtureId);
    const [request, metadata] = await Promise.all([
        readFile(path.join(fixtureDir, "request.json"), "utf8").then(JSON.parse),
        readFile(path.join(fixtureDir, "metadata.json"), "utf8").then(JSON.parse).catch(() => ({})),
    ]);
    const execution = await modules.runReferenceSimulation({
        ...request,
        options: {
            ...request.options,
            trace: {
                ...request.options?.trace,
                enabled: true,
                maxEntries: Math.max(1, Number(request.options?.trace?.maxEntries || 100_000)),
            },
        },
    });
    if (!execution.trace) {
        throw new Error(`Fixture ${fixtureId} did not produce a trace.`);
    }
    if (execution.trace.truncatedEntries > 0) {
        throw new Error(`Fixture ${fixtureId} trace was truncated by ${execution.trace.truncatedEntries} entries.`);
    }
    const normalizedExecution = {
        fixtureDir,
        request,
        metadata,
        result: normalizeGoldenResult(execution.result),
        trace: normalizeGoldenTrace(execution.trace),
        randomDraws: execution.randomDraws,
    };
    validateFixtureAssertions(fixtureId, metadata, normalizedExecution);
    return normalizedExecution;
}

async function generateFixture(modules, fixtureId, options, packageJson) {
    const execution = await executeFixture(modules, fixtureId);
    const destination = options.outputDir
        ? path.join(options.outputDir, fixtureId)
        : execution.fixtureDir;
    await mkdir(destination, { recursive: true });

    const resultText = formatJson(execution.result);
    const traceText = formatJson(execution.trace);
    const traceArchiveText = encodeTraceArchive(traceText);
    const metadata = canonicalize({
        ...execution.metadata,
        fixtureVersion: Number(execution.metadata.fixtureVersion || 1),
        scenarioId: fixtureId,
        contractVersion: Number(execution.request.contractVersion || 1),
        referenceEngine: "reference-js",
        referenceEngineVersion: packageJson.version,
        dataVersion: execution.request.dataVersion,
        status: "golden",
        randomDraws: execution.randomDraws,
        expectedFiles: ["request.json", "expected-result.json", TRACE_ARCHIVE_NAME],
        traceEncoding: "gzip+base64",
        hashes: {
            expectedResultSha256: sha256(resultText),
            expectedTraceSha256: sha256(traceText),
            expectedTraceArchiveSha256: sha256(traceArchiveText),
        },
    });

    await Promise.all([
        writeFile(path.join(destination, "expected-result.json"), resultText, "utf8"),
        writeFile(path.join(destination, TRACE_ARCHIVE_NAME), traceArchiveText, "utf8"),
        writeFile(path.join(destination, "metadata.json"), formatJson(metadata), "utf8"),
    ]);
    console.log(`Generated parity fixture: ${fixtureId}`);
}

async function checkFixture(modules, fixtureId) {
    const execution = await executeFixture(modules, fixtureId);
    const [expectedResult, expectedTrace] = await Promise.all([
        readFile(path.join(execution.fixtureDir, "expected-result.json"), "utf8").then(JSON.parse),
        readFile(path.join(execution.fixtureDir, TRACE_ARCHIVE_NAME), "utf8").then(decodeTraceArchive),
    ]);
    const resultDifference = firstDifference(canonicalize(expectedResult), execution.result, "result");
    if (resultDifference) {
        throw new Error(`Parity result mismatch for ${fixtureId}: ${JSON.stringify(resultDifference)}`);
    }
    const traceDifference = firstDifference(canonicalize(expectedTrace), execution.trace, "trace");
    if (traceDifference) {
        throw new Error(`Parity trace mismatch for ${fixtureId}: ${JSON.stringify(traceDifference)}`);
    }
    if (Number(execution.metadata.randomDraws) !== execution.randomDraws) {
        throw new Error(
            `Parity random draw mismatch for ${fixtureId}: expected ${execution.metadata.randomDraws}, received ${execution.randomDraws}.`,
        );
    }
    console.log(`Verified parity fixture: ${fixtureId}`);
}

async function main() {
    const options = parseArguments(process.argv.slice(2));
    if (options.help) {
        printHelp();
        return;
    }
    const packageJson = JSON.parse(await readFile(path.join(REPOSITORY_ROOT, "package.json"), "utf8"));
    const fixtures = await discoverFixtures(options.fixture);
    if (fixtures.length === 0) {
        throw new Error("No parity fixtures were found.");
    }

    const modules = await loadModules();
    try {
        for (const fixtureId of fixtures) {
            if (options.mode === "generate") {
                await generateFixture(modules, fixtureId, options, packageJson);
            } else {
                await checkFixture(modules, fixtureId);
            }
        }
    } finally {
        await modules.vite.close();
    }
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
