import CombatSimulator from "./combatsimulator/combatSimulator";
import Player from "./combatsimulator/player";
import Zone from "./combatsimulator/zone";
import Labyrinth from "./combatsimulator/labyrinth";
import { buildSimulationExtraBuffs } from "./shared/simulationExtraBuffs.js";
import {
    createMathRandomSource,
    createRandomSourceFromConfig,
    createTracingRandomSource,
    withPatchedMathRandom,
} from "./shared/randomSource.js";
import { attachCombatTrace } from "./services/combatTrace.js";

onmessage = async function (event) {
    switch (event.data.type) {
        case "start_simulation": {
            let traceController = null;
            try {
                const extra = event.data.extra || {};
                const extraBuffs = buildSimulationExtraBuffs(extra);
                const playersData = Array.isArray(event.data.players) ? event.data.players : [];
                const players = [];

                let zone = null;
                if (event.data.zone) {
                    zone = new Zone(event.data.zone.zoneHrid, event.data.zone.difficultyTier);
                }

                let labyrinth = null;
                if (event.data.labyrinth) {
                    labyrinth = new Labyrinth(
                        event.data.labyrinth.labyrinthHrid,
                        event.data.labyrinth.roomLevel,
                        event.data.labyrinth.crates,
                    );
                }

                for (let index = 0; index < playersData.length; index++) {
                    const currentPlayer = Player.createFromDTO(structuredClone(playersData[index]));
                    currentPlayer.zoneBuffs = zone?.buffs || labyrinth?.buffs || [];
                    currentPlayer.extraBuffs = extraBuffs;
                    players.push(currentPlayer);
                }

                const simulationTimeLimit = event.data.simulationTimeLimit;
                const enableHpMpVisualization = extra.enableHpMpVisualization || false;
                const combatSimulator = new CombatSimulator(players, zone, labyrinth, { enableHpMpVisualization });
                const traceEnabled = event.data.trace?.enabled === true;

                if (traceEnabled) {
                    traceController = attachCombatTrace(combatSimulator, {
                        maxEntries: event.data.trace?.maxEntries,
                    });
                }

                let randomSource = createRandomSourceFromConfig(event.data.random);
                if (traceEnabled) {
                    randomSource = createTracingRandomSource(
                        randomSource || createMathRandomSource(),
                        (draw) => traceController.recordRandomDraw(draw),
                    );
                }

                combatSimulator.addEventListener("progress", (progressEvent) => {
                    this.postMessage({
                        type: "simulation_progress",
                        progress: progressEvent.detail.progress,
                        zone: progressEvent.detail.zone,
                        difficultyTier: progressEvent.detail.difficultyTier,
                        labyrinth: progressEvent.detail.labyrinth,
                        roomLevel: progressEvent.detail.roomLevel,
                        timeSeriesData: progressEvent.detail.timeSeriesData,
                    });
                });

                const simResult = await withPatchedMathRandom(
                    randomSource,
                    () => combatSimulator.simulate(simulationTimeLimit),
                );
                const response = { type: "simulation_result", simResult };
                if (traceController) {
                    response.trace = traceController.getTrace();
                }
                this.postMessage(response);
            } catch (error) {
                console.log(error);
                const response = { type: "simulation_error", error };
                if (traceController) {
                    response.trace = traceController.getTrace();
                }
                this.postMessage(response);
            } finally {
                traceController?.detach();
            }
            break;
        }
    }
};
