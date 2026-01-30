import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

const BufferLayout = require("buffer-layout");

/**
 * Layout for a public key
 */
export const publicKey = (property = "publicKey"): unknown => {
  const publicKeyLayout = BufferLayout.blob(32, property);

  const _decode = publicKeyLayout.decode.bind(publicKeyLayout);
  const _encode = publicKeyLayout.encode.bind(publicKeyLayout);

  publicKeyLayout.decode = (buffer: Buffer, offset: number) => {
    const data = _decode(buffer, offset);
    return new PublicKey(data);
  };

  publicKeyLayout.encode = (key: PublicKey, buffer: Buffer, offset: number) =>
    _encode(key.toBuffer(), buffer, offset);

  return publicKeyLayout;
};

/**
 * Layout for a 64bit unsigned value
 */
export const uint64 = (property = "uint64"): unknown => {
  const layout = BufferLayout.blob(8, property);

  const _decode = layout.decode.bind(layout);
  const _encode = layout.encode.bind(layout);

  layout.decode = (buffer: Buffer, offset: number) => {
    const data = _decode(buffer, offset);
    return new BN(
      [...data]
        .reverse()
        .map((i) => `00${i.toString(16)}`.slice(-2))
        .join(""),
      16
    );
  };

  layout.encode = (num: BN, buffer: Buffer, offset: number) => {
    const a = num.toArray().reverse();
    let b = Buffer.from(a);
    if (b.length !== 8) {
      const zeroPad = Buffer.alloc(8);
      b.copy(zeroPad);
      b = zeroPad;
    }
    return _encode(b, buffer, offset);
  };

  return layout;
};

/**
 * Layout for a 64bit signed value
 */
export const int64 = (property = "int64"): unknown => {
  const layout = BufferLayout.blob(8, property);

  const _decode = layout.decode.bind(layout);
  const _encode = layout.encode.bind(layout);

  layout.decode = (buffer: Buffer, offset: number) => {
    const data = _decode(buffer, offset);
    // Check if the integer is negative
    const isNegative = data[7] & 0x80;
    if (isNegative) {
      // Create a Buffer filled with 0xff for negative number twos complement
      const invertedData = Buffer.from(data.map((byte: any) => byte ^ 0xff));
      const negated = new BN(
        Array.from(invertedData)
          .reverse()
          .map((i) => `00${i.toString(16)}`.slice(-2))
          .join(""),
        16
      ).addn(1);
      return negated.neg();
    } else {
      return new BN(
        [...data]
          .reverse()
          .map((i) => `00${i.toString(16)}`.slice(-2))
          .join(""),
        16
      );
    }
  };

  layout.encode = (num: BN, buffer: Buffer, offset: number) => {
    if (num.isNeg()) {
      const absNum = num.abs();
      let a = absNum.subn(1).toArray().reverse();
      a = a.map((byte) => byte ^ 0xff); // Invert the bytes for two's complement
      let b = Buffer.from(a);
      if (b.length !== 8) {
        const zeroPad = Buffer.alloc(8, 0xff); // Use 0xff to fill the negative number padding
        b.copy(zeroPad);
        b = zeroPad;
      }
      return _encode(b, buffer, offset);
    } else {
      const a = num.toArray().reverse();
      let b = Buffer.from(a);
      if (b.length !== 8) {
        const zeroPad = Buffer.alloc(8);
        b.copy(zeroPad);
        b = zeroPad;
      }
      return _encode(b, buffer, offset);
    }
  };

  return layout;
};

export const uint128 = (property = "uint128"): unknown => {
  const layout = BufferLayout.blob(16, property);

  const _decode = layout.decode.bind(layout);
  const _encode = layout.encode.bind(layout);

  layout.decode = (buffer: Buffer, offset: number) => {
    const data = _decode(buffer, offset);
    return new BN(
      [...data]
        .reverse()
        .map((i) => `00${i.toString(16)}`.slice(-2))
        .join(""),
      16
    );
  };

  layout.encode = (num: BN, buffer: Buffer, offset: number) => {
    const a = num.toArray().reverse();
    let b = Buffer.from(a);
    if (b.length !== 16) {
      const zeroPad = Buffer.alloc(16);
      b.copy(zeroPad);
      b = zeroPad;
    }

    return _encode(b, buffer, offset);
  };

  return layout;
};

/**
 * Layout for a Rust String type
 */
export const rustString = (property = "string"): unknown => {
  const rsl = BufferLayout.struct(
    [
      BufferLayout.u32("length"),
      BufferLayout.u32("lengthPadding"),
      BufferLayout.blob(BufferLayout.offset(BufferLayout.u32(), -8), "chars"),
    ],
    property
  );
  const _decode = rsl.decode.bind(rsl);
  const _encode = rsl.encode.bind(rsl);

  rsl.decode = (buffer: Buffer, offset: number) => {
    const data = _decode(buffer, offset);
    return data.chars.toString("utf8");
  };

  rsl.encode = (str: string, buffer: Buffer, offset: number) => {
    const data = {
      chars: Buffer.from(str, "utf8"),
    };
    return _encode(data, buffer, offset);
  };

  return rsl;
};
