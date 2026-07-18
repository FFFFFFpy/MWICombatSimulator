import { readFile, writeFile } from "node:fs/promises";
import process from "node:process";
import { createServer } from "vite";

const ONE_SECOND = 1e9;

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

function createPlayer(Player) {
    const player = new Player();
    player.hrid = "player1";
    player.staminaLevel = 20;
    player.intelligenceLevel = 20;
    player.attackLevel = 20;
    player.meleeLevel = 20;
    player.defenseLevel = 20;
    player.rangedLevel = 20;
    player.magicLevel = 20;
    return player;
}

async function loadModules() {
    const vite = await createServer({
        appType: "custom",
        logLevel: "error",
        server: { middlewareMode: true },
    });
    try {
        const [combatModule, playerModule, zoneModule, randomModule] = await Promise.all([
            vite.ssrLoadModule("/src/combatsimulator/combatSimulator.js"),
            vite.ssrLoadModule("/src/combatsimulator/player.js"),
            vite.ssrLoadModule("/src/combatsimulator/zone.js"),
            vite.ssrLoadModule("/src/shared/randomSource.js"),
        ]);
        return {
            vite,
            CombatSimulator: combatModule.default,
            Player: playerModule.default,
            Zone: zoneModule.default,
            createSeededRandomSource: randomModule.createSeededRandomSource,
            withPatchedMathRandom: randomModule.withPatchedMathRandom,
        };
    } catch (error) {
        await vite.close();
        throw error;
    }
}

async function runScenario(modules, options, iteration) {
    const simulator = new modules.CombatSimulator(
        [createPlayer(modules.Player)],
        new modules.Zone("/actions/combat/fly", 0),
        null,
        { enableHpMpVisualization: false },
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

    return {
        iteration,
        elapsedMs,
        processedEvents,
        randomDraws: randomSource.drawCount,
        eventsPerSecond: elapsedMs > 0 ? processedEvents / (elapsedMs / 1000) : 0,
        peakEventQueueLength,
        resultSummary,
        resultFingerprint: JSON.stringify(resultSummary),
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

    const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const modules = await loadModules();
    try {
        for (let index = 0; index < options.warmup; index++) {
            await runScenario(modules, options, `warmup-${index}`);
        }

        const runs = [];
        for (let index = 0; index < options.iterations; index++) {
            runs.push(await runScenario(modules, options, index));
        }

        const fingerprints = new Set(runs.map((run) => run.resultFingerprint));
        if (fingerprints.size !== 1) {
            throw new Error("Benchmark iterations produced different deterministic result summaries.");
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
                contractVersion: 1,
                engine: "reference-js",
                engineVersion: packageJson.version,
                dataVersion: "unversioned",
                target: { kind: "zone", zoneHrid: "/actions/combat/fly", difficultyTier: 0 },
                simulationSeconds: options.simulationSeconds,
                seed: options.seed,
                statisticsMode: "full",
                trace: false,
                hpMpVisualization: false,
            },
            summary: {
                iterations: options.iterations,
                warmupIterations: options.warmup,
                minimumMs: elapsedValues[0] || 0,
                medianMs: percentile(elapsedValues, 0.5),
                p90Ms: percentile(elapsedValues, 0.9),
                medianEventsPerSecond: percentile(eventRates, 0.5),
                p90EventsPerSecond: percentile(eventRates, 0.9),
                resultFingerprint: runs[0]?.resultFingerprint || "",
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
