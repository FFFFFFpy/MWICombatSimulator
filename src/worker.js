import { legacyWorkerMessageToSimulationRequestV1 } from "./contracts/simulationContracts.js";
import { runReferenceSimulation } from "./services/referenceSimulationRunner.js";

onmessage = async function (event) {
    switch (event.data.type) {
        case "start_simulation": {
            try {
                const request = legacyWorkerMessageToSimulationRequestV1(event.data);
                const execution = await runReferenceSimulation(request, {
                    onProgress: (progress) => {
                        this.postMessage({
                            type: "simulation_progress",
                            progress: progress.progress,
                            zone: progress.zone,
                            difficultyTier: progress.difficultyTier,
                            labyrinth: progress.labyrinth,
                            roomLevel: progress.roomLevel,
                            timeSeriesData: progress.timeSeriesData,
                        });
                    },
                });
                const response = {
                    type: "simulation_result",
                    simResult: execution.result,
                };
                if (execution.trace) {
                    response.trace = execution.trace;
                }
                this.postMessage(response);
            } catch (error) {
                console.log(error);
                const response = { type: "simulation_error", error };
                if (error?.referenceSimulationTrace) {
                    response.trace = error.referenceSimulationTrace;
                }
                this.postMessage(response);
            }
            break;
        }
    }
};
