import BN from "bn.js";
import * as Layout from "../layout";

const BufferLayout = require("buffer-layout");

export const LastUpdateLayout: typeof BufferLayout.Structure =
  BufferLayout.struct(
    [Layout.uint64("slot"), BufferLayout.u8("stale")],
    "lastUpdate"
  );

export interface LastUpdate {
  slot: BN;
  stale: boolean;
}
