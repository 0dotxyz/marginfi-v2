import { Program } from "@coral-xyz/anchor";

import liquidityIdl from "../../../idls/liquidity.json";
import lendingIdl from "../../../idls/juplend_earn.json";
import rewardsIdl from "../../../idls/lending_reward_rate_model.json";
import { bankRunProvider } from "../../rootHooks";
import {
  JUPLEND_EARN_REWARDS_PROGRAM_ID,
  JUPLEND_LENDING_PROGRAM_ID,
  JUPLEND_LIQUIDITY_PROGRAM_ID,
} from "./juplend-pdas";
import type {
  JuplendLiquidityIdl,
  JuplendLendingIdl,
  JuplendPrograms,
  JuplendRewardsIdl,
} from "./types";

const liquidityIdlWithAddress = {
  ...liquidityIdl,
  address: JUPLEND_LIQUIDITY_PROGRAM_ID.toBase58(),
};
const lendingIdlWithAddress = {
  ...lendingIdl,
  address: JUPLEND_LENDING_PROGRAM_ID.toBase58(),
};
const rewardsIdlWithAddress = {
  ...rewardsIdl,
  address: JUPLEND_EARN_REWARDS_PROGRAM_ID.toBase58(),
};

export const getJuplendPrograms = (): JuplendPrograms => {
  return {
    liquidity: new Program<JuplendLiquidityIdl>(
      liquidityIdlWithAddress as JuplendLiquidityIdl,
      bankRunProvider,
    ),
    lending: new Program<JuplendLendingIdl>(
      lendingIdlWithAddress as JuplendLendingIdl,
      bankRunProvider,
    ),
    rewards: new Program<JuplendRewardsIdl>(
      rewardsIdlWithAddress as JuplendRewardsIdl,
      bankRunProvider,
    ),
  };
};

export const getJuplendProgramIds = () => ({
  liquidity: JUPLEND_LIQUIDITY_PROGRAM_ID.toBase58(),
  lending: JUPLEND_LENDING_PROGRAM_ID.toBase58(),
  rewards: JUPLEND_EARN_REWARDS_PROGRAM_ID.toBase58(),
});
