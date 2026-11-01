export const UINT_8_MAX = 2 ** 8 - 1;
export const UINT_16_MAX = 2 ** 16 - 1;
export const UINT_32_MAX = 2 ** 32 - 1;
export const UINT_64_MAX = 2n ** 64n - 1n;

export const INT_8_MAX = Math.floor(UINT_8_MAX / 2);
export const INT_16_MAX = Math.floor(UINT_16_MAX / 2);
export const INT_32_MAX = Math.floor(UINT_32_MAX / 2);
export const INT_64_MAX = UINT_64_MAX / 2n;
