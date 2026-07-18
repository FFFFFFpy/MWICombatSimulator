import CombatSimulator from "../combatsimulator/combatSimulator.js";
import Labyrinth from "../combatsimulator/labyrinth.js";
import Player from "../combatsimulator/player.js";
import Zone from "../combatsimulator/zone.js";
import { normalizeSimulationRequestV1 } from "../contracts/simulationContracts.js";
import {
    createMathRandomSource,
    createRandomSourceFromConfig,
    createTracingRandomSource,
    withPatchedMathRandom,
} from "../shared/randomSource.js";
import { buildSimulationExtraBuffs } from "../shared/simulationExtraBuffs.js";
import { attachCombatTrace } from "./combatTrace.js";

export const REFERENCE_ENGINE_ID = "reference-js";

export function createReferenceSimulation(requestInput) {
    const request = normalizeSimulationRequestV1(requestInput);
    let zone = null;
    let labyrinth = null;

    if (request.target.kind === "zone") {
        zone = new Zone(request.target.zoneHrid, request.target.difficultyTier);
    } else if (request.target.kind === "labyrinth") {
        labyrinth = new Labyrinth(
            request.target.labyrinthHrid,
            request.target.roomLevel,
            request.target.crates,
        );
    }

    const targetBuffs = zone?.buffs || labyrinth?.buffs || [];
    const extraBuffs = buildSimulationExtraBuffs(request.options.extra);
    const players = request.players.map((playerDto) => {
        const player = Player.createFromDTO(structuredClone(playerDto));
        player.zoneBuffs = targetBuffs;
        player.extraBuffs = extraBuffs;
        return player;
    });
    const simulator = new CombatSimulator(players, zone, labyrinth, {
        enableHpMpVisualization: request.options.enableHpMpVisualization,
    });

    return {
        request,
        simulator,
        players,
        zone,
        labyrinth,
    };
}

export async function runReferenceSimulation(requestInput, {
    onProgress = null,
    configureSimulator = null,
} = {}) {
    const runtime = createReferenceSimulation(requestInput);
    const { request, simulator } = runtime;
    const traceEnabled = request.options.trace.enabled === true;
    const traceController = traceEnabled
        ? attachCombatTrace(simulator, { maxEntries: request.options.trace.maxEntries })
        : null;

    let randomSource = createRandomSourceFromConfig(request.random);
    if (traceEnabled) {
        randomSource = createTracingRandomSource(
            randomSource || createMathRandomSource(),
            (draw) => traceController.recordRandomDraw(draw),
        );
    }

    const progressListener = typeof onProgress === "function"
        ? (event) => onProgress(event.detail, runtime)
        : null;
    if (progressListener) {
        simulator.addEventListener("progress", progressListener);
    }

    try {
        configureSimulator?.(simulator, runtime);
        const result = await withPatchedMathRandom(
            randomSource,
            () => simulator.simulate(request.simulationTimeLimit),
        );
        return {
            ...runtime,
            result,
            trace: traceController?.getTrace() || null,
            randomDraws: randomSource?.drawCount || 0,
        };
    } catch (error) {
        if (traceController) {
            error.referenceSimulationTrace = traceController.getTrace();
        }
        throw error;
    } finally {
        if (progressListener) {
            simulator.removeEventListener("progress", progressListener);
        }
        traceController?.detach();
    }
}
