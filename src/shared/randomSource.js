function normalizeUnitRandom(value) {
    const normalized = Number(value);
    if (!Number.isFinite(normalized) || normalized < 0 || normalized >= 1) {
        throw new RangeError(`Random values must be finite numbers in [0, 1); received ${String(value)}`);
    }
    return normalized;
}

function hashSeed(seed) {
    const text = typeof seed === "string" ? seed : JSON.stringify(seed ?? 0);
    let hash = 2166136261;
    for (let index = 0; index < text.length; index++) {
        hash ^= text.charCodeAt(index);
        hash = Math.imul(hash, 16777619);
    }
    return (hash >>> 0) || 0x6d2b79f5;
}

export function createMathRandomSource() {
    const nativeRandom = Math.random.bind(Math);
    let drawCount = 0;
    return {
        kind: "native",
        next() {
            drawCount += 1;
            return nativeRandom();
        },
        get drawCount() {
            return drawCount;
        },
    };
}

export function createSeededRandomSource(seed = 0) {
    let state = hashSeed(seed);
    let drawCount = 0;
    return {
        kind: "seeded",
        seed,
        next() {
            drawCount += 1;
            state = (state + 0x6d2b79f5) >>> 0;
            let value = state;
            value = Math.imul(value ^ (value >>> 15), value | 1);
            value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
            return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
        },
        get drawCount() {
            return drawCount;
        },
    };
}

export function createSequenceRandomSource(values, { loop = false } = {}) {
    const sequence = Array.from(values || [], normalizeUnitRandom);
    if (sequence.length === 0) {
        throw new Error("A sequence random source requires at least one value.");
    }

    let sequenceIndex = 0;
    let drawCount = 0;
    return {
        kind: "sequence",
        next() {
            if (sequenceIndex >= sequence.length) {
                if (!loop) {
                    throw new Error(`Random sequence exhausted after ${sequence.length} draws.`);
                }
                sequenceIndex = 0;
            }
            drawCount += 1;
            return sequence[sequenceIndex++];
        },
        get drawCount() {
            return drawCount;
        },
    };
}

export function createTracingRandomSource(source, onDraw) {
    if (!source || typeof source.next !== "function") {
        throw new TypeError("A tracing random source requires a source with next().");
    }
    let drawIndex = 0;
    return {
        kind: `tracing:${source.kind || "custom"}`,
        next() {
            const value = normalizeUnitRandom(source.next());
            const entry = { index: drawIndex++, value };
            onDraw?.(entry);
            return value;
        },
        get drawCount() {
            return drawIndex;
        },
    };
}

export function createRandomSourceFromConfig(config) {
    if (!config || config.type === "native") {
        return null;
    }
    if (config.type === "seeded") {
        return createSeededRandomSource(config.seed);
    }
    if (config.type === "sequence") {
        return createSequenceRandomSource(config.values, { loop: config.loop === true });
    }
    throw new Error(`Unsupported random source type: ${String(config.type)}`);
}

export async function withPatchedMathRandom(source, callback) {
    if (typeof callback !== "function") {
        throw new TypeError("withPatchedMathRandom requires a callback.");
    }
    if (!source) {
        return callback();
    }
    if (typeof source.next !== "function") {
        throw new TypeError("Random source must expose next().");
    }

    const previousRandom = Math.random;
    Math.random = () => normalizeUnitRandom(source.next());
    try {
        return await callback();
    } finally {
        Math.random = previousRandom;
    }
}
