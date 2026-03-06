/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/kamino_lending.json`.
 */
export type KaminoLending = {
  "address": "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD",
  "metadata": {
    "name": "kaminoLending",
    "version": "1.14.0",
    "spec": "0.1.0"
  },
  "instructions": [
    {
      "name": "initLendingMarket",
      "discriminator": [
        34,
        162,
        116,
        14,
        101,
        137,
        94,
        239
      ],
      "accounts": [
        {
          "name": "lendingMarketOwner",
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingMarket",
          "writable": true
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "systemProgram"
        },
        {
          "name": "rent"
        }
      ],
      "args": [
        {
          "name": "quoteCurrency",
          "type": {
            "array": [
              "u8",
              32
            ]
          }
        }
      ]
    },
    {
      "name": "updateLendingMarket",
      "discriminator": [
        209,
        157,
        53,
        210,
        97,
        180,
        31,
        45
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "lendingMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "mode",
          "type": "u64"
        },
        {
          "name": "value",
          "type": {
            "array": [
              "u8",
              72
            ]
          }
        }
      ]
    },
    {
      "name": "updateLendingMarketOwner",
      "discriminator": [
        118,
        224,
        10,
        62,
        196,
        230,
        184,
        89
      ],
      "accounts": [
        {
          "name": "lendingMarketOwnerCached",
          "signer": true
        },
        {
          "name": "lendingMarket",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "initReserve",
      "discriminator": [
        138,
        245,
        71,
        225,
        153,
        4,
        3,
        43
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "feeReceiver",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "writable": true
        },
        {
          "name": "reserveCollateralSupply",
          "writable": true
        },
        {
          "name": "initialLiquiditySource",
          "writable": true
        },
        {
          "name": "rent"
        },
        {
          "name": "liquidityTokenProgram"
        },
        {
          "name": "collateralTokenProgram"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": []
    },
    {
      "name": "initFarmsForReserve",
      "discriminator": [
        218,
        6,
        62,
        233,
        1,
        33,
        232,
        82
      ],
      "accounts": [
        {
          "name": "lendingMarketOwner",
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "farmsProgram"
        },
        {
          "name": "farmsGlobalConfig"
        },
        {
          "name": "farmState",
          "writable": true
        },
        {
          "name": "farmsVaultAuthority"
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "mode",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateReserveConfig",
      "discriminator": [
        61,
        148,
        100,
        70,
        143,
        107,
        17,
        13
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "globalConfig"
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "mode",
          "type": {
            "defined": {
              "name": "updateConfigMode"
            }
          }
        },
        {
          "name": "value",
          "type": "bytes"
        },
        {
          "name": "skipConfigIntegrityValidation",
          "type": "bool"
        }
      ]
    },
    {
      "name": "redeemFees",
      "discriminator": [
        215,
        39,
        180,
        41,
        173,
        46,
        248,
        220
      ],
      "accounts": [
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveLiquidityFeeReceiver",
          "writable": true
        },
        {
          "name": "reserveSupplyLiquidity",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "withdrawProtocolFee",
      "discriminator": [
        158,
        201,
        158,
        189,
        33,
        93,
        162,
        103
      ],
      "accounts": [
        {
          "name": "globalConfig"
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve"
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "feeVault",
          "writable": true
        },
        {
          "name": "feeCollectorAta",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "seedDepositOnInitReserve",
      "discriminator": [
        254,
        197,
        228,
        118,
        183,
        206,
        62,
        226
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "initialLiquiditySource",
          "writable": true
        },
        {
          "name": "liquidityTokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "socializeLoss",
      "discriminator": [
        245,
        75,
        91,
        0,
        236,
        97,
        19,
        3
      ],
      "accounts": [
        {
          "name": "lendingMarketOwner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "socializeLossV2",
      "discriminator": [
        238,
        95,
        98,
        220,
        187,
        40,
        204,
        154
      ],
      "accounts": [
        {
          "name": "socializeLossAccounts",
          "accounts": [
            {
              "name": "lendingMarketOwner",
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "reserve",
              "writable": true
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "markObligationForDeleveraging",
      "discriminator": [
        164,
        35,
        182,
        19,
        0,
        116,
        243,
        127
      ],
      "accounts": [
        {
          "name": "lendingMarketOwner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        }
      ],
      "args": [
        {
          "name": "autodeleverageTargetLtvPct",
          "type": "u8"
        }
      ]
    },
    {
      "name": "refreshReserve",
      "discriminator": [
        2,
        218,
        138,
        235,
        79,
        201,
        25,
        102
      ],
      "accounts": [
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "pythOracle",
          "optional": true
        },
        {
          "name": "switchboardPriceOracle",
          "optional": true
        },
        {
          "name": "switchboardTwapOracle",
          "optional": true
        },
        {
          "name": "scopePrices",
          "optional": true
        }
      ],
      "args": []
    },
    {
      "name": "refreshReservesBatch",
      "discriminator": [
        144,
        110,
        26,
        103,
        162,
        204,
        252,
        147
      ],
      "accounts": [],
      "args": [
        {
          "name": "skipPriceUpdates",
          "type": "bool"
        }
      ]
    },
    {
      "name": "depositReserveLiquidity",
      "discriminator": [
        169,
        201,
        30,
        126,
        6,
        205,
        102,
        68
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "writable": true
        },
        {
          "name": "userSourceLiquidity",
          "writable": true
        },
        {
          "name": "userDestinationCollateral",
          "writable": true
        },
        {
          "name": "collateralTokenProgram"
        },
        {
          "name": "liquidityTokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "redeemReserveCollateral",
      "discriminator": [
        234,
        117,
        181,
        125,
        185,
        142,
        220,
        29
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveCollateralMint",
          "writable": true
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "userSourceCollateral",
          "writable": true
        },
        {
          "name": "userDestinationLiquidity",
          "writable": true
        },
        {
          "name": "collateralTokenProgram"
        },
        {
          "name": "liquidityTokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initObligation",
      "discriminator": [
        251,
        10,
        231,
        76,
        27,
        11,
        159,
        96
      ],
      "accounts": [
        {
          "name": "obligationOwner",
          "signer": true
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "seed1Account"
        },
        {
          "name": "seed2Account"
        },
        {
          "name": "ownerUserMetadata"
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "initObligationArgs"
            }
          }
        }
      ]
    },
    {
      "name": "initObligationFarmsForReserve",
      "discriminator": [
        136,
        63,
        15,
        186,
        211,
        152,
        168,
        164
      ],
      "accounts": [
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "owner"
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveFarmState",
          "writable": true
        },
        {
          "name": "obligationFarm",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "farmsProgram"
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "mode",
          "type": "u8"
        }
      ]
    },
    {
      "name": "refreshObligationFarmsForReserve",
      "discriminator": [
        140,
        144,
        253,
        21,
        10,
        74,
        248,
        3
      ],
      "accounts": [
        {
          "name": "crank",
          "signer": true
        },
        {
          "name": "baseAccounts",
          "accounts": [
            {
              "name": "obligation"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "reserve"
            },
            {
              "name": "reserveFarmState",
              "writable": true
            },
            {
              "name": "obligationFarmUserState",
              "writable": true
            },
            {
              "name": "lendingMarket"
            }
          ]
        },
        {
          "name": "farmsProgram"
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "mode",
          "type": "u8"
        }
      ]
    },
    {
      "name": "refreshObligation",
      "discriminator": [
        33,
        132,
        147,
        228,
        151,
        192,
        72,
        89
      ],
      "accounts": [
        {
          "name": "lendingMarket"
        },
        {
          "name": "obligation",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "depositObligationCollateral",
      "discriminator": [
        108,
        209,
        4,
        72,
        21,
        22,
        118,
        133
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "depositReserve",
          "writable": true
        },
        {
          "name": "reserveDestinationCollateral",
          "writable": true
        },
        {
          "name": "userSourceCollateral",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "depositObligationCollateralV2",
      "discriminator": [
        137,
        145,
        151,
        94,
        167,
        113,
        4,
        145
      ],
      "accounts": [
        {
          "name": "depositAccounts",
          "accounts": [
            {
              "name": "owner",
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "depositReserve",
              "writable": true
            },
            {
              "name": "reserveDestinationCollateral",
              "writable": true
            },
            {
              "name": "userSourceCollateral",
              "writable": true
            },
            {
              "name": "tokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "withdrawObligationCollateral",
      "discriminator": [
        37,
        116,
        205,
        103,
        243,
        192,
        92,
        198
      ],
      "accounts": [
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "withdrawReserve",
          "writable": true
        },
        {
          "name": "reserveSourceCollateral",
          "writable": true
        },
        {
          "name": "userDestinationCollateral",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "withdrawObligationCollateralV2",
      "discriminator": [
        202,
        249,
        117,
        114,
        231,
        192,
        47,
        138
      ],
      "accounts": [
        {
          "name": "withdrawAccounts",
          "accounts": [
            {
              "name": "owner",
              "writable": true,
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "withdrawReserve",
              "writable": true
            },
            {
              "name": "reserveSourceCollateral",
              "writable": true
            },
            {
              "name": "userDestinationCollateral",
              "writable": true
            },
            {
              "name": "tokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "borrowObligationLiquidity",
      "discriminator": [
        121,
        127,
        18,
        204,
        73,
        245,
        225,
        65
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "borrowReserve",
          "writable": true
        },
        {
          "name": "borrowReserveLiquidityMint"
        },
        {
          "name": "reserveSourceLiquidity",
          "writable": true
        },
        {
          "name": "borrowReserveLiquidityFeeReceiver",
          "writable": true
        },
        {
          "name": "userDestinationLiquidity",
          "writable": true
        },
        {
          "name": "referrerTokenState",
          "writable": true,
          "optional": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "borrowObligationLiquidityV2",
      "discriminator": [
        161,
        128,
        143,
        245,
        171,
        199,
        194,
        6
      ],
      "accounts": [
        {
          "name": "borrowAccounts",
          "accounts": [
            {
              "name": "owner",
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "borrowReserve",
              "writable": true
            },
            {
              "name": "borrowReserveLiquidityMint"
            },
            {
              "name": "reserveSourceLiquidity",
              "writable": true
            },
            {
              "name": "borrowReserveLiquidityFeeReceiver",
              "writable": true
            },
            {
              "name": "userDestinationLiquidity",
              "writable": true
            },
            {
              "name": "referrerTokenState",
              "writable": true,
              "optional": true
            },
            {
              "name": "tokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "repayObligationLiquidity",
      "discriminator": [
        145,
        178,
        13,
        225,
        76,
        240,
        147,
        72
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "repayReserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveDestinationLiquidity",
          "writable": true
        },
        {
          "name": "userSourceLiquidity",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "repayObligationLiquidityV2",
      "discriminator": [
        116,
        174,
        213,
        76,
        180,
        53,
        210,
        144
      ],
      "accounts": [
        {
          "name": "repayAccounts",
          "accounts": [
            {
              "name": "owner",
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "repayReserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveDestinationLiquidity",
              "writable": true
            },
            {
              "name": "userSourceLiquidity",
              "writable": true
            },
            {
              "name": "tokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "repayAndWithdrawAndRedeem",
      "discriminator": [
        2,
        54,
        152,
        3,
        148,
        96,
        109,
        218
      ],
      "accounts": [
        {
          "name": "repayAccounts",
          "accounts": [
            {
              "name": "owner",
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "repayReserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveDestinationLiquidity",
              "writable": true
            },
            {
              "name": "userSourceLiquidity",
              "writable": true
            },
            {
              "name": "tokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "withdrawAccounts",
          "accounts": [
            {
              "name": "owner",
              "writable": true,
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "withdrawReserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveSourceCollateral",
              "writable": true
            },
            {
              "name": "reserveCollateralMint",
              "writable": true
            },
            {
              "name": "reserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "userDestinationLiquidity",
              "writable": true
            },
            {
              "name": "placeholderUserDestinationCollateral",
              "optional": true
            },
            {
              "name": "collateralTokenProgram"
            },
            {
              "name": "liquidityTokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "collateralFarmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "debtFarmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "repayAmount",
          "type": "u64"
        },
        {
          "name": "withdrawCollateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "depositAndWithdraw",
      "discriminator": [
        141,
        153,
        39,
        15,
        64,
        61,
        88,
        84
      ],
      "accounts": [
        {
          "name": "depositAccounts",
          "accounts": [
            {
              "name": "owner",
              "writable": true,
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "reserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "reserveCollateralMint",
              "writable": true
            },
            {
              "name": "reserveDestinationDepositCollateral",
              "writable": true
            },
            {
              "name": "userSourceLiquidity",
              "writable": true
            },
            {
              "name": "placeholderUserDestinationCollateral",
              "optional": true
            },
            {
              "name": "collateralTokenProgram"
            },
            {
              "name": "liquidityTokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "withdrawAccounts",
          "accounts": [
            {
              "name": "owner",
              "writable": true,
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "withdrawReserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveSourceCollateral",
              "writable": true
            },
            {
              "name": "reserveCollateralMint",
              "writable": true
            },
            {
              "name": "reserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "userDestinationLiquidity",
              "writable": true
            },
            {
              "name": "placeholderUserDestinationCollateral",
              "optional": true
            },
            {
              "name": "collateralTokenProgram"
            },
            {
              "name": "liquidityTokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "depositFarmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "withdrawFarmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        },
        {
          "name": "withdrawCollateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "depositReserveLiquidityAndObligationCollateral",
      "discriminator": [
        129,
        199,
        4,
        2,
        222,
        39,
        26,
        46
      ],
      "accounts": [
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "writable": true
        },
        {
          "name": "reserveDestinationDepositCollateral",
          "writable": true
        },
        {
          "name": "userSourceLiquidity",
          "writable": true
        },
        {
          "name": "placeholderUserDestinationCollateral",
          "optional": true
        },
        {
          "name": "collateralTokenProgram"
        },
        {
          "name": "liquidityTokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "depositReserveLiquidityAndObligationCollateralV2",
      "discriminator": [
        216,
        224,
        191,
        27,
        204,
        151,
        102,
        175
      ],
      "accounts": [
        {
          "name": "depositAccounts",
          "accounts": [
            {
              "name": "owner",
              "writable": true,
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "reserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "reserveCollateralMint",
              "writable": true
            },
            {
              "name": "reserveDestinationDepositCollateral",
              "writable": true
            },
            {
              "name": "userSourceLiquidity",
              "writable": true
            },
            {
              "name": "placeholderUserDestinationCollateral",
              "optional": true
            },
            {
              "name": "collateralTokenProgram"
            },
            {
              "name": "liquidityTokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "withdrawObligationCollateralAndRedeemReserveCollateral",
      "discriminator": [
        75,
        93,
        93,
        220,
        34,
        150,
        218,
        196
      ],
      "accounts": [
        {
          "name": "owner",
          "writable": true,
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "withdrawReserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveSourceCollateral",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "writable": true
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "userDestinationLiquidity",
          "writable": true
        },
        {
          "name": "placeholderUserDestinationCollateral",
          "optional": true
        },
        {
          "name": "collateralTokenProgram"
        },
        {
          "name": "liquidityTokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "withdrawObligationCollateralAndRedeemReserveCollateralV2",
      "discriminator": [
        235,
        52,
        119,
        152,
        149,
        197,
        20,
        7
      ],
      "accounts": [
        {
          "name": "withdrawAccounts",
          "accounts": [
            {
              "name": "owner",
              "writable": true,
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "withdrawReserve",
              "writable": true
            },
            {
              "name": "reserveLiquidityMint"
            },
            {
              "name": "reserveSourceCollateral",
              "writable": true
            },
            {
              "name": "reserveCollateralMint",
              "writable": true
            },
            {
              "name": "reserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "userDestinationLiquidity",
              "writable": true
            },
            {
              "name": "placeholderUserDestinationCollateral",
              "optional": true
            },
            {
              "name": "collateralTokenProgram"
            },
            {
              "name": "liquidityTokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "liquidateObligationAndRedeemReserveCollateral",
      "discriminator": [
        177,
        71,
        154,
        188,
        226,
        133,
        74,
        55
      ],
      "accounts": [
        {
          "name": "liquidator",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "repayReserve",
          "writable": true
        },
        {
          "name": "repayReserveLiquidityMint"
        },
        {
          "name": "repayReserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "withdrawReserve",
          "writable": true
        },
        {
          "name": "withdrawReserveLiquidityMint"
        },
        {
          "name": "withdrawReserveCollateralMint",
          "writable": true
        },
        {
          "name": "withdrawReserveCollateralSupply",
          "writable": true
        },
        {
          "name": "withdrawReserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "withdrawReserveLiquidityFeeReceiver",
          "writable": true
        },
        {
          "name": "userSourceLiquidity",
          "writable": true
        },
        {
          "name": "userDestinationCollateral",
          "writable": true
        },
        {
          "name": "userDestinationLiquidity",
          "writable": true
        },
        {
          "name": "collateralTokenProgram"
        },
        {
          "name": "repayLiquidityTokenProgram"
        },
        {
          "name": "withdrawLiquidityTokenProgram"
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        },
        {
          "name": "minAcceptableReceivedLiquidityAmount",
          "type": "u64"
        },
        {
          "name": "maxAllowedLtvOverridePercent",
          "type": "u64"
        }
      ]
    },
    {
      "name": "liquidateObligationAndRedeemReserveCollateralV2",
      "discriminator": [
        162,
        161,
        35,
        143,
        30,
        187,
        185,
        103
      ],
      "accounts": [
        {
          "name": "liquidationAccounts",
          "accounts": [
            {
              "name": "liquidator",
              "signer": true
            },
            {
              "name": "obligation",
              "writable": true
            },
            {
              "name": "lendingMarket"
            },
            {
              "name": "lendingMarketAuthority"
            },
            {
              "name": "repayReserve",
              "writable": true
            },
            {
              "name": "repayReserveLiquidityMint"
            },
            {
              "name": "repayReserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "withdrawReserve",
              "writable": true
            },
            {
              "name": "withdrawReserveLiquidityMint"
            },
            {
              "name": "withdrawReserveCollateralMint",
              "writable": true
            },
            {
              "name": "withdrawReserveCollateralSupply",
              "writable": true
            },
            {
              "name": "withdrawReserveLiquiditySupply",
              "writable": true
            },
            {
              "name": "withdrawReserveLiquidityFeeReceiver",
              "writable": true
            },
            {
              "name": "userSourceLiquidity",
              "writable": true
            },
            {
              "name": "userDestinationCollateral",
              "writable": true
            },
            {
              "name": "userDestinationLiquidity",
              "writable": true
            },
            {
              "name": "collateralTokenProgram"
            },
            {
              "name": "repayLiquidityTokenProgram"
            },
            {
              "name": "withdrawLiquidityTokenProgram"
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "collateralFarmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "debtFarmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        },
        {
          "name": "minAcceptableReceivedLiquidityAmount",
          "type": "u64"
        },
        {
          "name": "maxAllowedLtvOverridePercent",
          "type": "u64"
        }
      ]
    },
    {
      "name": "flashRepayReserveLiquidity",
      "discriminator": [
        185,
        117,
        0,
        203,
        96,
        245,
        180,
        186
      ],
      "accounts": [
        {
          "name": "userTransferAuthority",
          "signer": true
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveDestinationLiquidity",
          "writable": true
        },
        {
          "name": "userSourceLiquidity",
          "writable": true
        },
        {
          "name": "reserveLiquidityFeeReceiver",
          "writable": true
        },
        {
          "name": "referrerTokenState",
          "writable": true,
          "optional": true
        },
        {
          "name": "referrerAccount",
          "writable": true,
          "optional": true
        },
        {
          "name": "sysvarInfo"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        },
        {
          "name": "borrowInstructionIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "flashBorrowReserveLiquidity",
      "discriminator": [
        135,
        231,
        52,
        167,
        7,
        52,
        212,
        193
      ],
      "accounts": [
        {
          "name": "userTransferAuthority",
          "signer": true
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveSourceLiquidity",
          "writable": true
        },
        {
          "name": "userDestinationLiquidity",
          "writable": true
        },
        {
          "name": "reserveLiquidityFeeReceiver",
          "writable": true
        },
        {
          "name": "referrerTokenState",
          "writable": true,
          "optional": true
        },
        {
          "name": "referrerAccount",
          "writable": true,
          "optional": true
        },
        {
          "name": "sysvarInfo"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "liquidityAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "requestElevationGroup",
      "discriminator": [
        36,
        119,
        251,
        129,
        34,
        240,
        7,
        147
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        }
      ],
      "args": [
        {
          "name": "elevationGroup",
          "type": "u8"
        }
      ]
    },
    {
      "name": "initReferrerTokenState",
      "discriminator": [
        116,
        45,
        66,
        148,
        58,
        13,
        218,
        115
      ],
      "accounts": [
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve"
        },
        {
          "name": "referrer"
        },
        {
          "name": "referrerTokenState",
          "writable": true
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": []
    },
    {
      "name": "initUserMetadata",
      "discriminator": [
        117,
        169,
        176,
        69,
        197,
        23,
        15,
        162
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "userMetadata",
          "writable": true
        },
        {
          "name": "referrerUserMetadata",
          "optional": true
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "userLookupTable",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "withdrawReferrerFees",
      "discriminator": [
        171,
        118,
        121,
        201,
        233,
        140,
        23,
        228
      ],
      "accounts": [
        {
          "name": "referrer",
          "writable": true,
          "signer": true
        },
        {
          "name": "referrerTokenState",
          "writable": true
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveSupplyLiquidity",
          "writable": true
        },
        {
          "name": "referrerTokenAccount",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "initReferrerStateAndShortUrl",
      "discriminator": [
        165,
        19,
        25,
        127,
        100,
        55,
        31,
        90
      ],
      "accounts": [
        {
          "name": "referrer",
          "writable": true,
          "signer": true
        },
        {
          "name": "referrerState",
          "writable": true
        },
        {
          "name": "referrerShortUrl",
          "writable": true
        },
        {
          "name": "referrerUserMetadata"
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "shortUrl",
          "type": "string"
        }
      ]
    },
    {
      "name": "deleteReferrerStateAndShortUrl",
      "discriminator": [
        153,
        185,
        99,
        28,
        228,
        179,
        187,
        150
      ],
      "accounts": [
        {
          "name": "referrer",
          "writable": true,
          "signer": true
        },
        {
          "name": "referrerState",
          "writable": true
        },
        {
          "name": "shortUrl",
          "writable": true
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": []
    },
    {
      "name": "setObligationOrder",
      "discriminator": [
        81,
        1,
        99,
        156,
        211,
        83,
        78,
        46
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true
        },
        {
          "name": "obligation",
          "writable": true
        },
        {
          "name": "lendingMarket"
        }
      ],
      "args": [
        {
          "name": "index",
          "type": "u8"
        },
        {
          "name": "order",
          "type": {
            "defined": {
              "name": "obligationOrder"
            }
          }
        }
      ]
    },
    {
      "name": "setBorrowOrder",
      "discriminator": [
        177,
        186,
        45,
        61,
        235,
        91,
        68,
        139
      ],
      "accounts": [
        {
          "name": "owner",
          "docs": [
            "The [Self::obligation]'s owner."
          ],
          "signer": true
        },
        {
          "name": "obligation",
          "docs": [
            "The obligation to set the [BorrowOrder] on."
          ],
          "writable": true
        },
        {
          "name": "lendingMarket",
          "docs": [
            "The [Self::obligation]'s market - needed to validate feature flags and minimum order value."
          ]
        },
        {
          "name": "reserve",
          "docs": [
            "The reserve matching the [Self::debt_liquidity_mint] - needed for the minimum order value",
            "check (and refreshed internally). On cancellation, this account is ignored (but still must",
            "belong to the same market)."
          ]
        },
        {
          "name": "filledDebtDestination",
          "docs": [
            "The [BorrowOrder::filled_debt_destination] to set on order creation. Not editable on order",
            "updates.",
            "Ignored when cancelling the order."
          ]
        },
        {
          "name": "debtLiquidityMint",
          "docs": [
            "The [BorrowOrder::debt_liquidity_mint] to set on order creation. Not editable on order",
            "updates.",
            "Ignored when cancelling the order."
          ]
        },
        {
          "name": "instructionSysvarAccount"
        },
        {
          "name": "eventAuthority"
        },
        {
          "name": "program"
        }
      ],
      "args": [
        {
          "name": "orderConfig",
          "type": {
            "defined": {
              "name": "borrowOrderConfigArgs"
            }
          }
        },
        {
          "name": "minExpectedCurrentRemainingDebtAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "fillBorrowOrder",
      "discriminator": [
        102,
        4,
        167,
        76,
        131,
        170,
        93,
        19
      ],
      "accounts": [
        {
          "name": "borrowAccounts",
          "accounts": [
            {
              "name": "payer",
              "signer": true
            },
            {
              "name": "obligation",
              "docs": [
                "The obligation with a [BorrowOrder]."
              ],
              "writable": true
            },
            {
              "name": "lendingMarket",
              "docs": [
                "The [Self::obligation]'s market - needed for borrowing-related configuration."
              ]
            },
            {
              "name": "lendingMarketAuthority",
              "docs": [
                "The [Self::lending_market]'s authority, needed to transfer the newly-borrowed funds out of",
                "the [Self::reserve_source_liquidity]."
              ]
            },
            {
              "name": "borrowReserve",
              "docs": [
                "The reserve to borrow from.",
                "",
                "Its mint must match the asset requested by the [BorrowOrder::debt_liquidity_mint]."
              ],
              "writable": true
            },
            {
              "name": "borrowReserveLiquidityMint",
              "docs": [
                "The mint of [Self::borrow_reserve] - needed to execute the transfer."
              ]
            },
            {
              "name": "reserveSourceLiquidity",
              "docs": [
                "The vault of [Self::borrow_reserve], from which the funds are transferred."
              ],
              "writable": true
            },
            {
              "name": "borrowReserveLiquidityFeeReceiver",
              "docs": [
                "The fee vault of [Self::borrow_reserve], to which the fees are transferred."
              ],
              "writable": true
            },
            {
              "name": "userDestinationLiquidity",
              "docs": [
                "The destination token account that should receive the newly borrowed funds.",
                "",
                "It must match [BorrowOrder::filled_debt_destination], owner and mint.",
                "",
                "**Warning:** An altered destination account will prevent an order from being filled."
              ],
              "writable": true
            },
            {
              "name": "referrerTokenState",
              "docs": [
                "The referrer's account, for accumulating fees - needed if the [Obligation::has_referrer]."
              ],
              "writable": true,
              "optional": true
            },
            {
              "name": "tokenProgram",
              "docs": [
                "The token program of [Self::borrow_reserve] - needed to execute the transfer."
              ]
            },
            {
              "name": "instructionSysvarAccount"
            }
          ]
        },
        {
          "name": "farmsAccounts",
          "accounts": [
            {
              "name": "obligationFarmUserState",
              "writable": true,
              "optional": true
            },
            {
              "name": "reserveFarmState",
              "writable": true,
              "optional": true
            }
          ]
        },
        {
          "name": "farmsProgram"
        },
        {
          "name": "eventAuthority"
        },
        {
          "name": "program"
        }
      ],
      "args": []
    },
    {
      "name": "enqueueToWithdraw",
      "discriminator": [
        134,
        113,
        160,
        207,
        90,
        75,
        213,
        219
      ],
      "accounts": [
        {
          "name": "owner",
          "docs": [
            "The depositor holding ctokens."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "The market's authority, needed to initialize the [Self::owner_queued_collateral_vault]."
          ]
        },
        {
          "name": "reserve",
          "writable": true
        },
        {
          "name": "userSourceCollateralTa",
          "docs": [
            "The source of collateral to be enqueued."
          ],
          "writable": true
        },
        {
          "name": "userDestinationLiquidityTa",
          "docs": [
            "The account to which the liquidity should be finally transferred later (to be recorded in",
            "the ticket)."
          ]
        },
        {
          "name": "reserveLiquidityMint"
        },
        {
          "name": "reserveCollateralMint"
        },
        {
          "name": "collateralTokenProgram",
          "docs": [
            "The collateral's program - needed for invoking the transfer to the vault *and* (implicitly)",
            "for handling the `init_if_needed`."
          ]
        },
        {
          "name": "withdrawTicket",
          "docs": [
            "The new account to be initialized with the issued ticket's data."
          ],
          "writable": true
        },
        {
          "name": "ownerQueuedCollateralVault",
          "docs": [
            "The per-owner \"this reserve's queued collateral\" vault (in which the collateral will be",
            "locked)."
          ],
          "writable": true
        },
        {
          "name": "systemProgram",
          "docs": [
            "The System program - needed only for `init` / `init_if_needed` of the accounts above."
          ]
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "collateralAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "withdrawQueuedLiquidity",
      "discriminator": [
        66,
        149,
        187,
        201,
        74,
        191,
        174,
        120
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "The executor of the permissionless tx (not necessarily the ticket owner)."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingMarket",
          "docs": [
            "The lending market."
          ]
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "The market's authority, needed for burning the collateral (from",
            "[Self::owner_queued_collateral_vault]) and transferring the liquidity (from",
            "[Self::reserve_liquidity_supply])."
          ]
        },
        {
          "name": "reserve",
          "docs": [
            "The reserve."
          ],
          "writable": true
        },
        {
          "name": "reserveLiquidityMint",
          "docs": [
            "The liquidity mint, needed to invoke the transfer."
          ]
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The collateral mint, needed to burn (`mut`!) the queued collateral."
          ],
          "writable": true
        },
        {
          "name": "reserveLiquiditySupply",
          "docs": [
            "The liquidity supply vault (to withdraw the liquidity from)."
          ],
          "writable": true
        },
        {
          "name": "ownerQueuedCollateralVault",
          "docs": [
            "The per-owner \"this reserve's queued collateral\" vault (from which the collateral will be",
            "burnt)."
          ],
          "writable": true
        },
        {
          "name": "userDestinationLiquidity",
          "docs": [
            "The token account to which the liquidity should be transferred (the one recorded in the",
            "ticket)."
          ],
          "writable": true
        },
        {
          "name": "collateralTokenProgram",
          "docs": [
            "The program of [Self::reserve_collateral_mint], needed for transfer."
          ]
        },
        {
          "name": "liquidityTokenProgram",
          "docs": [
            "The program of [Self::reserve_liquidity_mint], needed for transfer."
          ]
        },
        {
          "name": "withdrawTicket",
          "docs": [
            "The ticket's data itself.",
            "",
            "Note: in case of complete withdrawal, this account will be closed. In case of partial",
            "withdrawal, its [WithdrawTicket::queued_collateral_amount] will simply be reduced, and the",
            "ticket will maintain its position in the queue."
          ],
          "writable": true
        },
        {
          "name": "withdrawTicketOwner",
          "docs": [
            "The owner of the [Self::withdraw_ticket]; needed only to return the rent of the",
            "[WithdrawTicket] account (if it is getting fully-consumed and closed here)."
          ],
          "writable": true
        },
        {
          "name": "associatedTokenProgram",
          "docs": [
            "The ATA program - needed for potential destination ATA creation."
          ]
        },
        {
          "name": "systemProgram",
          "docs": [
            "The System program - needed for potential destination ATA creation."
          ]
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [],
      "returns": "bool"
    },
    {
      "name": "recoverInvalidTicketCollateral",
      "discriminator": [
        28,
        48,
        176,
        102,
        159,
        206,
        210,
        246
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "The transaction executor.",
            "",
            "This instruction is, in principle, permissionless. However, only the ticket owner can use",
            "arbitrary token account as destination for recovered collateral. Other signers can only",
            "transfer the collateral to the ticket owner's ATA (see [Self::user_source_collateral])."
          ],
          "signer": true
        },
        {
          "name": "lendingMarket",
          "docs": [
            "The lending market."
          ]
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "The market's authority, needed for transferring the collateral (from",
            "[Self::owner_queued_collateral_vault])."
          ]
        },
        {
          "name": "reserve",
          "docs": [
            "The reserve, needed only to validate the other accounts."
          ]
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The collateral mint, needed to invoke the transfer."
          ]
        },
        {
          "name": "ownerQueuedCollateralVault",
          "docs": [
            "The per-owner \"this reserve's queued collateral\" vault (from which the collateral will be",
            "recovered)."
          ],
          "writable": true
        },
        {
          "name": "userSourceCollateral",
          "docs": [
            "The ticket's owner token account to which the ticket-locked collateral should be returned.",
            "",
            "Only the ticket's owner can indicate an arbitrary token account here. Permissionless",
            "executors must indicate the ticket's owner ATA."
          ],
          "writable": true
        },
        {
          "name": "collateralTokenProgram",
          "docs": [
            "The program of [Self::reserve_collateral_mint], needed for transfer."
          ]
        },
        {
          "name": "withdrawTicket",
          "docs": [
            "The ticket's account, necessarily marked as [WithdrawTicket::invalid] first (by the",
            "`handler_withdraw_queued_liquidity`)."
          ],
          "writable": true
        },
        {
          "name": "withdrawTicketOwner",
          "docs": [
            "The owner of the [Self::withdraw_ticket]; needed only to return the rent of the",
            "[WithdrawTicket] account."
          ],
          "writable": true
        },
        {
          "name": "instructionSysvarAccount"
        }
      ],
      "args": [
        {
          "name": "ticketSequenceNumber",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initGlobalConfig",
      "discriminator": [
        140,
        136,
        214,
        48,
        87,
        0,
        120,
        255
      ],
      "accounts": [
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "writable": true
        },
        {
          "name": "programData"
        },
        {
          "name": "systemProgram"
        },
        {
          "name": "rent"
        }
      ],
      "args": []
    },
    {
      "name": "updateGlobalConfig",
      "discriminator": [
        164,
        84,
        130,
        189,
        111,
        58,
        250,
        200
      ],
      "accounts": [
        {
          "name": "globalAdmin",
          "signer": true
        },
        {
          "name": "globalConfig",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "mode",
          "type": {
            "defined": {
              "name": "updateGlobalConfigMode"
            }
          }
        },
        {
          "name": "value",
          "type": "bytes"
        }
      ]
    },
    {
      "name": "updateGlobalConfigAdmin",
      "discriminator": [
        184,
        87,
        23,
        193,
        156,
        238,
        175,
        119
      ],
      "accounts": [
        {
          "name": "pendingAdmin",
          "signer": true
        },
        {
          "name": "globalConfig",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "idlMissingTypes",
      "discriminator": [
        130,
        80,
        38,
        153,
        80,
        212,
        182,
        253
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "globalConfig"
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "reserve",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "reserveFarmKind",
          "type": {
            "defined": {
              "name": "reserveFarmKind"
            }
          }
        },
        {
          "name": "feeCalculation",
          "type": {
            "defined": {
              "name": "feeCalculation"
            }
          }
        },
        {
          "name": "reserveStatus",
          "type": {
            "defined": {
              "name": "reserveStatus"
            }
          }
        },
        {
          "name": "updateConfigMode",
          "type": {
            "defined": {
              "name": "updateConfigMode"
            }
          }
        },
        {
          "name": "updateLendingMarketConfigValue",
          "type": {
            "defined": {
              "name": "updateLendingMarketConfigValue"
            }
          }
        },
        {
          "name": "updateLendingMarketConfigMode",
          "type": {
            "defined": {
              "name": "updateLendingMarketMode"
            }
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "userState",
      "discriminator": [
        72,
        177,
        85,
        249,
        76,
        167,
        186,
        126
      ]
    },
    {
      "name": "globalConfig",
      "discriminator": [
        149,
        8,
        156,
        202,
        160,
        252,
        176,
        217
      ]
    },
    {
      "name": "lendingMarket",
      "discriminator": [
        246,
        114,
        50,
        98,
        72,
        157,
        28,
        120
      ]
    },
    {
      "name": "obligation",
      "discriminator": [
        168,
        206,
        141,
        106,
        88,
        76,
        172,
        167
      ]
    },
    {
      "name": "referrerState",
      "discriminator": [
        194,
        81,
        217,
        103,
        12,
        19,
        12,
        66
      ]
    },
    {
      "name": "referrerTokenState",
      "discriminator": [
        39,
        15,
        208,
        77,
        32,
        195,
        105,
        56
      ]
    },
    {
      "name": "shortUrl",
      "discriminator": [
        28,
        89,
        174,
        25,
        226,
        124,
        126,
        212
      ]
    },
    {
      "name": "userMetadata",
      "discriminator": [
        157,
        214,
        220,
        235,
        98,
        135,
        171,
        28
      ]
    },
    {
      "name": "reserve",
      "discriminator": [
        43,
        242,
        204,
        202,
        26,
        247,
        59,
        127
      ]
    },
    {
      "name": "withdrawTicket",
      "discriminator": [
        237,
        23,
        164,
        58,
        53,
        248,
        240,
        94
      ]
    }
  ],
  "events": [
    {
      "name": "borrowOrderCancelEvent",
      "discriminator": [
        88,
        228,
        231,
        230,
        234,
        248,
        69,
        144
      ]
    },
    {
      "name": "borrowOrderFullFillEvent",
      "discriminator": [
        177,
        241,
        237,
        250,
        143,
        20,
        14,
        183
      ]
    },
    {
      "name": "borrowOrderPartialFillEvent",
      "discriminator": [
        113,
        81,
        252,
        193,
        152,
        24,
        99,
        84
      ]
    },
    {
      "name": "borrowOrderPlaceEvent",
      "discriminator": [
        43,
        211,
        208,
        186,
        94,
        227,
        218,
        198
      ]
    },
    {
      "name": "borrowOrderUpdateEvent",
      "discriminator": [
        21,
        33,
        67,
        131,
        48,
        184,
        90,
        64
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "invalidMarketAuthority",
      "msg": "Market authority is invalid"
    },
    {
      "code": 6001,
      "name": "invalidMarketOwner",
      "msg": "Market owner is invalid"
    },
    {
      "code": 6002,
      "name": "invalidAccountOwner",
      "msg": "Input account owner is not the program address"
    },
    {
      "code": 6003,
      "name": "invalidAmount",
      "msg": "Input amount is invalid"
    },
    {
      "code": 6004,
      "name": "invalidConfig",
      "msg": "Input config value is invalid"
    },
    {
      "code": 6005,
      "name": "invalidSigner",
      "msg": "Signer is not allowed to perform this action"
    },
    {
      "code": 6006,
      "name": "invalidAccountInput",
      "msg": "Invalid account input"
    },
    {
      "code": 6007,
      "name": "mathOverflow",
      "msg": "Math operation overflow"
    },
    {
      "code": 6008,
      "name": "insufficientLiquidity",
      "msg": "Insufficient liquidity available"
    },
    {
      "code": 6009,
      "name": "reserveStale",
      "msg": "Reserve state needs to be refreshed"
    },
    {
      "code": 6010,
      "name": "withdrawTooSmall",
      "msg": "Withdraw amount too small"
    },
    {
      "code": 6011,
      "name": "withdrawTooLarge",
      "msg": "Withdraw amount too large"
    },
    {
      "code": 6012,
      "name": "borrowTooSmall",
      "msg": "Borrow amount too small to receive liquidity after fees"
    },
    {
      "code": 6013,
      "name": "borrowTooLarge",
      "msg": "Borrow amount too large for deposited collateral"
    },
    {
      "code": 6014,
      "name": "repayTooSmall",
      "msg": "Repay amount too small to transfer liquidity"
    },
    {
      "code": 6015,
      "name": "liquidationTooSmall",
      "msg": "Liquidation amount too small to receive collateral"
    },
    {
      "code": 6016,
      "name": "obligationHealthy",
      "msg": "Cannot liquidate healthy obligations"
    },
    {
      "code": 6017,
      "name": "obligationStale",
      "msg": "Obligation state needs to be refreshed"
    },
    {
      "code": 6018,
      "name": "obligationReserveLimit",
      "msg": "Obligation reserve limit exceeded"
    },
    {
      "code": 6019,
      "name": "invalidObligationOwner",
      "msg": "Obligation owner is invalid"
    },
    {
      "code": 6020,
      "name": "obligationDepositsEmpty",
      "msg": "Obligation deposits are empty"
    },
    {
      "code": 6021,
      "name": "obligationBorrowsEmpty",
      "msg": "Obligation borrows are empty"
    },
    {
      "code": 6022,
      "name": "obligationDepositsZero",
      "msg": "Obligation deposits have zero value"
    },
    {
      "code": 6023,
      "name": "obligationBorrowsZero",
      "msg": "Obligation borrows have zero value"
    },
    {
      "code": 6024,
      "name": "invalidObligationCollateral",
      "msg": "Invalid obligation collateral"
    },
    {
      "code": 6025,
      "name": "invalidObligationLiquidity",
      "msg": "Invalid obligation liquidity"
    },
    {
      "code": 6026,
      "name": "obligationCollateralEmpty",
      "msg": "Obligation collateral is empty"
    },
    {
      "code": 6027,
      "name": "obligationLiquidityEmpty",
      "msg": "Obligation liquidity is empty"
    },
    {
      "code": 6028,
      "name": "negativeInterestRate",
      "msg": "Interest rate is negative"
    },
    {
      "code": 6029,
      "name": "invalidOracleConfig",
      "msg": "Input oracle config is invalid"
    },
    {
      "code": 6030,
      "name": "insufficientProtocolFeesToRedeem",
      "msg": "Insufficient protocol fees to claim or no liquidity available"
    },
    {
      "code": 6031,
      "name": "flashBorrowCpi",
      "msg": "No cpi flash borrows allowed"
    },
    {
      "code": 6032,
      "name": "noFlashRepayFound",
      "msg": "No corresponding repay found for flash borrow"
    },
    {
      "code": 6033,
      "name": "invalidFlashRepay",
      "msg": "Invalid repay found"
    },
    {
      "code": 6034,
      "name": "flashRepayCpi",
      "msg": "No cpi flash repays allowed"
    },
    {
      "code": 6035,
      "name": "multipleFlashBorrows",
      "msg": "Multiple flash borrows not allowed in the same transaction"
    },
    {
      "code": 6036,
      "name": "flashLoansDisabled",
      "msg": "Flash loans are disabled for this reserve"
    },
    {
      "code": 6037,
      "name": "switchboardV2Error",
      "msg": "Switchboard error"
    },
    {
      "code": 6038,
      "name": "couldNotDeserializeScope",
      "msg": "Cannot deserialize the scope price account"
    },
    {
      "code": 6039,
      "name": "priceTooOld",
      "msg": "Price too old"
    },
    {
      "code": 6040,
      "name": "priceTooDivergentFromTwap",
      "msg": "Price too divergent from twap"
    },
    {
      "code": 6041,
      "name": "invalidTwapPrice",
      "msg": "Invalid twap price"
    },
    {
      "code": 6042,
      "name": "globalEmergencyMode",
      "msg": "Emergency mode is enabled"
    },
    {
      "code": 6043,
      "name": "invalidFlag",
      "msg": "Invalid lending market config"
    },
    {
      "code": 6044,
      "name": "priceNotValid",
      "msg": "Price is not valid"
    },
    {
      "code": 6045,
      "name": "priceIsBiggerThanHeuristic",
      "msg": "Price is bigger than allowed by heuristic"
    },
    {
      "code": 6046,
      "name": "priceIsLowerThanHeuristic",
      "msg": "Price lower than allowed by heuristic"
    },
    {
      "code": 6047,
      "name": "priceIsZero",
      "msg": "Price is zero"
    },
    {
      "code": 6048,
      "name": "priceConfidenceTooWide",
      "msg": "Price confidence too wide"
    },
    {
      "code": 6049,
      "name": "integerOverflow",
      "msg": "Conversion between integers failed"
    },
    {
      "code": 6050,
      "name": "noFarmForReserve",
      "msg": "This reserve does not have a farm"
    },
    {
      "code": 6051,
      "name": "incorrectInstructionInPosition",
      "msg": "Wrong instruction at expected position"
    },
    {
      "code": 6052,
      "name": "noPriceFound",
      "msg": "No price found"
    },
    {
      "code": 6053,
      "name": "invalidTwapConfig",
      "msg": "Invalid Twap configuration: Twap is enabled but one of the enabled price doesn't have a twap"
    },
    {
      "code": 6054,
      "name": "invalidPythPriceAccount",
      "msg": "Pyth price account does not match configuration"
    },
    {
      "code": 6055,
      "name": "invalidSwitchboardAccount",
      "msg": "Switchboard account(s) do not match configuration"
    },
    {
      "code": 6056,
      "name": "invalidScopePriceAccount",
      "msg": "Scope price account does not match configuration"
    },
    {
      "code": 6057,
      "name": "obligationCollateralLtvZero",
      "msg": "The obligation has one collateral with an LTV set to 0. Withdraw it before withdrawing other collaterals"
    },
    {
      "code": 6058,
      "name": "invalidObligationSeedsValue",
      "msg": "Seeds must be default pubkeys for tag 0, and mint addresses for tag 1 or 2"
    },
    {
      "code": 6059,
      "name": "deprecatedInvalidObligationId",
      "msg": "[DEPRECATED] Obligation id must be 0"
    },
    {
      "code": 6060,
      "name": "invalidBorrowRateCurvePoint",
      "msg": "Invalid borrow rate curve point"
    },
    {
      "code": 6061,
      "name": "invalidUtilizationRate",
      "msg": "Invalid utilization rate"
    },
    {
      "code": 6062,
      "name": "cannotSocializeObligationWithCollateral",
      "msg": "Obligation hasn't been fully liquidated and debt cannot be socialized."
    },
    {
      "code": 6063,
      "name": "obligationEmpty",
      "msg": "Obligation has no borrows or deposits."
    },
    {
      "code": 6064,
      "name": "withdrawalCapReached",
      "msg": "Withdrawal cap is reached"
    },
    {
      "code": 6065,
      "name": "lastTimestampGreaterThanCurrent",
      "msg": "The last interval start timestamp is greater than the current timestamp"
    },
    {
      "code": 6066,
      "name": "liquidationRewardTooSmall",
      "msg": "The reward amount is less than the minimum acceptable received liquidity"
    },
    {
      "code": 6067,
      "name": "isolatedAssetTierViolation",
      "msg": "Isolated Asset Tier Violation"
    },
    {
      "code": 6068,
      "name": "inconsistentElevationGroup",
      "msg": "The obligation's elevation group and the reserve's are not the same"
    },
    {
      "code": 6069,
      "name": "invalidElevationGroup",
      "msg": "The elevation group chosen for the reserve does not exist in the lending market"
    },
    {
      "code": 6070,
      "name": "invalidElevationGroupConfig",
      "msg": "The elevation group updated has wrong parameters set"
    },
    {
      "code": 6071,
      "name": "unhealthyElevationGroupLtv",
      "msg": "The current obligation must have most or all its debt repaid before changing the elevation group"
    },
    {
      "code": 6072,
      "name": "elevationGroupNewLoansDisabled",
      "msg": "Elevation group does not accept any new loans or any new borrows/withdrawals"
    },
    {
      "code": 6073,
      "name": "reserveDeprecated",
      "msg": "Reserve was deprecated, no longer usable"
    },
    {
      "code": 6074,
      "name": "referrerAccountNotInitialized",
      "msg": "Referrer account not initialized"
    },
    {
      "code": 6075,
      "name": "referrerAccountMintMissmatch",
      "msg": "Referrer account mint does not match the operation reserve mint"
    },
    {
      "code": 6076,
      "name": "referrerAccountWrongAddress",
      "msg": "Referrer account address is not a valid program address"
    },
    {
      "code": 6077,
      "name": "referrerAccountReferrerMissmatch",
      "msg": "Referrer account referrer does not match the owner referrer"
    },
    {
      "code": 6078,
      "name": "referrerAccountMissing",
      "msg": "Referrer account missing for obligation with referrer"
    },
    {
      "code": 6079,
      "name": "insufficientReferralFeesToRedeem",
      "msg": "Insufficient referral fees to claim or no liquidity available"
    },
    {
      "code": 6080,
      "name": "cpiDisabled",
      "msg": "CPI disabled for this instruction"
    },
    {
      "code": 6081,
      "name": "shortUrlNotAsciiAlphanumeric",
      "msg": "Referrer short_url is not ascii alphanumeric"
    },
    {
      "code": 6082,
      "name": "reserveObsolete",
      "msg": "Reserve is marked as obsolete"
    },
    {
      "code": 6083,
      "name": "elevationGroupAlreadyActivated",
      "msg": "Obligation already part of the same elevation group"
    },
    {
      "code": 6084,
      "name": "obligationInObsoleteReserve",
      "msg": "Obligation has a deposit or borrow in an obsolete reserve"
    },
    {
      "code": 6085,
      "name": "referrerStateOwnerMismatch",
      "msg": "Referrer state owner does not match the given signer"
    },
    {
      "code": 6086,
      "name": "userMetadataOwnerAlreadySet",
      "msg": "User metadata owner is already set"
    },
    {
      "code": 6087,
      "name": "collateralNonLiquidatable",
      "msg": "This collateral cannot be liquidated (LTV set to 0)"
    },
    {
      "code": 6088,
      "name": "borrowingDisabled",
      "msg": "Borrowing is disabled"
    },
    {
      "code": 6089,
      "name": "borrowLimitExceeded",
      "msg": "Cannot borrow above borrow limit"
    },
    {
      "code": 6090,
      "name": "depositLimitExceeded",
      "msg": "Cannot deposit above deposit limit"
    },
    {
      "code": 6091,
      "name": "borrowingDisabledOutsideElevationGroup",
      "msg": "Reserve does not accept any new borrows outside elevation group"
    },
    {
      "code": 6092,
      "name": "netValueRemainingTooSmall",
      "msg": "Net value remaining too small"
    },
    {
      "code": 6093,
      "name": "worseLtvBlocked",
      "msg": "Cannot get the obligation in a worse position"
    },
    {
      "code": 6094,
      "name": "liabilitiesBiggerThanAssets",
      "msg": "Cannot have more liabilities than assets in a position"
    },
    {
      "code": 6095,
      "name": "reserveTokenBalanceMismatch",
      "msg": "Reserve state and token account cannot drift"
    },
    {
      "code": 6096,
      "name": "reserveVaultBalanceMismatch",
      "msg": "Reserve token account has been unexpectedly modified"
    },
    {
      "code": 6097,
      "name": "reserveAccountingMismatch",
      "msg": "Reserve internal state accounting has been unexpectedly modified"
    },
    {
      "code": 6098,
      "name": "borrowingAboveUtilizationRateDisabled",
      "msg": "Borrowing above set utilization rate is disabled"
    },
    {
      "code": 6099,
      "name": "liquidationBorrowFactorPriority",
      "msg": "Liquidation must prioritize the debt with the highest borrow factor"
    },
    {
      "code": 6100,
      "name": "liquidationLowestLiquidationLtvPriority",
      "msg": "Liquidation must prioritize the collateral with the lowest liquidation LTV"
    },
    {
      "code": 6101,
      "name": "elevationGroupBorrowLimitExceeded",
      "msg": "Elevation group borrow limit exceeded"
    },
    {
      "code": 6102,
      "name": "elevationGroupWithoutDebtReserve",
      "msg": "The elevation group does not have a debt reserve defined"
    },
    {
      "code": 6103,
      "name": "elevationGroupMaxCollateralReserveZero",
      "msg": "The elevation group does not allow any collateral reserves"
    },
    {
      "code": 6104,
      "name": "elevationGroupHasAnotherDebtReserve",
      "msg": "In elevation group attempt to borrow from a reserve that is not the debt reserve"
    },
    {
      "code": 6105,
      "name": "elevationGroupDebtReserveAsCollateral",
      "msg": "The elevation group's debt reserve cannot be used as a collateral reserve"
    },
    {
      "code": 6106,
      "name": "obligationCollateralExceedsElevationGroupLimit",
      "msg": "Obligation have more collateral than the maximum allowed by the elevation group"
    },
    {
      "code": 6107,
      "name": "obligationElevationGroupMultipleDebtReserve",
      "msg": "Obligation is an elevation group but have more than one debt reserve"
    },
    {
      "code": 6108,
      "name": "unsupportedTokenExtension",
      "msg": "Mint has a token (2022) extension that is not supported"
    },
    {
      "code": 6109,
      "name": "invalidTokenAccount",
      "msg": "Can't have an spl token mint with a t22 account"
    },
    {
      "code": 6110,
      "name": "depositDisabledOutsideElevationGroup",
      "msg": "Can't deposit into this reserve outside elevation group"
    },
    {
      "code": 6111,
      "name": "cannotCalculateReferralAmountDueToSlotsMismatch",
      "msg": "Cannot calculate referral amount due to slots mismatch"
    },
    {
      "code": 6112,
      "name": "obligationOwnersMustMatch",
      "msg": "Obligation owners must match"
    },
    {
      "code": 6113,
      "name": "obligationsMustMatch",
      "msg": "Obligations must match"
    },
    {
      "code": 6114,
      "name": "lendingMarketsMustMatch",
      "msg": "Lending markets must match"
    },
    {
      "code": 6115,
      "name": "obligationCurrentlyMarkedForDeleveraging",
      "msg": "Obligation is already marked for deleveraging"
    },
    {
      "code": 6116,
      "name": "maximumWithdrawValueZero",
      "msg": "Maximum withdrawable value of this collateral is zero, LTV needs improved"
    },
    {
      "code": 6117,
      "name": "zeroMaxLtvAssetsInDeposits",
      "msg": "No max LTV 0 assets allowed in deposits for repay and withdraw"
    },
    {
      "code": 6118,
      "name": "lowestLtvAssetsPriority",
      "msg": "Withdrawing must prioritize the collateral with the lowest reserve max-LTV"
    },
    {
      "code": 6119,
      "name": "worseLtvThanUnhealthyLtv",
      "msg": "Cannot get the obligation liquidatable"
    },
    {
      "code": 6120,
      "name": "farmAccountsMissing",
      "msg": "Farm accounts to refresh are missing"
    },
    {
      "code": 6121,
      "name": "repayTooSmallForFullLiquidation",
      "msg": "Repay amount is too small to satisfy the mandatory full liquidation"
    },
    {
      "code": 6122,
      "name": "insufficientRepayAmount",
      "msg": "Liquidator provided repay amount lower than required by liquidation rules"
    },
    {
      "code": 6123,
      "name": "orderIndexOutOfBounds",
      "msg": "Obligation order of the given index cannot exist"
    },
    {
      "code": 6124,
      "name": "invalidOrderConfiguration",
      "msg": "Given order configuration has wrong parameters"
    },
    {
      "code": 6125,
      "name": "orderConfigurationNotSupportedByObligation",
      "msg": "Given order configuration cannot be used with the current state of the obligation"
    },
    {
      "code": 6126,
      "name": "operationNotPermittedWithCurrentObligationOrders",
      "msg": "Single debt, single collateral obligation orders have to be cancelled before changing the deposit/borrow count"
    },
    {
      "code": 6127,
      "name": "operationNotPermittedMarketImmutable",
      "msg": "Cannot update lending market because it is set as immutable"
    },
    {
      "code": 6128,
      "name": "orderCreationDisabled",
      "msg": "Creation of new orders is disabled"
    },
    {
      "code": 6129,
      "name": "noUpgradeAuthority",
      "msg": "Cannot initialize global config because there is no upgrade authority to the program"
    },
    {
      "code": 6130,
      "name": "initialAdminDepositExecuted",
      "msg": "Initial admin deposit in reserve already executed"
    },
    {
      "code": 6131,
      "name": "reserveHasNotReceivedInitialDeposit",
      "msg": "Reserve has not received the initial deposit, cannot update config"
    },
    {
      "code": 6132,
      "name": "cTokenUsageBlocked",
      "msg": "CToken minting/redeeming is blocked for this reserve"
    },
    {
      "code": 6133,
      "name": "cannotUseSameReserve",
      "msg": "Cannot call ix with same reserve"
    },
    {
      "code": 6134,
      "name": "transactionIncludesRestrictedPrograms",
      "msg": "Transaction includes restricted programs"
    },
    {
      "code": 6135,
      "name": "borrowOrderDebtLiquidityMintMismatch",
      "msg": "There is no borrow order requesting debt in the given asset"
    },
    {
      "code": 6136,
      "name": "borrowOrderMaxBorrowRateExceeded",
      "msg": "Reserve used for fill exceeds the maximum borrow rate specified by the order"
    },
    {
      "code": 6137,
      "name": "borrowOrderMinDebtTermInsufficient",
      "msg": "Reserve used for fill defines a debt term shorter than specified by the order"
    },
    {
      "code": 6138,
      "name": "borrowOrderFillTimeLimitExceeded",
      "msg": "Borrow order can no longer be filled"
    },
    {
      "code": 6139,
      "name": "reserveDebtMaturityReached",
      "msg": "Cannot borrow from a reserve that reached its debt maturity timestamp"
    },
    {
      "code": 6140,
      "name": "nonUpdatableOrderConfiguration",
      "msg": "Some piece of the order's configuration cannot be updated (the order should be cancelled and placed again)"
    },
    {
      "code": 6141,
      "name": "borrowOrderExecutionDisabled",
      "msg": "Execution of borrow orders is disabled"
    },
    {
      "code": 6142,
      "name": "debtReachedReserveDebtTerm",
      "msg": "Cannot increase the debt that has reached its end of term configured by the reserve"
    },
    {
      "code": 6143,
      "name": "expectationNotMet",
      "msg": "The on-chain state does not meet expectation specified by the caller, so the operation must be aborted (to avoid race conditions)"
    },
    {
      "code": 6144,
      "name": "borrowOrderFillValueTooSmall",
      "msg": "Available liquidity could not satisfy the minimum required borrow order fill value"
    },
    {
      "code": 6145,
      "name": "withdrawTicketIssuanceDisabled",
      "msg": "Issuing new withdraw tickets is disabled by the market"
    },
    {
      "code": 6146,
      "name": "withdrawTicketRedemptionDisabled",
      "msg": "Redeeming withdraw tickets is disabled by the market"
    },
    {
      "code": 6147,
      "name": "withdrawTicketStillValid",
      "msg": "Recovering collateral is only available after the withdraw ticket has been marked invalid"
    },
    {
      "code": 6148,
      "name": "withdrawTicketRequiresFullRedemption",
      "msg": "The withdraw ticket's current state requires that it is fully redeemed (e.g. due to owner ATA creation), but there is not enough liquidity"
    },
    {
      "code": 6149,
      "name": "userTokenBalanceMismatch",
      "msg": "The user's token account has changed its balance in an unexpected way"
    },
    {
      "code": 6150,
      "name": "withdrawQueuedLiquidityValueTooSmall",
      "msg": "Available liquidity could not satisfy the minimum required ticketed withdrawal value"
    },
    {
      "code": 6151,
      "name": "invalidTokenAccountState",
      "msg": "Token account is in a state preventing the handler's operation (e.g. frozen or delegate)"
    },
    {
      "code": 6152,
      "name": "withdrawTicketInvalid",
      "msg": "Cannot use ticket that was already marked invalid"
    },
    {
      "code": 6153,
      "name": "borrowOrderValueTooSmall",
      "msg": "Borrow order's value would be below the market-configured minimum"
    },
    {
      "code": 6154,
      "name": "withdrawTicketValueTooSmall",
      "msg": "Withdraw ticket's value would be below the market-configured minimum"
    }
  ],
  "types": [
    {
      "name": "borrowOrderConfigArgs",
      "docs": [
        "A subset of [BorrowOrderConfig] excluding the accounts passed via [SetBorrowOrder]."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "remainingDebtAmount",
            "type": "u64"
          },
          {
            "name": "maxBorrowRateBps",
            "type": "u32"
          },
          {
            "name": "minDebtTermSeconds",
            "type": "u64"
          },
          {
            "name": "fillableUntilTimestamp",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "updateConfigMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "updateLoanToValuePct"
          },
          {
            "name": "updateMaxLiquidationBonusBps"
          },
          {
            "name": "updateLiquidationThresholdPct"
          },
          {
            "name": "updateProtocolLiquidationFee"
          },
          {
            "name": "updateProtocolTakeRate"
          },
          {
            "name": "updateFeesOriginationFee"
          },
          {
            "name": "updateFeesFlashLoanFee"
          },
          {
            "name": "deprecatedUpdateFeesReferralFeeBps"
          },
          {
            "name": "updateDepositLimit"
          },
          {
            "name": "updateBorrowLimit"
          },
          {
            "name": "updateTokenInfoLowerHeuristic"
          },
          {
            "name": "updateTokenInfoUpperHeuristic"
          },
          {
            "name": "updateTokenInfoExpHeuristic"
          },
          {
            "name": "updateTokenInfoTwapDivergence"
          },
          {
            "name": "updateTokenInfoScopeTwap"
          },
          {
            "name": "updateTokenInfoScopeChain"
          },
          {
            "name": "updateTokenInfoName"
          },
          {
            "name": "updateTokenInfoPriceMaxAge"
          },
          {
            "name": "updateTokenInfoTwapMaxAge"
          },
          {
            "name": "updateScopePriceFeed"
          },
          {
            "name": "updatePythPrice"
          },
          {
            "name": "updateSwitchboardFeed"
          },
          {
            "name": "updateSwitchboardTwapFeed"
          },
          {
            "name": "updateBorrowRateCurve"
          },
          {
            "name": "deprecatedUpdateEntireReserveConfig"
          },
          {
            "name": "updateDebtWithdrawalCap"
          },
          {
            "name": "updateDepositWithdrawalCap"
          },
          {
            "name": "deprecatedUpdateDebtWithdrawalCapCurrentTotal"
          },
          {
            "name": "deprecatedUpdateDepositWithdrawalCapCurrentTotal"
          },
          {
            "name": "updateBadDebtLiquidationBonusBps"
          },
          {
            "name": "updateMinLiquidationBonusBps"
          },
          {
            "name": "updateDeleveragingMarginCallPeriod"
          },
          {
            "name": "updateBorrowFactor"
          },
          {
            "name": "deprecatedUpdateAssetTier"
          },
          {
            "name": "updateElevationGroup"
          },
          {
            "name": "updateDeleveragingThresholdDecreaseBpsPerDay"
          },
          {
            "name": "deprecatedUpdateMultiplierSideBoost"
          },
          {
            "name": "deprecatedUpdateMultiplierTagBoost"
          },
          {
            "name": "updateReserveStatus"
          },
          {
            "name": "updateFarmCollateral"
          },
          {
            "name": "updateFarmDebt"
          },
          {
            "name": "updateDisableUsageAsCollateralOutsideEmode"
          },
          {
            "name": "updateBlockBorrowingAboveUtilizationPct"
          },
          {
            "name": "updateBlockPriceUsage"
          },
          {
            "name": "updateBorrowLimitOutsideElevationGroup"
          },
          {
            "name": "updateBorrowLimitsInElevationGroupAgainstThisReserve"
          },
          {
            "name": "updateHostFixedInterestRateBps"
          },
          {
            "name": "updateAutodeleverageEnabled"
          },
          {
            "name": "updateDeleveragingBonusIncreaseBpsPerDay"
          },
          {
            "name": "updateProtocolOrderExecutionFee"
          },
          {
            "name": "updateProposerAuthorityLock"
          },
          {
            "name": "updateMinDeleveragingBonusBps"
          },
          {
            "name": "updateBlockCTokenUsage"
          },
          {
            "name": "updateDebtMaturityTimestamp"
          },
          {
            "name": "updateDebtTermSeconds"
          }
        ]
      }
    },
    {
      "name": "updateLendingMarketConfigValue",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "bool",
            "fields": [
              "bool"
            ]
          },
          {
            "name": "u8",
            "fields": [
              "u8"
            ]
          },
          {
            "name": "u8Array",
            "fields": [
              {
                "array": [
                  "u8",
                  8
                ]
              }
            ]
          },
          {
            "name": "u16",
            "fields": [
              "u16"
            ]
          },
          {
            "name": "u64",
            "fields": [
              "u64"
            ]
          },
          {
            "name": "u128",
            "fields": [
              "u128"
            ]
          },
          {
            "name": "pubkey",
            "fields": [
              "pubkey"
            ]
          },
          {
            "name": "elevationGroup",
            "fields": [
              {
                "defined": {
                  "name": "elevationGroup"
                }
              }
            ]
          },
          {
            "name": "name",
            "fields": [
              {
                "array": [
                  "u8",
                  32
                ]
              }
            ]
          }
        ]
      }
    },
    {
      "name": "updateLendingMarketMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "updateOwner"
          },
          {
            "name": "updateEmergencyMode"
          },
          {
            "name": "updateLiquidationCloseFactor"
          },
          {
            "name": "updateLiquidationMaxValue"
          },
          {
            "name": "deprecatedUpdateGlobalUnhealthyBorrow"
          },
          {
            "name": "updateGlobalAllowedBorrow"
          },
          {
            "name": "updateEmergencyCouncil"
          },
          {
            "name": "updateMinFullLiquidationThreshold"
          },
          {
            "name": "updateInsolvencyRiskLtv"
          },
          {
            "name": "updateElevationGroup"
          },
          {
            "name": "updateReferralFeeBps"
          },
          {
            "name": "deprecatedUpdateMultiplierPoints"
          },
          {
            "name": "updatePriceRefreshTriggerToMaxAgePct"
          },
          {
            "name": "updateAutodeleverageEnabled"
          },
          {
            "name": "updateBorrowingDisabled"
          },
          {
            "name": "updateMinNetValueObligationPostAction"
          },
          {
            "name": "updateMinValueLtvSkipPriorityLiqCheck"
          },
          {
            "name": "updateMinValueBfSkipPriorityLiqCheck"
          },
          {
            "name": "updatePaddingFields"
          },
          {
            "name": "updateName"
          },
          {
            "name": "updateIndividualAutodeleverageMarginCallPeriodSecs"
          },
          {
            "name": "updateInitialDepositAmount"
          },
          {
            "name": "updateObligationOrderExecutionEnabled"
          },
          {
            "name": "updateImmutableFlag"
          },
          {
            "name": "updateObligationOrderCreationEnabled"
          },
          {
            "name": "updateProposerAuthority"
          },
          {
            "name": "updatePriceTriggeredLiquidationDisabled"
          },
          {
            "name": "updateMatureReserveDebtLiquidationEnabled"
          },
          {
            "name": "updateObligationBorrowDebtTermLiquidationEnabled"
          },
          {
            "name": "updateBorrowOrderCreationEnabled"
          },
          {
            "name": "updateBorrowOrderExecutionEnabled"
          },
          {
            "name": "updateMinBorrowOrderFillValue"
          },
          {
            "name": "updateWithdrawTicketIssuanceEnabled"
          },
          {
            "name": "updateWithdrawTicketRedemptionEnabled"
          },
          {
            "name": "updateMinWithdrawQueuedLiquidityValue"
          }
        ]
      }
    },
    {
      "name": "updateGlobalConfigMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "pendingAdmin"
          },
          {
            "name": "feeCollector"
          }
        ]
      }
    },
    {
      "name": "lastUpdate",
      "docs": [
        "Last update state"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "slot",
            "docs": [
              "Last slot when updated"
            ],
            "type": "u64"
          },
          {
            "name": "stale",
            "docs": [
              "True when marked stale, false when slot updated"
            ],
            "type": "u8"
          },
          {
            "name": "priceStatus",
            "docs": [
              "Status of the prices used to calculate the last update"
            ],
            "type": "u8"
          },
          {
            "name": "placeholder",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          }
        ]
      }
    },
    {
      "name": "elevationGroup",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "maxLiquidationBonusBps",
            "type": "u16"
          },
          {
            "name": "id",
            "type": "u8"
          },
          {
            "name": "ltvPct",
            "type": "u8"
          },
          {
            "name": "liquidationThresholdPct",
            "type": "u8"
          },
          {
            "name": "allowNewLoans",
            "type": "u8"
          },
          {
            "name": "maxReservesAsCollateral",
            "type": "u8"
          },
          {
            "name": "padding0",
            "type": "u8"
          },
          {
            "name": "debtReserve",
            "docs": [
              "Mandatory debt reserve for this elevation group"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u64",
                4
              ]
            }
          }
        ]
      }
    },
    {
      "name": "borrowOrder",
      "docs": [
        "A borrow order.",
        "",
        "When the [Obligation::borrow_order] is populated (i.e. non-zeroed) on an Obligation, then the",
        "permissionless \"fill\" operations may borrow liquidity to the owner according to this",
        "specification."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "debtLiquidityMint",
            "docs": [
              "The asset to be borrowed.",
              "The reserves used for [Obligation::borrows] *must* all provide exactly this asset."
            ],
            "type": "pubkey"
          },
          {
            "name": "remainingDebtAmount",
            "docs": [
              "The amount of debt that still needs to be filled, in lamports."
            ],
            "type": "u64"
          },
          {
            "name": "filledDebtDestination",
            "docs": [
              "The token account owned by the [Obligation::owner] and holding [Self::debt_liquidity_mint],",
              "where the filled funds should be transferred to."
            ],
            "type": "pubkey"
          },
          {
            "name": "minDebtTermSeconds",
            "docs": [
              "The minimum allowed debt term that the obligation owner agrees to.",
              "The reserves used to fill this order *cannot* define their debt term *lower* than this.",
              "",
              "If zeroed, then only indefinite-term reserves may be used."
            ],
            "type": "u64"
          },
          {
            "name": "fillableUntilTimestamp",
            "docs": [
              "The time until which the borrow order can still be filled."
            ],
            "type": "u64"
          },
          {
            "name": "placedAtTimestamp",
            "docs": [
              "The time at which this order was placed.",
              "Currently, this is only a piece of metadata."
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdatedAtTimestamp",
            "docs": [
              "The time at which this order was most-recently updated (including: created).",
              "Currently, this is only a piece of metadata."
            ],
            "type": "u64"
          },
          {
            "name": "requestedDebtAmount",
            "docs": [
              "The amount of debt that was originally requested when this order was most-recently updated.",
              "In other words: this field holds a value of [Self::remaining_debt_amount] captured at",
              "[Self::last_updated_at_timestamp].",
              "Currently, this is only a piece of metadata."
            ],
            "type": "u64"
          },
          {
            "name": "maxBorrowRateBps",
            "docs": [
              "The maximum borrow rate that the obligation owner agrees to.",
              "The reserves used for [Obligation::borrows] *cannot* define their maximum borrow rate",
              "*higher* than this."
            ],
            "type": "u32"
          },
          {
            "name": "active",
            "docs": [
              "Whether the [Self::remaining_debt_amount] is non-zero.",
              "",
              "This field is *not* used by smart contract logic (which prefers to treat the above",
              "[Self::remaining_debt_amount]-based definition as the single source of truth). However, it",
              "is useful for off-chain bots (order-searchers) to efficiently list (i.e. `memcmp` filter)",
              "just the obligations that have active borrow orders."
            ],
            "type": "u8"
          },
          {
            "name": "padding1",
            "docs": [
              "Alignment padding."
            ],
            "type": {
              "array": [
                "u8",
                3
              ]
            }
          },
          {
            "name": "endPadding",
            "docs": [
              "End padding."
            ],
            "type": {
              "array": [
                "u64",
                5
              ]
            }
          }
        ]
      }
    },
    {
      "name": "initObligationArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "tag",
            "type": "u8"
          },
          {
            "name": "id",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "obligationCollateral",
      "docs": [
        "Obligation collateral state"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "depositReserve",
            "docs": [
              "Reserve collateral is deposited to"
            ],
            "type": "pubkey"
          },
          {
            "name": "depositedAmount",
            "docs": [
              "Amount of collateral deposited"
            ],
            "type": "u64"
          },
          {
            "name": "marketValueSf",
            "docs": [
              "Collateral market value in quote currency (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "borrowedAmountAgainstThisCollateralInElevationGroup",
            "docs": [
              "Debt amount (lamport) taken against this collateral.",
              "(only meaningful if this obligation is part of an elevation group, otherwise 0)",
              "This is only indicative of the debt computed on the last refresh obligation.",
              "If the obligation have multiple collateral this value is the same for all of them."
            ],
            "type": "u64"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                9
              ]
            }
          }
        ]
      }
    },
    {
      "name": "obligationLiquidity",
      "docs": [
        "Obligation liquidity state"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "borrowReserve",
            "docs": [
              "Reserve liquidity is borrowed from"
            ],
            "type": "pubkey"
          },
          {
            "name": "cumulativeBorrowRateBsf",
            "docs": [
              "Borrow rate used for calculating interest (big scaled fraction)"
            ],
            "type": {
              "defined": {
                "name": "bigFractionBytes"
              }
            }
          },
          {
            "name": "firstBorrowedAtTimestamp",
            "docs": [
              "The timestamp at which this debt was taken.",
              "More specifically: when the *first* borrow operation from this reserve happened.",
              "This means that:",
              "- adding debt of the same reserve does *not* change this timestamp,",
              "- repaying the entire debt of this reserve *does* reset this timestamp.",
              "",
              "Note: this field is *not* only metadata: it is used in the logic, e.g. for enforcing the",
              "fixed-term borrows (i.e. those induced by [ReserveConfig::debt_term_seconds])."
            ],
            "type": "u64"
          },
          {
            "name": "borrowedAmountSf",
            "docs": [
              "Amount of liquidity borrowed plus interest (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "marketValueSf",
            "docs": [
              "Liquidity market value in quote currency (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "borrowFactorAdjustedMarketValueSf",
            "docs": [
              "Risk adjusted liquidity market value in quote currency - DEBUG ONLY - use market_value instead"
            ],
            "type": "u128"
          },
          {
            "name": "borrowedAmountOutsideElevationGroups",
            "docs": [
              "Amount of liquidity borrowed outside of an elevation group"
            ],
            "type": "u64"
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u64",
                7
              ]
            }
          }
        ]
      }
    },
    {
      "name": "obligationOrder",
      "docs": [
        "A single obligation order.",
        "See [Obligation::obligation_orders]."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "conditionThresholdSf",
            "docs": [
              "A threshold value used by the condition (scaled [Fraction]).",
              "The exact meaning depends on the specific [Self::condition_type].",
              "",
              "Examples:",
              "- when `condition_type == 2 (UserLtvBelow)`:",
              "then a value of `0.455` here means that the order is active only when the obligation's",
              "user LTV is less than `0.455` (i.e. < 45.5%).",
              "- when `condition_type == 3 (DebtCollPriceRatioAbove)`:",
              "assuming the obligation uses BTC collateral for SOL debt, then a value of `491.3` here",
              "means that the order is active only when the BTC-SOL price is greater than `491.3` (i.e.",
              "> 491.3 SOL per BTC)."
            ],
            "type": "u128"
          },
          {
            "name": "opportunityParameterSf",
            "docs": [
              "A configuration parameter used by the opportunity (scaled [Fraction]).",
              "The exact meaning depends on the specific [Self::opportunity_type].",
              "",
              "Examples:",
              "- when `opportunity_type == 0 (DeleverageSingleDebtAmount)`:",
              "Assuming the obligation uses BTC collateral for SOL debt, then a value of `1_234_000_000`",
              "here means that a liquidator may repay up to 1234000000 lamports (i.e. 1.234 SOL) on this",
              "obligation.",
              "Note: the special value of [Fraction::MAX] is *not* allowed in this case.",
              "- when `opportunity_type == 1 (DeleverageAllDebtAmount)`:",
              "The only allowed value in this case is [Fraction::MAX] (to emphasize that *all* debt",
              "should be repaid)."
            ],
            "type": "u128"
          },
          {
            "name": "minExecutionBonusBps",
            "docs": [
              "A *minimum* additional fraction of collateral transferred to the liquidator, in bps.",
              "",
              "The minimum bonus is applied exactly when the [Self::condition_threshold_sf] is met, and",
              "grows linearly towards the [Self::max_execution_bonus_bps].",
              "",
              "Example: a value of `50` here means 50bps == 0.5% bonus for an \"LTV > 65%\" order, when",
              "executed precisely at the moment LTV exceeds 65%."
            ],
            "type": "u16"
          },
          {
            "name": "maxExecutionBonusBps",
            "docs": [
              "A *maximum* additional fraction of collateral transferred to the liquidator, in bps.",
              "",
              "The maximum bonus is applied at the relevant \"extreme\" state of the obligation, i.e.:",
              "- for a stop-loss condition, it is a point at which the obligation becomes liquidatable;",
              "- for a take-profit condition, it is a point at which obligation has 0% LTV.",
              "",
              "In non-extreme states, the actual bonus value is interpolated linearly, starting from",
              "[Self::min_execution_bonus_bps] (at the point specified by the order's condition).",
              "",
              "Example: a value of `300` here means 300bps == 3.0% bonus for a \"debt/coll price > 140\"",
              "order, when executed at a higher price = 200, at which the obligation's LTV happens to",
              "be equal to its liquidation LTV."
            ],
            "type": "u16"
          },
          {
            "name": "conditionType",
            "docs": [
              "Serialized [ConditionType].",
              "The entire order is void when this is zeroed (i.e. representing [ConditionType::Never]).",
              "",
              "Example: a value of `2` here denotes `UserLtvBelow` condition type. Of course, to",
              "interpret this condition, we also need to take the [Self::condition_threshold_sf] into",
              "account."
            ],
            "type": "u8"
          },
          {
            "name": "opportunityType",
            "docs": [
              "Serialized [OpportunityType].",
              "",
              "Example: a value of `0` here denotes `DeleverageSingleDebtAmount` opportunity. Of course, to",
              "interpret this opportunity, we also need to take the [Self::opportunity_parameter_sf] into",
              "account."
            ],
            "type": "u8"
          },
          {
            "name": "padding1",
            "docs": [
              "Alignment padding.",
              "The fields above take up 2+2+1+1 bytes = 48 bits, which means we need 80 bits = 10 bytes to",
              "align with `u128`s."
            ],
            "type": {
              "array": [
                "u8",
                10
              ]
            }
          },
          {
            "name": "padding2",
            "docs": [
              "End padding.",
              "The total size of a single instance is 8*u128 = 128 bytes."
            ],
            "type": {
              "array": [
                "u128",
                5
              ]
            }
          }
        ]
      }
    },
    {
      "name": "bigFractionBytes",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "value",
            "type": {
              "array": [
                "u64",
                4
              ]
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                2
              ]
            }
          }
        ]
      }
    },
    {
      "name": "feeCalculation",
      "docs": [
        "Calculate fees exlusive or inclusive of an amount"
      ],
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "exclusive"
          },
          {
            "name": "inclusive"
          }
        ]
      }
    },
    {
      "name": "reserveCollateral",
      "docs": [
        "Reserve collateral"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "mintPubkey",
            "docs": [
              "Reserve collateral mint address"
            ],
            "type": "pubkey"
          },
          {
            "name": "mintTotalSupply",
            "docs": [
              "Reserve collateral mint supply, used for exchange rate"
            ],
            "type": "u64"
          },
          {
            "name": "supplyVault",
            "docs": [
              "Reserve collateral supply address"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u128",
                32
              ]
            }
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u128",
                32
              ]
            }
          }
        ]
      }
    },
    {
      "name": "reserveConfig",
      "docs": [
        "Reserve configuration values"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "status",
            "docs": [
              "Status of the reserve Active/Obsolete/Hidden"
            ],
            "type": "u8"
          },
          {
            "name": "paddingDeprecatedAssetTier",
            "docs": [
              "Asset tier -> 0 - regular (collateral & debt), 1 - isolated collateral, 2 - isolated debt"
            ],
            "type": "u8"
          },
          {
            "name": "hostFixedInterestRateBps",
            "docs": [
              "Flat rate that goes to the host"
            ],
            "type": "u16"
          },
          {
            "name": "minDeleveragingBonusBps",
            "docs": [
              "Starting bonus for deleveraging-related liquidations, in bps."
            ],
            "type": "u16"
          },
          {
            "name": "blockCtokenUsage",
            "docs": [
              "Boolean flag to block minting/redeeming of ctokens",
              "Blocks usage of ctokens (minting or withdrawing from obligation)",
              "Effectively blocks deposit_reserve_liquidity and withdraw_obligation_collateral"
            ],
            "type": "u8"
          },
          {
            "name": "reserved1",
            "docs": [
              "Past reserved space - feel free to reuse."
            ],
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "protocolOrderExecutionFeePct",
            "docs": [
              "Cut of the order execution bonus that the protocol receives, as a percentage"
            ],
            "type": "u8"
          },
          {
            "name": "protocolTakeRatePct",
            "docs": [
              "Protocol take rate is the amount borrowed interest protocol receives, as a percentage"
            ],
            "type": "u8"
          },
          {
            "name": "protocolLiquidationFeePct",
            "docs": [
              "Cut of the liquidation bonus that the protocol receives, as a percentage"
            ],
            "type": "u8"
          },
          {
            "name": "loanToValuePct",
            "docs": [
              "Target ratio of the value of borrows to deposits, as a percentage",
              "0 if use as collateral is disabled"
            ],
            "type": "u8"
          },
          {
            "name": "liquidationThresholdPct",
            "docs": [
              "Loan to value ratio at which an obligation can be liquidated, as percentage"
            ],
            "type": "u8"
          },
          {
            "name": "minLiquidationBonusBps",
            "docs": [
              "Minimum bonus a liquidator receives when repaying part of an unhealthy obligation, as bps"
            ],
            "type": "u16"
          },
          {
            "name": "maxLiquidationBonusBps",
            "docs": [
              "Maximum bonus a liquidator receives when repaying part of an unhealthy obligation, as bps"
            ],
            "type": "u16"
          },
          {
            "name": "badDebtLiquidationBonusBps",
            "docs": [
              "Bad debt liquidation bonus for an undercollateralized obligation, as bps"
            ],
            "type": "u16"
          },
          {
            "name": "deleveragingMarginCallPeriodSecs",
            "docs": [
              "Time in seconds that must pass before redemptions are enabled after the deposit limit is",
              "crossed.",
              "Only relevant when `autodeleverage_enabled == 1`, and must not be 0 in such case."
            ],
            "type": "u64"
          },
          {
            "name": "deleveragingThresholdDecreaseBpsPerDay",
            "docs": [
              "The rate at which the deleveraging threshold decreases, in bps per day.",
              "Only relevant when `autodeleverage_enabled == 1`, and must not be 0 in such case."
            ],
            "type": "u64"
          },
          {
            "name": "fees",
            "docs": [
              "Program owner fees assessed, separate from gains due to interest accrual"
            ],
            "type": {
              "defined": {
                "name": "reserveFees"
              }
            }
          },
          {
            "name": "borrowRateCurve",
            "docs": [
              "Borrow rate curve based on utilization"
            ],
            "type": {
              "defined": {
                "name": "borrowRateCurve"
              }
            }
          },
          {
            "name": "borrowFactorPct",
            "docs": [
              "Borrow factor in percentage - used for risk adjustment"
            ],
            "type": "u64"
          },
          {
            "name": "depositLimit",
            "docs": [
              "Maximum deposit limit of liquidity in native units, u64::MAX for inf"
            ],
            "type": "u64"
          },
          {
            "name": "borrowLimit",
            "docs": [
              "Maximum amount borrowed, u64::MAX for inf, 0 to disable borrows (protected deposits)"
            ],
            "type": "u64"
          },
          {
            "name": "tokenInfo",
            "docs": [
              "Token id from TokenInfos struct"
            ],
            "type": {
              "defined": {
                "name": "tokenInfo"
              }
            }
          },
          {
            "name": "depositWithdrawalCap",
            "docs": [
              "Deposit withdrawal caps - deposit & redeem"
            ],
            "type": {
              "defined": {
                "name": "withdrawalCaps"
              }
            }
          },
          {
            "name": "debtWithdrawalCap",
            "docs": [
              "Debt withdrawal caps - borrow & repay"
            ],
            "type": {
              "defined": {
                "name": "withdrawalCaps"
              }
            }
          },
          {
            "name": "elevationGroups",
            "type": {
              "array": [
                "u8",
                20
              ]
            }
          },
          {
            "name": "disableUsageAsCollOutsideEmode",
            "type": "u8"
          },
          {
            "name": "utilizationLimitBlockBorrowingAbovePct",
            "docs": [
              "Utilization (in percentage) above which borrowing is blocked. 0 to disable."
            ],
            "type": "u8"
          },
          {
            "name": "autodeleverageEnabled",
            "docs": [
              "Whether this reserve should be subject to auto-deleveraging after deposit or borrow limit is",
              "crossed.",
              "Besides this flag, the lending market's flag also needs to be enabled (logical `AND`).",
              "**NOTE:** the manual \"target LTV\" deleveraging is NOT affected by this flag."
            ],
            "type": "u8"
          },
          {
            "name": "proposerAuthorityLocked",
            "docs": [
              "Boolean flag indicating whether the reserve is locked for the proposer authority.",
              "",
              "Once the proposer have finished preparing the reserve, it must be locked to prevent",
              "further changes to the reserve configuration allowing review and voting on the proposal",
              "without alteration during the voting period."
            ],
            "type": "u8"
          },
          {
            "name": "borrowLimitOutsideElevationGroup",
            "docs": [
              "Maximum amount liquidity of this reserve borrowed outside all elevation groups",
              "- u64::MAX for inf",
              "- 0 to disable borrows outside elevation groups"
            ],
            "type": "u64"
          },
          {
            "name": "borrowLimitAgainstThisCollateralInElevationGroup",
            "docs": [
              "Defines the maximum amount (in lamports of elevation group debt asset)",
              "that can be borrowed when this reserve is used as collateral.",
              "- u64::MAX for inf",
              "- 0 to disable borrows in this elevation group (expected value for the debt asset)"
            ],
            "type": {
              "array": [
                "u64",
                32
              ]
            }
          },
          {
            "name": "deleveragingBonusIncreaseBpsPerDay",
            "docs": [
              "The rate at which the deleveraging-related liquidation bonus increases, in bps per day.",
              "Only relevant when `autodeleverage_enabled == 1`, and must not be 0 in such case."
            ],
            "type": "u64"
          },
          {
            "name": "debtMaturityTimestamp",
            "docs": [
              "The timestamp at which all [Obligation::borrows] using this reserve become liquidatable",
              "(on the same terms as reserve-wide deleveraging).",
              "Inactive when zeroed (i.e. debt never matures).",
              "",
              "Note: this feature is independent of [Self::debt_term_seconds] - the liquidation mechanism",
              "is based directly on the timestamp defined here, on Reserve's level."
            ],
            "type": "u64"
          },
          {
            "name": "debtTermSeconds",
            "docs": [
              "The duration after which any debt coming from this Reserve must be repaid.",
              "Inactive when zeroed (i.e. funds can be borrowed indefinitely).",
              "",
              "Note: this feature is independent of [Self::debt_maturity_timestamp] - the liquidation",
              "mechanism is based on the [ObligationLiquidity::first_borrowed_at_timestamp]."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "reserveFarmKind",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "collateral"
          },
          {
            "name": "debt"
          }
        ]
      }
    },
    {
      "name": "reserveFees",
      "docs": [
        "Additional fee information on a reserve",
        "",
        "These exist separately from interest accrual fees, and are specifically for the program owner",
        "and referral fee. The fees are paid out as a percentage of liquidity token amounts during",
        "repayments and liquidations."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "originationFeeSf",
            "docs": [
              "Fee assessed on `BorrowObligationLiquidity`, as scaled fraction (60 bits fractional part)",
              "Must be between `0` and `2^60`, such that `2^60 = 1`.  A few examples for",
              "clarity:",
              "1% = (1 << 60) / 100 = 11529215046068470",
              "0.01% (1 basis point) = 115292150460685",
              "0.00001% (Aave origination fee) = 115292150461"
            ],
            "type": "u64"
          },
          {
            "name": "flashLoanFeeSf",
            "docs": [
              "Fee for flash loan, expressed as scaled fraction.",
              "0.3% (Aave flash loan fee) = 0.003 * 2^60 = 3458764513820541"
            ],
            "type": "u64"
          },
          {
            "name": "padding",
            "docs": [
              "Used for allignment"
            ],
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          }
        ]
      }
    },
    {
      "name": "reserveLiquidity",
      "docs": [
        "Reserve liquidity"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "mintPubkey",
            "docs": [
              "Reserve liquidity mint address"
            ],
            "type": "pubkey"
          },
          {
            "name": "supplyVault",
            "docs": [
              "Reserve liquidity supply address"
            ],
            "type": "pubkey"
          },
          {
            "name": "feeVault",
            "docs": [
              "Reserve liquidity fee collection address"
            ],
            "type": "pubkey"
          },
          {
            "name": "totalAvailableAmount",
            "docs": [
              "Total reserve liquidity available.",
              "",
              "Note: not all of this liquidity can be freely used for any purpose. Production code should",
              "use the specialized getters - see e.g. [Reserve::total_available_liquidity_amount()],",
              "[Reserve::freely_available_liquidity_amount()]."
            ],
            "type": "u64"
          },
          {
            "name": "borrowedAmountSf",
            "docs": [
              "Reserve liquidity borrowed (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "marketPriceSf",
            "docs": [
              "Reserve liquidity market price in quote currency (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "marketPriceLastUpdatedTs",
            "docs": [
              "Unix timestamp of the market price (from the oracle)"
            ],
            "type": "u64"
          },
          {
            "name": "mintDecimals",
            "docs": [
              "Reserve liquidity mint decimals"
            ],
            "type": "u64"
          },
          {
            "name": "depositLimitCrossedTimestamp",
            "docs": [
              "Timestamp when the last refresh reserve detected that the liquidity amount is above the deposit cap. When this threshold is crossed, then redemptions (auto-deleverage) are enabled.",
              "If the threshold is not crossed, then the timestamp is set to 0"
            ],
            "type": "u64"
          },
          {
            "name": "borrowLimitCrossedTimestamp",
            "docs": [
              "Timestamp when the last refresh reserve detected that the borrowed amount is above the borrow cap. When this threshold is crossed, then redemptions (auto-deleverage) are enabled.",
              "If the threshold is not crossed, then the timestamp is set to 0"
            ],
            "type": "u64"
          },
          {
            "name": "cumulativeBorrowRateBsf",
            "docs": [
              "Reserve liquidity cumulative borrow rate (scaled fraction)"
            ],
            "type": {
              "defined": {
                "name": "bigFractionBytes"
              }
            }
          },
          {
            "name": "accumulatedProtocolFeesSf",
            "docs": [
              "Reserve cumulative protocol fees (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "accumulatedReferrerFeesSf",
            "docs": [
              "Reserve cumulative referrer fees (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "pendingReferrerFeesSf",
            "docs": [
              "Reserve pending referrer fees, to be claimed in refresh_obligation by referrer or protocol (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "absoluteReferralRateSf",
            "docs": [
              "Reserve referrer fee absolute rate calculated at each refresh_reserve operation (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "tokenProgram",
            "docs": [
              "Token program of the liquidity mint"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u64",
                51
              ]
            }
          },
          {
            "name": "padding3",
            "type": {
              "array": [
                "u128",
                32
              ]
            }
          }
        ]
      }
    },
    {
      "name": "reserveStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "active"
          },
          {
            "name": "obsolete"
          },
          {
            "name": "hidden"
          }
        ]
      }
    },
    {
      "name": "withdrawQueue",
      "docs": [
        "A tracker of ticket-based withdrawals."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "queuedCollateralAmount",
            "docs": [
              "The part of [ReserveLiquidity::total_available_amount] locked for ticketed withdrawals."
            ],
            "type": "u64"
          },
          {
            "name": "nextIssuedTicketSequenceNumber",
            "docs": [
              "The sequence number of the next ticket to be issued when enqueueing to withdraw.",
              "Note: it is also a number of tickets issued so far."
            ],
            "type": "u64"
          },
          {
            "name": "nextWithdrawableTicketSequenceNumber",
            "docs": [
              "The sequence number of the next ticket to be used for actually transferring the withdrawn",
              "liquidity (assuming it is available in the reserve).",
              "Note: it is also a number of fully-consumed tickets so far."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "withdrawalCaps",
      "docs": [
        "Reserve Withdrawal Caps State"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "configCapacity",
            "type": "i64"
          },
          {
            "name": "currentTotal",
            "type": "i64"
          },
          {
            "name": "lastIntervalStartTimestamp",
            "type": "u64"
          },
          {
            "name": "configIntervalLengthSeconds",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "priceHeuristic",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lower",
            "docs": [
              "Lower value of acceptable price"
            ],
            "type": "u64"
          },
          {
            "name": "upper",
            "docs": [
              "Upper value of acceptable price"
            ],
            "type": "u64"
          },
          {
            "name": "exp",
            "docs": [
              "Number of decimals of the previously defined values"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "pythConfiguration",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "price",
            "docs": [
              "Pubkey of the base price feed (disabled if `null` or `default`)"
            ],
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "scopeConfiguration",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "priceFeed",
            "docs": [
              "Pubkey of the scope price feed (disabled if `null` or `default`)"
            ],
            "type": "pubkey"
          },
          {
            "name": "priceChain",
            "docs": [
              "This is the scope_id price chain that results in a price for the token"
            ],
            "type": {
              "array": [
                "u16",
                4
              ]
            }
          },
          {
            "name": "twapChain",
            "docs": [
              "This is the scope_id price chain for the twap"
            ],
            "type": {
              "array": [
                "u16",
                4
              ]
            }
          }
        ]
      }
    },
    {
      "name": "switchboardConfiguration",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "priceAggregator",
            "docs": [
              "Pubkey of the base price feed (disabled if `null` or `default`)"
            ],
            "type": "pubkey"
          },
          {
            "name": "twapAggregator",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "tokenInfo",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "name",
            "docs": [
              "UTF-8 encoded name of the token (null-terminated)"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "heuristic",
            "docs": [
              "Heuristics limits of acceptable price"
            ],
            "type": {
              "defined": {
                "name": "priceHeuristic"
              }
            }
          },
          {
            "name": "maxTwapDivergenceBps",
            "docs": [
              "Max divergence between twap and price in bps"
            ],
            "type": "u64"
          },
          {
            "name": "maxAgePriceSeconds",
            "type": "u64"
          },
          {
            "name": "maxAgeTwapSeconds",
            "type": "u64"
          },
          {
            "name": "scopeConfiguration",
            "docs": [
              "Scope price configuration"
            ],
            "type": {
              "defined": {
                "name": "scopeConfiguration"
              }
            }
          },
          {
            "name": "switchboardConfiguration",
            "docs": [
              "Switchboard configuration"
            ],
            "type": {
              "defined": {
                "name": "switchboardConfiguration"
              }
            }
          },
          {
            "name": "pythConfiguration",
            "docs": [
              "Pyth configuration"
            ],
            "type": {
              "defined": {
                "name": "pythConfiguration"
              }
            }
          },
          {
            "name": "blockPriceUsage",
            "type": "u8"
          },
          {
            "name": "reserved",
            "type": {
              "array": [
                "u8",
                7
              ]
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                19
              ]
            }
          }
        ]
      }
    },
    {
      "name": "borrowRateCurve",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "points",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "curvePoint"
                  }
                },
                11
              ]
            }
          }
        ]
      }
    },
    {
      "name": "curvePoint",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "utilizationRateBps",
            "type": "u32"
          },
          {
            "name": "borrowRateBps",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "userState",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "userId",
            "type": "u64"
          },
          {
            "name": "farmState",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "isFarmDelegated",
            "type": "u8"
          },
          {
            "name": "padding0",
            "type": {
              "array": [
                "u8",
                7
              ]
            }
          },
          {
            "name": "rewardsTallyScaled",
            "type": {
              "array": [
                "u128",
                10
              ]
            }
          },
          {
            "name": "rewardsIssuedUnclaimed",
            "type": {
              "array": [
                "u64",
                10
              ]
            }
          },
          {
            "name": "lastClaimTs",
            "type": {
              "array": [
                "u64",
                10
              ]
            }
          },
          {
            "name": "activeStakeScaled",
            "type": "u128"
          },
          {
            "name": "pendingDepositStakeScaled",
            "type": "u128"
          },
          {
            "name": "pendingDepositStakeTs",
            "type": "u64"
          },
          {
            "name": "pendingWithdrawalUnstakeScaled",
            "type": "u128"
          },
          {
            "name": "pendingWithdrawalUnstakeTs",
            "type": "u64"
          },
          {
            "name": "bump",
            "type": "u64"
          },
          {
            "name": "delegatee",
            "type": "pubkey"
          },
          {
            "name": "lastStakeTs",
            "type": "u64"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u64",
                50
              ]
            }
          }
        ]
      }
    },
    {
      "name": "globalConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "globalAdmin",
            "docs": [
              "Global admin of the program"
            ],
            "type": "pubkey"
          },
          {
            "name": "pendingAdmin",
            "docs": [
              "Pending admin must sign a specific transaction to become the global admin"
            ],
            "type": "pubkey"
          },
          {
            "name": "feeCollector",
            "docs": [
              "Fee collector is the only allowed owner of token accounts receiving protocol fees"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding",
            "docs": [
              "Padding to make the struct size 1024 bytes"
            ],
            "type": {
              "array": [
                "u8",
                928
              ]
            }
          }
        ]
      }
    },
    {
      "name": "lendingMarket",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "docs": [
              "Version of lending market"
            ],
            "type": "u64"
          },
          {
            "name": "bumpSeed",
            "docs": [
              "Bump seed for derived authority address"
            ],
            "type": "u64"
          },
          {
            "name": "lendingMarketOwner",
            "docs": [
              "Owner authority which can add new reserves"
            ],
            "type": "pubkey"
          },
          {
            "name": "lendingMarketOwnerCached",
            "docs": [
              "Temporary cache of the lending market owner, used in update_lending_market_owner"
            ],
            "type": "pubkey"
          },
          {
            "name": "quoteCurrency",
            "docs": [
              "Currency market prices are quoted in",
              "e.g. \"USD\" null padded (`*b\"USD\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\"`) or a SPL token mint pubkey"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "referralFeeBps",
            "docs": [
              "Referral fee for the lending market, as bps out of the total protocol fee"
            ],
            "type": "u16"
          },
          {
            "name": "emergencyMode",
            "type": "u8"
          },
          {
            "name": "autodeleverageEnabled",
            "docs": [
              "Whether the obligations on this market should be subject to auto-deleveraging after deposit",
              "or borrow limit is crossed.",
              "Besides this flag, the particular reserve's flag also needs to be enabled (logical `AND`).",
              "**NOTE:** this also affects the individual \"target LTV\" deleveraging."
            ],
            "type": "u8"
          },
          {
            "name": "borrowDisabled",
            "type": "u8"
          },
          {
            "name": "priceRefreshTriggerToMaxAgePct",
            "docs": [
              "Refresh price from oracle only if it's older than this percentage of the price max age.",
              "e.g. if the max age is set to 100s and this is set to 80%, the price will be refreshed if it's older than 80s.",
              "Price is always refreshed if this set to 0."
            ],
            "type": "u8"
          },
          {
            "name": "liquidationMaxDebtCloseFactorPct",
            "docs": [
              "Percentage of the total borrowed value in an obligation available for liquidation"
            ],
            "type": "u8"
          },
          {
            "name": "insolvencyRiskUnhealthyLtvPct",
            "docs": [
              "Minimum acceptable unhealthy LTV before max_debt_close_factor_pct becomes 100%"
            ],
            "type": "u8"
          },
          {
            "name": "minFullLiquidationValueThreshold",
            "docs": [
              "Minimum liquidation value threshold triggering full liquidation for an obligation, in full",
              "units of the quote currency (e.g. `2` means \"$2\", not \"2 lamports of USDC\")."
            ],
            "type": "u64"
          },
          {
            "name": "maxLiquidatableDebtMarketValueAtOnce",
            "docs": [
              "Max allowed liquidation value in one ix call"
            ],
            "type": "u64"
          },
          {
            "name": "reserved0",
            "docs": [
              "[DEPRECATED] Global maximum unhealthy borrow value allowed for any obligation"
            ],
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "globalAllowedBorrowValue",
            "docs": [
              "Global maximum allowed borrow value allowed for any obligation"
            ],
            "type": "u64"
          },
          {
            "name": "emergencyCouncil",
            "docs": [
              "The address of the emergency council, in charge of taking emergency actions on the market (e.g., enabling emergency mode)"
            ],
            "type": "pubkey"
          },
          {
            "name": "reserved1",
            "docs": [
              "[DEPRECATED] Reward points multiplier per obligation type"
            ],
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "elevationGroups",
            "docs": [
              "Elevation groups are used to group together reserves that have the same risk parameters and can bump the ltv and liquidation threshold"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "elevationGroup"
                  }
                },
                32
              ]
            }
          },
          {
            "name": "elevationGroupPadding",
            "type": {
              "array": [
                "u64",
                90
              ]
            }
          },
          {
            "name": "minNetValueInObligationSf",
            "docs": [
              "Min net value accepted to be found in a position after any lending action in an obligation (scaled by quote currency decimals)"
            ],
            "type": "u128"
          },
          {
            "name": "minValueSkipLiquidationLtvChecks",
            "docs": [
              "Minimum value to enforce smallest ltv priority checks on the collateral reserves on liquidation"
            ],
            "type": "u64"
          },
          {
            "name": "name",
            "docs": [
              "Market name, zero-padded."
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "minValueSkipLiquidationBfChecks",
            "docs": [
              "Minimum value to enforce highest borrow factor priority checks on the debt reserves on liquidation"
            ],
            "type": "u64"
          },
          {
            "name": "individualAutodeleverageMarginCallPeriodSecs",
            "docs": [
              "Time (in seconds) that must pass before liquidation is allowed on an obligation that has",
              "been individually marked for auto-deleveraging."
            ],
            "type": "u64"
          },
          {
            "name": "minInitialDepositAmount",
            "docs": [
              "Minimum amount of deposit at creation of a reserve to prevent artificial inflation",
              "Note: this amount cannot be recovered, the ctoken associated are never minted"
            ],
            "type": "u64"
          },
          {
            "name": "obligationOrderExecutionEnabled",
            "docs": [
              "Whether the obligation orders should be evaluated during liquidations."
            ],
            "type": "u8"
          },
          {
            "name": "immutable",
            "docs": [
              "Whether the lending market is set as immutable."
            ],
            "type": "u8"
          },
          {
            "name": "obligationOrderCreationEnabled",
            "docs": [
              "Whether new obligation orders can be created.",
              "Note: updating or cancelling existing orders is *not* affected by this flag."
            ],
            "type": "u8"
          },
          {
            "name": "priceTriggeredLiquidationDisabled",
            "docs": [
              "Whether the liquidation operations that are triggered by price changes should be disabled.",
              "This includes regular liquidation (i.e. LTV exceeding the unhealthy threshold) and some",
              "obligation orders' execution.",
              "",
              "*Caution:* this flag is *disabling* the liquidations when `1` - contrary to all the other",
              "liquidation-driving flags (see e.g. [Self::autodeleverage_enabled])."
            ],
            "type": "u8"
          },
          {
            "name": "matureReserveDebtLiquidationEnabled",
            "docs": [
              "Whether the debts that reached their reserve's [ReserveConfig::debt_maturity_timestamp] can",
              "be liquidated."
            ],
            "type": "u8"
          },
          {
            "name": "obligationBorrowDebtTermLiquidationEnabled",
            "docs": [
              "Whether the [Obligation::borrows] that reached their [ReserveConfig::debt_term_seconds] can",
              "be liquidated."
            ],
            "type": "u8"
          },
          {
            "name": "borrowOrderCreationEnabled",
            "docs": [
              "Whether new borrow orders can be created.",
              "Note: updating or cancelling existing orders is *not* affected by this flag."
            ],
            "type": "u8"
          },
          {
            "name": "borrowOrderExecutionEnabled",
            "docs": [
              "Whether the existing borrow orders can be filled."
            ],
            "type": "u8"
          },
          {
            "name": "proposerAuthority",
            "docs": [
              "Authority that can propose creating of new reserves but cannot enable them."
            ],
            "type": "pubkey"
          },
          {
            "name": "minBorrowOrderFillValue",
            "docs": [
              "Minimum value that can be filled in a single `fill_borrow_order()` call, in full units of",
              "the quote currency (e.g. `2` means \"$2\", not \"2 lamports of USDC\")."
            ],
            "type": "u64"
          },
          {
            "name": "withdrawTicketIssuanceEnabled",
            "docs": [
              "Whether any new withdraw tickets can be issued (i.e. whether new requests can enter the",
              "withdraw queue)."
            ],
            "type": "u8"
          },
          {
            "name": "withdrawTicketRedemptionEnabled",
            "docs": [
              "Whether the existing withdraw tickets can be redeemed (i.e. whether the tickets can be used",
              "to transfer accumulated pending liquidity to destination accounts)."
            ],
            "type": "u8"
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "minWithdrawQueuedLiquidityValue",
            "docs": [
              "Minimum value that can be withdrawn in a single `withdraw_queued_liquidity()` call, in full",
              "units of the quote currency (e.g. `2` means \"$2\", not \"2 lamports of USDC\")."
            ],
            "type": "u64"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u64",
                162
              ]
            }
          }
        ]
      }
    },
    {
      "name": "obligation",
      "docs": [
        "Lending market obligation state"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "tag",
            "docs": [
              "Version of the struct"
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdate",
            "docs": [
              "Last update to collateral, liquidity, or their market values"
            ],
            "type": {
              "defined": {
                "name": "lastUpdate"
              }
            }
          },
          {
            "name": "lendingMarket",
            "docs": [
              "Lending market address"
            ],
            "type": "pubkey"
          },
          {
            "name": "owner",
            "docs": [
              "Owner authority which can borrow liquidity"
            ],
            "type": "pubkey"
          },
          {
            "name": "deposits",
            "docs": [
              "Deposited collateral for the obligation, unique by deposit reserve address"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "obligationCollateral"
                  }
                },
                8
              ]
            }
          },
          {
            "name": "lowestReserveDepositLiquidationLtv",
            "docs": [
              "Worst LTV for the collaterals backing the loan, represented as a percentage"
            ],
            "type": "u64"
          },
          {
            "name": "depositedValueSf",
            "docs": [
              "Market value of deposits (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "borrows",
            "docs": [
              "Borrowed liquidity for the obligation, unique by borrow reserve address"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "obligationLiquidity"
                  }
                },
                5
              ]
            }
          },
          {
            "name": "borrowFactorAdjustedDebtValueSf",
            "docs": [
              "Risk adjusted market value of borrows/debt (sum of price * borrowed_amount * borrow_factor) (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "borrowedAssetsMarketValueSf",
            "docs": [
              "Market value of borrows - used for max_liquidatable_borrowed_amount (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "allowedBorrowValueSf",
            "docs": [
              "The maximum borrow value at the weighted average loan to value ratio (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "unhealthyBorrowValueSf",
            "docs": [
              "The dangerous borrow value at the weighted average liquidation threshold (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "paddingDeprecatedAssetTiers",
            "docs": [
              "The asset tier of the deposits"
            ],
            "type": {
              "array": [
                "u8",
                13
              ]
            }
          },
          {
            "name": "elevationGroup",
            "docs": [
              "The elevation group id the obligation opted into."
            ],
            "type": "u8"
          },
          {
            "name": "numOfObsoleteDepositReserves",
            "docs": [
              "The number of obsolete reserves the obligation has a deposit in"
            ],
            "type": "u8"
          },
          {
            "name": "hasDebt",
            "docs": [
              "Marked = 1 if borrows array is not empty, 0 = borrows empty"
            ],
            "type": "u8"
          },
          {
            "name": "referrer",
            "docs": [
              "Wallet address of the referrer"
            ],
            "type": "pubkey"
          },
          {
            "name": "borrowingDisabled",
            "docs": [
              "Marked = 1 if borrowing disabled, 0 = borrowing enabled"
            ],
            "type": "u8"
          },
          {
            "name": "autodeleverageTargetLtvPct",
            "docs": [
              "A target LTV set by the market owner when marking this obligation for deleveraging.",
              "Only effective when `deleveraging_margin_call_started_slot != 0`."
            ],
            "type": "u8"
          },
          {
            "name": "lowestReserveDepositMaxLtvPct",
            "docs": [
              "The lowest max LTV found amongst the collateral deposits"
            ],
            "type": "u8"
          },
          {
            "name": "numOfObsoleteBorrowReserves",
            "docs": [
              "The number of obsolete reserves the obligation has a borrow in"
            ],
            "type": "u8"
          },
          {
            "name": "reserved",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          },
          {
            "name": "highestBorrowFactorPct",
            "type": "u64"
          },
          {
            "name": "autodeleverageMarginCallStartedTimestamp",
            "docs": [
              "A timestamp at which the market owner most-recently marked this obligation for deleveraging.",
              "Zero if not currently subject to deleveraging."
            ],
            "type": "u64"
          },
          {
            "name": "obligationOrders",
            "docs": [
              "Owner-defined, permissionlessly-executed repay orders.",
              "Typical use-cases would be a stop-loss and a take-profit (possibly co-existing)."
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "obligationOrder"
                  }
                },
                2
              ]
            }
          },
          {
            "name": "borrowOrder",
            "docs": [
              "Owner-defined, permissionlessly-executed borrow order applicable to this obligation.",
              "Non-zeroed only on a newly-initialized fixed-rate, fixed-term obligation."
            ],
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          },
          {
            "name": "padding3",
            "type": {
              "array": [
                "u64",
                73
              ]
            }
          }
        ]
      }
    },
    {
      "name": "referrerState",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "shortUrl",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "referrerTokenState",
      "docs": [
        "Referrer account -> each owner can have multiple accounts for specific reserves"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "referrer",
            "docs": [
              "Pubkey of the referrer/owner"
            ],
            "type": "pubkey"
          },
          {
            "name": "mint",
            "docs": [
              "Token mint for the account"
            ],
            "type": "pubkey"
          },
          {
            "name": "amountUnclaimedSf",
            "docs": [
              "Amount that has been accumulated and not claimed yet -> available to claim (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "amountCumulativeSf",
            "docs": [
              "Amount that has been accumulated in total -> both already claimed and unclaimed (scaled fraction)"
            ],
            "type": "u128"
          },
          {
            "name": "bump",
            "docs": [
              "Referrer token state bump, used for address validation"
            ],
            "type": "u64"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                31
              ]
            }
          }
        ]
      }
    },
    {
      "name": "shortUrl",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "referrer",
            "type": "pubkey"
          },
          {
            "name": "shortUrl",
            "type": "string"
          }
        ]
      }
    },
    {
      "name": "userMetadata",
      "docs": [
        "Referrer account -> each owner can have multiple accounts for specific reserves"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "referrer",
            "docs": [
              "Pubkey of the referrer/owner - pubkey::default if no referrer"
            ],
            "type": "pubkey"
          },
          {
            "name": "bump",
            "docs": [
              "Bump used for validation of account address"
            ],
            "type": "u64"
          },
          {
            "name": "userLookupTable",
            "docs": [
              "User lookup table - used to store all user accounts - atas for each reserve mint, each obligation PDA, UserMetadata itself and all referrer_token_states if there is a referrer"
            ],
            "type": "pubkey"
          },
          {
            "name": "owner",
            "docs": [
              "User metadata account owner"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u64",
                51
              ]
            }
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u64",
                64
              ]
            }
          }
        ]
      }
    },
    {
      "name": "reserve",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "docs": [
              "Version of the reserve"
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdate",
            "docs": [
              "Last slot when supply and rates updated"
            ],
            "type": {
              "defined": {
                "name": "lastUpdate"
              }
            }
          },
          {
            "name": "lendingMarket",
            "docs": [
              "Lending market address"
            ],
            "type": "pubkey"
          },
          {
            "name": "farmCollateral",
            "type": "pubkey"
          },
          {
            "name": "farmDebt",
            "type": "pubkey"
          },
          {
            "name": "liquidity",
            "docs": [
              "Reserve liquidity"
            ],
            "type": {
              "defined": {
                "name": "reserveLiquidity"
              }
            }
          },
          {
            "name": "reserveLiquidityPadding",
            "type": {
              "array": [
                "u64",
                150
              ]
            }
          },
          {
            "name": "collateral",
            "docs": [
              "Reserve collateral"
            ],
            "type": {
              "defined": {
                "name": "reserveCollateral"
              }
            }
          },
          {
            "name": "reserveCollateralPadding",
            "type": {
              "array": [
                "u64",
                150
              ]
            }
          },
          {
            "name": "config",
            "docs": [
              "Reserve configuration values"
            ],
            "type": {
              "defined": {
                "name": "reserveConfig"
              }
            }
          },
          {
            "name": "configPadding",
            "type": {
              "array": [
                "u64",
                114
              ]
            }
          },
          {
            "name": "borrowedAmountOutsideElevationGroup",
            "type": "u64"
          },
          {
            "name": "borrowedAmountsAgainstThisReserveInElevationGroups",
            "docs": [
              "Amount of token borrowed in lamport of debt asset in the given",
              "elevation group when this reserve is part of the collaterals."
            ],
            "type": {
              "array": [
                "u64",
                32
              ]
            }
          },
          {
            "name": "withdrawQueue",
            "docs": [
              "The tracker of ticket-based withdrawals."
            ],
            "type": {
              "defined": {
                "name": "withdrawQueue"
              }
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                204
              ]
            }
          }
        ]
      }
    },
    {
      "name": "withdrawTicket",
      "docs": [
        "A finite-lifecycle account representing a specific depositor's place in the withdraw queue of",
        "a specific reserve.",
        "",
        "The lifecycle:",
        "1. The depositor holding ctokens wants to withdraw funds from the reserve, and finds out that",
        "the required amount is not available (due to high utilization).",
        "2. The depositor calls the `enqueue_to_withdraw` handler.",
        "3. The handler transfers the depositor's ctokens to the reserve's internal \"pending\" vault.",
        "4. The handler initializes a new [WithdrawTicket] account, with the next available sequence",
        "number.",
        "5. The depositor waits until his ticket is the next expected one for actual withdraw, and until",
        "the reserve has enough liquidity.",
        "6. Anyone (the depositor or a bot) calls the permissionless `withdraw_queued_liquidity`",
        "handler. If the ticket became invalid (e.g. destination account no longer exists), then the",
        "depositor can call the `recover_invalid_ticket_collateral` handler instead.",
        "7. The handler transfers the liquidity amount according to the current exchange rate.",
        "8. The handler closes the ticket account."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "sequenceNumber",
            "docs": [
              "This ticket's place in the queue; the same as used for PDA derivation."
            ],
            "type": "u64"
          },
          {
            "name": "owner",
            "docs": [
              "The funds' owner (the user who called the `enqueue_to_withdraw` handler)."
            ],
            "type": "pubkey"
          },
          {
            "name": "reserve",
            "docs": [
              "The reserve to withdraw from."
            ],
            "type": "pubkey"
          },
          {
            "name": "userDestinationLiquidityTa",
            "docs": [
              "The token account to which the finally-available liquidity should be transferred (by the",
              "`withdraw_queued_liquidity` handler)."
            ],
            "type": "pubkey"
          },
          {
            "name": "queuedCollateralAmount",
            "docs": [
              "The amount of collateral still waiting to be withdrawn using this ticket."
            ],
            "type": "u64"
          },
          {
            "name": "createdAtTimestamp",
            "docs": [
              "The timestamp at which the queue was entered.",
              "",
              "This is currently only a piece of metadata, not used by the logic."
            ],
            "type": "u64"
          },
          {
            "name": "invalid",
            "docs": [
              "Whether the ticket has been found to be invalid (e.g. the [Self::user_destination_liquidity]",
              "has been repurposed) by the `withdraw_queued_liquidity` handler.",
              "To be specific: valid = `0`, invalid = `1`.",
              "",
              "An invalid ticket cannot be made valid again, and can only be passed to the",
              "`recover_invalid_ticket_collateral` handler."
            ],
            "type": "u8"
          },
          {
            "name": "alignmentPadding",
            "docs": [
              "Inner padding, for alignment."
            ],
            "type": {
              "array": [
                "u8",
                7
              ]
            }
          },
          {
            "name": "endPadding",
            "docs": [
              "Trailing padding, for future developments."
            ],
            "type": {
              "array": [
                "u64",
                48
              ]
            }
          }
        ]
      }
    },
    {
      "name": "borrowOrderCancelEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "before",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          }
        ]
      }
    },
    {
      "name": "borrowOrderFullFillEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "before",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          }
        ]
      }
    },
    {
      "name": "borrowOrderPartialFillEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "before",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          },
          {
            "name": "after",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          }
        ]
      }
    },
    {
      "name": "borrowOrderPlaceEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "after",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          }
        ]
      }
    },
    {
      "name": "borrowOrderUpdateEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "before",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          },
          {
            "name": "after",
            "type": {
              "defined": {
                "name": "borrowOrder"
              }
            }
          }
        ]
      }
    }
  ]
};

