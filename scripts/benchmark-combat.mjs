import { readFile, writeFile } from "node:fs/promises";
import process from "node:process";
import { createServer } from "vite";

const ONE_SECOND = 1e9;
const FIXTURE_URL = new URL("../fixtures/parity/zone-solo-basic/request.json", import.meta.url);

function parseArguments(argv) {
    const options = {
        iterations: 5,
        warmup: 1,
        simulationSeconds: 600,
        seed: "benchmark-zone-solo-basic",
        output: "",
    };

    for (let index = 0; index < argv.length; index++) {
        const argument = argv[index];
        const nextValue = argv[index + 1];
        if (argument === "--iterations" && nextValue) {
            options.iterations = Math.max(1, Math.trunc(Number(nextValue) || 1));
            index += 1;
        } else if (argument === "--warmup" && nextValue) {
            options.warmup = Math.max(0, Math.trunc(Number(nextValue) || 0));
            index += 1;
        } else if (argument === "--simulation-seconds" && nextValue) {
            options.simulationSeconds = Math.max(1, Number(nextValue) || 1);
            index += 1;
        } else if (argument === "--seed" && nextValue) {
            options.seed = nextValue;
            index += 1;
        } else if (argument === "--output" && nextValue) {
            options.output = nextValue;
            index += 1;
        } else if (argument === "--help") {
            options.help = true;
        } else {
            throw new Error(`Unknown or incomplete argument: ${argument}`);
        }
    }

    return options;
}

function percentile(sortedValues, ratio) {
    if (sortedValues.length === 0) return 0;
    const index = Math.min(sortedValues.length - 1, Math.ceil(sortedValues.length * ratio) - 1);
    return sortedValues[Math.max(0, index)];
}

async function loadModules() {
    const vite = await createServer({
        appType: "custom",
        logLevel: "error",
        server: { middlewareMode: true },
    });
    try {
        const [combatModule, playerModule, zoneModule, randomModule, contractModule] = await Promise.all([
            vite.ssrLoadModule("/src/combatsimulator/combatSimulator.js"),
            vite.ssrLoadModule("/src/combatsimulator/player.js"),
            vite.ssrLoadModule("/src/combatsimulator/zone.js"),
            vite.ssrLoadModule("/src/shared/randomSource.js"),
            vite.ssrLoadModule("/src/contracts/simulationContracts.js"),
        ]);
        return {
            vite,
            CombatSimulator: combatModule.default,
            Player: playerModule.default,
            Zone: zoneModule.default,
            createSeededRandomSource: randomModule.createSeededRandomSource,
            withPatchedMathRandom: randomModule.withPatchedMathRandom,
            normalizeSimulationRequestV1: contractModule.normalizeSimulationRequestV1,
        };
    } catch (error) {
        await vite.close();
        throw error;
    }
}

async function runScenario(modules, request, options, iteration) {
    const players = request.players.map((player) => modules.Player.createFromDTO(structuredClone(player)));
    const simulator = new modules.CombatSimulator(
        players,
        new modules.Zone(request.target.zoneHrid, request.target.difficultyTier),
        null,
        { enableHpMpVisualization: request.options.enableHpMpVisualization },
    );

    let processedEvents = 0;
    let peakEventQueueLength = 0;
    const eventQueue = simulator.eventQueue;
    const originalGetNextEvent = eventQueue.getNextEvent.bind(eventQueue);
    eventQueue.getNextEvent = () => {
        const queueLength = Number(eventQueue.minHeap?.length || 0);
        peakEventQueueLength = Math.max(peakEventQueueLength, queueLength);
        const event = originalGetNextEvent();
        if (event) processedEvents += 1;
        return event;
    };

    const randomSource = modules.createSeededRandomSource(options.seed);
    const startedAt = performance.now();
    const result = await modules.withPatchedMathRandom(
        randomSource,
        () => simulator.simulate(options.simulationSeconds * ONE_SECOND),
    );
    const elapsedMs = performance.now() - startedAt;
    const resultSummary = {
        simulatedTime: Number(result.simulatedTime || 0),
        encounters: Number(result.encounters || 0),
        dungeonsCompleted: Number(result.dungeonsCompleted || 0),
        dungeonsFailed: Number(result.dungeonsFailed || 0),
    };
    const workloadFingerprint = JSON.stringify({
        processedEvents,
        randomDraws: randomSource.drawCount,
        resultSummary,
    });

    return {
        iteration,
        elapsedMs,
        processedEvents,
        randomDraws: randomSource.drawCount,
        eventsPerSecond: elapsedMs > 0 ? processedEvents / (elapsedMs / 1000) : 0,
        peakEventQueueLength,
        resultSummary,
        workloadFingerprint,
    };
}

