export const SIZEOF_8 = 1;
export const SIZEOF_16 = 2;
export const SIZEOF_32 = 4;
export const SIZEOF_64 = 8;

export const IS_LITTLE_ENDIAN =
    new Uint16Array(new Uint8Array([1, 0]).buffer)[0] === 1;
