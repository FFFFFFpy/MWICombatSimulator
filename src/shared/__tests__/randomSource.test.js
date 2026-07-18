import { describe, expect, it } from "vitest";
import {
    createRandomSourceFromConfig,
    createSeededRandomSource,
    createSequenceRandomSource,
    createTracingRandomSource,
    withPatchedMathRandom,
} from "../randomSource.js";

describe("randomSource", () => {
    it("replays the same seeded sequence", () => {
        const first = createSeededRandomSource("fixture-a");
        const second = createSeededRandomSource("fixture-a");
        const firstValues = Array.from({ length: 16 }, () => first.next());
        const secondValues = Array.from({ length: 16 }, () => second.next());

        expect(firstValues).toEqual(secondValues);
        expect(firstValues.every((value) => value >= 0 && value < 1)).toBe(true);
    });

    it("does not collapse distinct seeds to the same opening sequence", () => {
        const first = createSeededRandomSource("fixture-a");
        const second = createSeededRandomSource("fixture-b");

        expect(Array.from({ length: 8 }, () => first.next()))
            .not.toEqual(Array.from({ length: 8 }, () => second.next()));
    });

    it("supports finite and looping sequence sources", () => {
        const finite = createSequenceRandomSource([0.1, 0.2]);
        expect(finite.next()).toBe(0.1);
        expect(finite.next()).toBe(0.2);
        expect(() => finite.next()).toThrow(/exhausted/i);

        const looping = createSequenceRandomSource([0.3, 0.4], { loop: true });
        expect([looping.next(), looping.next(), looping.next()]).toEqual([0.3, 0.4, 0.3]);
    });

    it("records random consumption order without changing values", () => {
        const draws = [];
        const source = createTracingRandomSource(
            createSequenceRandomSource([0.15, 0.75]),
            (entry) => draws.push(entry),
        );

        expect(source.next()).toBe(0.15);
        expect(source.next()).toBe(0.75);
        expect(draws).toEqual([
            { index: 0, value: 0.15 },
            { index: 1, value: 0.75 },
        ]);
    });

    it("builds configured sources and leaves native mode unpatched", () => {
        expect(createRandomSourceFromConfig()).toBeNull();
        expect(createRandomSourceFromConfig({ type: "native" })).toBeNull();
        expect(createRandomSourceFromConfig({ type: "seeded", seed: 7 }).next()).toBeTypeOf("number");
        expect(createRandomSourceFromConfig({ type: "sequence", values: [0.25] }).next()).toBe(0.25);
        expect(() => createRandomSourceFromConfig({ type: "unknown" })).toThrow(/unsupported/i);
    });

    it("patches Math.random only for the callback and restores it after success", async () => {
        const original = Math.random;
        const result = await withPatchedMathRandom(
            createSequenceRandomSource([0.11, 0.22]),
            async () => [Math.random(), await Promise.resolve(Math.random())],
        );

        expect(result).toEqual([0.11, 0.22]);
        expect(Math.random).toBe(original);
    });

    it("restores Math.random after callback failure", async () => {
        const original = Math.random;

        await expect(withPatchedMathRandom(
            createSequenceRandomSource([0.5]),
            async () => {
                expect(Math.random()).toBe(0.5);
                throw new Error("fixture failure");
            },
        )).rejects.toThrow("fixture failure");

        expect(Math.random).toBe(original);
    });
});
