import { expect } from 'vitest';
import { DecodeError } from '@buffy/core';

/**
 * Expects reading bytes to return values or throw `DecodeError`, rather than
 * any other error
 */
export function expectValuesOrDecodeError(read: () => unknown): void {
    try {
        read();
    } catch (error) {
        expect(error).toBeInstanceOf(DecodeError);
    }
}