function printHelp() {
    console.log(`Usage: npm run benchmark:combat -- [options]\n\nOptions:\n  --iterations <n>          Measured iterations (default: 5)\n  --warmup <n>              Warm-up iterations (default: 1)\n  --simulation-seconds <n>  Simulated combat seconds (default: 600)\n  --seed <value>            Deterministic seed\n  --output <path>           Write the JSON report to a file\n  --help                    Show this help`);
}

async function main() {
    const options = parseArguments(process.argv.slice(2));
    if (options.help) {
        printHelp();
        return;
    }

    const [packageJson, rawFixture] = await Promise.all([
        readFile(new URL("../package.json", import.meta.url), "utf8").then(JSON.parse),
        readFile(FIXTURE_URL, "utf8").then(JSON.parse),
    ]);
    const modules = await loadModules();
    try {
        const request = modules.normalizeSimulationRequestV1({
            ...rawFixture,
            random: { type: "seeded", seed: options.seed },
            simulationTimeLimit: options.simulationSeconds * ONE_SECOND,
        });

        for (let index = 0; index < options.warmup; index++) {
            await runScenario(modules, request, options, `warmup-${index}`);
        }

        const runs = [];
        for (let index = 0; index < options.iterations; index++) {
            runs.push(await runScenario(modules, request, options, index));
        }

        const fingerprints = new Set(runs.map((run) => run.workloadFingerprint));
        if (fingerprints.size !== 1) {
            throw new Error("Benchmark iterations produced different deterministic workloads.");
        }

        const elapsedValues = runs.map((run) => run.elapsedMs).sort((left, right) => left - right);
        const eventRates = runs.map((run) => run.eventsPerSecond).sort((left, right) => left - right);
        const report = {
            benchmarkVersion: 1,
            generatedAt: new Date().toISOString(),
            environment: {
                node: process.version,
                platform: process.platform,
                architecture: process.arch,
            },
            scenario: {
                id: "zone-solo-basic",
                fixture: "fixtures/parity/zone-solo-basic/request.json",
                contractVersion: request.contractVersion,
                engine: "reference-js",
                engineVersion: packageJson.version,
                dataVersion: request.dataVersion,
                target: request.target,
                simulationSeconds: options.simulationSeconds,
                seed: options.seed,
                statisticsMode: request.options.statisticsMode,
                trace: false,
                hpMpVisualization: request.options.enableHpMpVisualization,
            },
            summary: {
                iterations: options.iterations,
                warmupIterations: options.warmup,
                minimumMs: elapsedValues[0] || 0,
                medianMs: percentile(elapsedValues, 0.5),
                p90Ms: percentile(elapsedValues, 0.9),
                medianEventsPerSecond: percentile(eventRates, 0.5),
                p90EventsPerSecond: percentile(eventRates, 0.9),
                workloadFingerprint: runs[0]?.workloadFingerprint || "",
            },
            runs,
        };

        const json = `${JSON.stringify(report, null, 2)}\n`;
        if (options.output) {
            await writeFile(options.output, json, "utf8");
        }
        process.stdout.write(json);
    } finally {
        await modules.vite.close();
    }
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
