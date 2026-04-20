/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/drift.json`.
 */
export type Drift = {
  "address": "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH",
  "metadata": {
    "name": "drift",
    "version": "2.114.0",
    "spec": "0.1.0"
  },
  "instructions": [
    {
      "name": "initializeUser",
      "discriminator": [
        111,
        17,
        185,
        250,
        60,
        122,
        38,
        254
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
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
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "name",
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
      "name": "initializeUserStats",
      "discriminator": [
        254,
        243,
        72,
        98,
        251,
        130,
        168,
        213
      ],
      "accounts": [
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
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
      "name": "initializeSignedMsgUserOrders",
      "discriminator": [
        164,
        99,
        156,
        126,
        156,
        57,
        99,
        180
      ],
      "accounts": [
        {
          "name": "signedMsgUserOrders",
          "writable": true
        },
        {
          "name": "authority"
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
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
          "name": "numOrders",
          "type": "u16"
        }
      ]
    },
    {
      "name": "resizeSignedMsgUserOrders",
      "discriminator": [
        137,
        10,
        87,
        150,
        18,
        115,
        79,
        168
      ],
      "accounts": [
        {
          "name": "signedMsgUserOrders",
          "writable": true
        },
        {
          "name": "authority"
        },
        {
          "name": "user"
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "numOrders",
          "type": "u16"
        }
      ]
    },
    {
      "name": "initializeSignedMsgWsDelegates",
      "discriminator": [
        40,
        132,
        96,
        219,
        184,
        193,
        80,
        8
      ],
      "accounts": [
        {
          "name": "signedMsgWsDelegates",
          "writable": true
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
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
          "name": "delegates",
          "type": {
            "vec": "pubkey"
          }
        }
      ]
    },
    {
      "name": "changeSignedMsgWsDelegateStatus",
      "discriminator": [
        252,
        202,
        252,
        219,
        179,
        27,
        84,
        138
      ],
      "accounts": [
        {
          "name": "signedMsgWsDelegates",
          "writable": true
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram"
        }
      ],
      "args": [
        {
          "name": "delegate",
          "type": "pubkey"
        },
        {
          "name": "add",
          "type": "bool"
        }
      ]
    },
    {
      "name": "initializeFuelOverflow",
      "discriminator": [
        88,
        223,
        132,
        161,
        208,
        88,
        142,
        42
      ],
      "accounts": [
        {
          "name": "fuelOverflow",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority"
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
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
      "name": "sweepFuel",
      "discriminator": [
        175,
        107,
        19,
        56,
        165,
        241,
        43,
        69
      ],
      "accounts": [
        {
          "name": "fuelOverflow",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority"
        },
        {
          "name": "signer",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "resetFuelSeason",
      "discriminator": [
        199,
        122,
        192,
        255,
        32,
        99,
        63,
        200
      ],
      "accounts": [
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority"
        },
        {
          "name": "state"
        },
        {
          "name": "admin",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "initializeReferrerName",
      "discriminator": [
        235,
        126,
        231,
        10,
        42,
        164,
        26,
        61
      ],
      "accounts": [
        {
          "name": "referrerName",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
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
          "name": "name",
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
      "name": "deposit",
      "discriminator": [
        242,
        35,
        198,
        137,
        82,
        225,
        242,
        182
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "userTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "reduceOnly",
          "type": "bool"
        }
      ]
    },
    {
      "name": "withdraw",
      "discriminator": [
        183,
        18,
        70,
        156,
        148,
        109,
        161,
        34
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "userTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "reduceOnly",
          "type": "bool"
        }
      ]
    },
    {
      "name": "transferDeposit",
      "discriminator": [
        20,
        20,
        147,
        223,
        41,
        63,
        204,
        111
      ],
      "accounts": [
        {
          "name": "fromUser",
          "writable": true
        },
        {
          "name": "toUser",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarketVault"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "amount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "transferPools",
      "discriminator": [
        197,
        103,
        154,
        25,
        107,
        90,
        60,
        94
      ],
      "accounts": [
        {
          "name": "fromUser",
          "writable": true
        },
        {
          "name": "toUser",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "depositFromSpotMarketVault",
          "writable": true
        },
        {
          "name": "depositToSpotMarketVault",
          "writable": true
        },
        {
          "name": "borrowFromSpotMarketVault",
          "writable": true
        },
        {
          "name": "borrowToSpotMarketVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        }
      ],
      "args": [
        {
          "name": "depositFromMarketIndex",
          "type": "u16"
        },
        {
          "name": "depositToMarketIndex",
          "type": "u16"
        },
        {
          "name": "borrowFromMarketIndex",
          "type": "u16"
        },
        {
          "name": "borrowToMarketIndex",
          "type": "u16"
        },
        {
          "name": "depositAmount",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "borrowAmount",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "transferPerpPosition",
      "discriminator": [
        23,
        172,
        188,
        168,
        134,
        210,
        3,
        108
      ],
      "accounts": [
        {
          "name": "fromUser",
          "writable": true
        },
        {
          "name": "toUser",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "amount",
          "type": {
            "option": "i64"
          }
        }
      ]
    },
    {
      "name": "placePerpOrder",
      "discriminator": [
        69,
        161,
        93,
        202,
        120,
        126,
        76,
        185
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        }
      ]
    },
    {
      "name": "cancelOrder",
      "discriminator": [
        95,
        129,
        237,
        240,
        8,
        49,
        223,
        132
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "orderId",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "cancelOrderByUserId",
      "discriminator": [
        107,
        211,
        250,
        133,
        18,
        37,
        57,
        100
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "userOrderId",
          "type": "u8"
        }
      ]
    },
    {
      "name": "cancelOrders",
      "discriminator": [
        238,
        225,
        95,
        158,
        227,
        103,
        8,
        194
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "marketType",
          "type": {
            "option": {
              "defined": {
                "name": "marketType"
              }
            }
          }
        },
        {
          "name": "marketIndex",
          "type": {
            "option": "u16"
          }
        },
        {
          "name": "direction",
          "type": {
            "option": {
              "defined": {
                "name": "positionDirection"
              }
            }
          }
        }
      ]
    },
    {
      "name": "cancelOrdersByIds",
      "discriminator": [
        134,
        19,
        144,
        165,
        94,
        240,
        210,
        94
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "orderIds",
          "type": {
            "vec": "u32"
          }
        }
      ]
    },
    {
      "name": "modifyOrder",
      "discriminator": [
        47,
        124,
        117,
        255,
        201,
        197,
        130,
        94
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "orderId",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "modifyOrderParams",
          "type": {
            "defined": {
              "name": "modifyOrderParams"
            }
          }
        }
      ]
    },
    {
      "name": "modifyOrderByUserId",
      "discriminator": [
        158,
        77,
        4,
        253,
        252,
        194,
        161,
        179
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "userOrderId",
          "type": "u8"
        },
        {
          "name": "modifyOrderParams",
          "type": {
            "defined": {
              "name": "modifyOrderParams"
            }
          }
        }
      ]
    },
    {
      "name": "placeAndTakePerpOrder",
      "discriminator": [
        213,
        51,
        1,
        187,
        108,
        220,
        230,
        224
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        },
        {
          "name": "successCondition",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "placeAndMakePerpOrder",
      "discriminator": [
        149,
        117,
        11,
        237,
        47,
        95,
        89,
        237
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "taker",
          "writable": true
        },
        {
          "name": "takerStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        },
        {
          "name": "takerOrderId",
          "type": "u32"
        }
      ]
    },
    {
      "name": "placeAndMakeSignedMsgPerpOrder",
      "discriminator": [
        16,
        26,
        123,
        131,
        94,
        29,
        175,
        98
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "taker",
          "writable": true
        },
        {
          "name": "takerStats",
          "writable": true
        },
        {
          "name": "takerSignedMsgUserOrders"
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        },
        {
          "name": "signedMsgOrderUuid",
          "type": {
            "array": [
              "u8",
              8
            ]
          }
        }
      ]
    },
    {
      "name": "placeSignedMsgTakerOrder",
      "discriminator": [
        32,
        79,
        101,
        139,
        25,
        6,
        98,
        15
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "signedMsgUserOrders",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "ixSysvar",
          "docs": [
            "the supplied Sysvar could be anything else.",
            "The Instruction Sysvar has not been implemented",
            "in the Anchor framework yet, so this is the safe approach."
          ]
        }
      ],
      "args": [
        {
          "name": "signedMsgOrderParamsMessageBytes",
          "type": "bytes"
        },
        {
          "name": "isDelegateSigner",
          "type": "bool"
        }
      ]
    },
    {
      "name": "placeSpotOrder",
      "discriminator": [
        45,
        79,
        81,
        160,
        248,
        90,
        91,
        220
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        }
      ]
    },
    {
      "name": "placeAndTakeSpotOrder",
      "discriminator": [
        191,
        3,
        138,
        71,
        114,
        198,
        202,
        100
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        },
        {
          "name": "fulfillmentType",
          "type": {
            "option": {
              "defined": {
                "name": "spotFulfillmentType"
              }
            }
          }
        },
        {
          "name": "makerOrderId",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "placeAndMakeSpotOrder",
      "discriminator": [
        149,
        158,
        85,
        66,
        239,
        9,
        243,
        98
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "taker",
          "writable": true
        },
        {
          "name": "takerStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "orderParams"
            }
          }
        },
        {
          "name": "takerOrderId",
          "type": "u32"
        },
        {
          "name": "fulfillmentType",
          "type": {
            "option": {
              "defined": {
                "name": "spotFulfillmentType"
              }
            }
          }
        }
      ]
    },
    {
      "name": "placeOrders",
      "discriminator": [
        60,
        63,
        50,
        123,
        12,
        197,
        60,
        190
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "vec": {
              "defined": {
                "name": "orderParams"
              }
            }
          }
        }
      ]
    },
    {
      "name": "beginSwap",
      "discriminator": [
        174,
        109,
        228,
        1,
        242,
        105,
        232,
        105
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "outSpotMarketVault",
          "writable": true
        },
        {
          "name": "inSpotMarketVault",
          "writable": true
        },
        {
          "name": "outTokenAccount",
          "writable": true
        },
        {
          "name": "inTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "instructions",
          "docs": [
            "Instructions Sysvar for instruction introspection"
          ]
        }
      ],
      "args": [
        {
          "name": "inMarketIndex",
          "type": "u16"
        },
        {
          "name": "outMarketIndex",
          "type": "u16"
        },
        {
          "name": "amountIn",
          "type": "u64"
        }
      ]
    },
    {
      "name": "endSwap",
      "discriminator": [
        177,
        184,
        27,
        193,
        34,
        13,
        210,
        145
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "outSpotMarketVault",
          "writable": true
        },
        {
          "name": "inSpotMarketVault",
          "writable": true
        },
        {
          "name": "outTokenAccount",
          "writable": true
        },
        {
          "name": "inTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "instructions",
          "docs": [
            "Instructions Sysvar for instruction introspection"
          ]
        }
      ],
      "args": [
        {
          "name": "inMarketIndex",
          "type": "u16"
        },
        {
          "name": "outMarketIndex",
          "type": "u16"
        },
        {
          "name": "limitPrice",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "reduceOnly",
          "type": {
            "option": {
              "defined": {
                "name": "swapReduceOnly"
              }
            }
          }
        }
      ]
    },
    {
      "name": "addPerpLpShares",
      "discriminator": [
        56,
        209,
        56,
        197,
        119,
        254,
        188,
        117
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "nShares",
          "type": "u64"
        },
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "removePerpLpShares",
      "discriminator": [
        213,
        89,
        217,
        18,
        160,
        55,
        53,
        141
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "sharesToBurn",
          "type": "u64"
        },
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "removePerpLpSharesInExpiringMarket",
      "discriminator": [
        83,
        254,
        253,
        137,
        59,
        122,
        68,
        156
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "sharesToBurn",
          "type": "u64"
        },
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateUserName",
      "discriminator": [
        135,
        25,
        185,
        56,
        165,
        53,
        34,
        136
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "name",
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
      "name": "updateUserCustomMarginRatio",
      "discriminator": [
        21,
        221,
        140,
        187,
        32,
        129,
        11,
        123
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "marginRatio",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateUserMarginTradingEnabled",
      "discriminator": [
        194,
        92,
        204,
        223,
        246,
        188,
        31,
        203
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "marginTradingEnabled",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updateUserPoolId",
      "discriminator": [
        219,
        86,
        73,
        106,
        56,
        218,
        128,
        109
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "poolId",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateUserDelegate",
      "discriminator": [
        139,
        205,
        141,
        141,
        113,
        36,
        94,
        187
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "delegate",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "updateUserReduceOnly",
      "discriminator": [
        199,
        71,
        42,
        67,
        144,
        19,
        86,
        109
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "reduceOnly",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updateUserAdvancedLp",
      "discriminator": [
        66,
        80,
        107,
        186,
        27,
        242,
        66,
        95
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "advancedLp",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updateUserProtectedMakerOrders",
      "discriminator": [
        114,
        39,
        123,
        198,
        187,
        25,
        90,
        219
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "protectedMakerModeConfig",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        },
        {
          "name": "protectedMakerOrders",
          "type": "bool"
        }
      ]
    },
    {
      "name": "deleteUser",
      "discriminator": [
        186,
        85,
        17,
        249,
        219,
        231,
        98,
        251
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "forceDeleteUser",
      "discriminator": [
        2,
        241,
        195,
        172,
        227,
        24,
        254,
        158
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "authority",
          "writable": true
        },
        {
          "name": "keeper",
          "writable": true,
          "signer": true
        },
        {
          "name": "driftSigner"
        }
      ],
      "args": []
    },
    {
      "name": "deleteSignedMsgUserOrders",
      "discriminator": [
        221,
        247,
        128,
        253,
        212,
        254,
        46,
        153
      ],
      "accounts": [
        {
          "name": "signedMsgUserOrders",
          "writable": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "reclaimRent",
      "discriminator": [
        218,
        200,
        19,
        197,
        227,
        89,
        192,
        22
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "rent"
        }
      ],
      "args": []
    },
    {
      "name": "enableUserHighLeverageMode",
      "discriminator": [
        231,
        24,
        230,
        112,
        201,
        173,
        73,
        184
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "highLeverageModeConfig",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "subAccountId",
          "type": "u16"
        }
      ]
    },
    {
      "name": "fillPerpOrder",
      "discriminator": [
        13,
        188,
        248,
        103,
        134,
        217,
        106,
        240
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "fillerStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "orderId",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "makerOrderId",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "revertFill",
      "discriminator": [
        236,
        238,
        176,
        69,
        239,
        10,
        181,
        193
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "fillerStats",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "fillSpotOrder",
      "discriminator": [
        212,
        206,
        130,
        173,
        21,
        34,
        199,
        40
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "fillerStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "orderId",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "fulfillmentType",
          "type": {
            "option": {
              "defined": {
                "name": "spotFulfillmentType"
              }
            }
          }
        },
        {
          "name": "makerOrderId",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "triggerOrder",
      "discriminator": [
        63,
        112,
        51,
        233,
        232,
        47,
        240,
        199
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "orderId",
          "type": "u32"
        }
      ]
    },
    {
      "name": "forceCancelOrders",
      "discriminator": [
        64,
        181,
        196,
        63,
        222,
        72,
        64,
        232
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updateUserIdle",
      "discriminator": [
        253,
        133,
        67,
        22,
        103,
        161,
        20,
        100
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "logUserBalances",
      "discriminator": [
        162,
        21,
        35,
        251,
        32,
        57,
        161,
        210
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "disableUserHighLeverageMode",
      "discriminator": [
        183,
        155,
        45,
        0,
        226,
        85,
        213,
        69
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "highLeverageModeConfig",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updateUserFuelBonus",
      "discriminator": [
        88,
        175,
        201,
        190,
        222,
        100,
        143,
        57
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updateUserStatsReferrerStatus",
      "discriminator": [
        174,
        154,
        72,
        42,
        191,
        148,
        145,
        205
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updateUserOpenOrdersCount",
      "discriminator": [
        104,
        39,
        65,
        210,
        250,
        163,
        100,
        134
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "filler",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "adminDisableUpdatePerpBidAskTwap",
      "discriminator": [
        17,
        164,
        82,
        45,
        183,
        86,
        191,
        199
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "disable",
          "type": "bool"
        }
      ]
    },
    {
      "name": "settlePnl",
      "discriminator": [
        43,
        61,
        234,
        45,
        15,
        95,
        152,
        153
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "spotMarketVault"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "settleMultiplePnls",
      "discriminator": [
        127,
        66,
        117,
        57,
        40,
        50,
        152,
        127
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "spotMarketVault"
        }
      ],
      "args": [
        {
          "name": "marketIndexes",
          "type": {
            "vec": "u16"
          }
        },
        {
          "name": "mode",
          "type": {
            "defined": {
              "name": "settlePnlMode"
            }
          }
        }
      ]
    },
    {
      "name": "settleFundingPayment",
      "discriminator": [
        222,
        90,
        202,
        94,
        28,
        45,
        115,
        183
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "settleLp",
      "discriminator": [
        155,
        231,
        116,
        113,
        97,
        229,
        139,
        141
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "settleExpiredMarket",
      "discriminator": [
        120,
        89,
        11,
        25,
        122,
        77,
        72,
        193
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "liquidatePerp",
      "discriminator": [
        75,
        35,
        119,
        247,
        191,
        18,
        139,
        2
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "liquidatorMaxBaseAssetAmount",
          "type": "u64"
        },
        {
          "name": "limitPrice",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "liquidatePerpWithFill",
      "discriminator": [
        95,
        111,
        124,
        105,
        86,
        169,
        187,
        34
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "liquidateSpot",
      "discriminator": [
        107,
        0,
        128,
        41,
        35,
        229,
        251,
        18
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "assetMarketIndex",
          "type": "u16"
        },
        {
          "name": "liabilityMarketIndex",
          "type": "u16"
        },
        {
          "name": "liquidatorMaxLiabilityTransfer",
          "type": "u128"
        },
        {
          "name": "limitPrice",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "liquidateSpotWithSwapBegin",
      "discriminator": [
        12,
        43,
        176,
        83,
        156,
        251,
        117,
        13
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "liabilitySpotMarketVault",
          "writable": true
        },
        {
          "name": "assetSpotMarketVault",
          "writable": true
        },
        {
          "name": "liabilityTokenAccount",
          "writable": true
        },
        {
          "name": "assetTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "instructions",
          "docs": [
            "Instructions Sysvar for instruction introspection"
          ]
        }
      ],
      "args": [
        {
          "name": "assetMarketIndex",
          "type": "u16"
        },
        {
          "name": "liabilityMarketIndex",
          "type": "u16"
        },
        {
          "name": "swapAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "liquidateSpotWithSwapEnd",
      "discriminator": [
        142,
        88,
        163,
        160,
        223,
        75,
        55,
        225
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "liabilitySpotMarketVault",
          "writable": true
        },
        {
          "name": "assetSpotMarketVault",
          "writable": true
        },
        {
          "name": "liabilityTokenAccount",
          "writable": true
        },
        {
          "name": "assetTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "instructions",
          "docs": [
            "Instructions Sysvar for instruction introspection"
          ]
        }
      ],
      "args": [
        {
          "name": "assetMarketIndex",
          "type": "u16"
        },
        {
          "name": "liabilityMarketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "liquidateBorrowForPerpPnl",
      "discriminator": [
        169,
        17,
        32,
        90,
        207,
        148,
        209,
        27
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "perpMarketIndex",
          "type": "u16"
        },
        {
          "name": "spotMarketIndex",
          "type": "u16"
        },
        {
          "name": "liquidatorMaxLiabilityTransfer",
          "type": "u128"
        },
        {
          "name": "limitPrice",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "liquidatePerpPnlForDeposit",
      "discriminator": [
        237,
        75,
        198,
        235,
        233,
        186,
        75,
        35
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "perpMarketIndex",
          "type": "u16"
        },
        {
          "name": "spotMarketIndex",
          "type": "u16"
        },
        {
          "name": "liquidatorMaxPnlTransfer",
          "type": "u128"
        },
        {
          "name": "limitPrice",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "setUserStatusToBeingLiquidated",
      "discriminator": [
        106,
        133,
        160,
        206,
        193,
        171,
        192,
        194
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "resolvePerpPnlDeficit",
      "discriminator": [
        168,
        204,
        68,
        150,
        159,
        126,
        95,
        148
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "spotMarketIndex",
          "type": "u16"
        },
        {
          "name": "perpMarketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "resolvePerpBankruptcy",
      "discriminator": [
        224,
        16,
        176,
        214,
        162,
        213,
        183,
        222
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "quoteSpotMarketIndex",
          "type": "u16"
        },
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "resolveSpotBankruptcy",
      "discriminator": [
        124,
        194,
        240,
        254,
        198,
        213,
        52,
        122
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "liquidator",
          "writable": true
        },
        {
          "name": "liquidatorStats",
          "writable": true
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "settleRevenueToInsuranceFund",
      "discriminator": [
        200,
        120,
        93,
        136,
        69,
        38,
        199,
        159
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "spotMarketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateFundingRate",
      "discriminator": [
        201,
        178,
        116,
        212,
        166,
        144,
        72,
        238
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updatePrelaunchOracle",
      "discriminator": [
        220,
        132,
        27,
        27,
        233,
        220,
        61,
        219
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "perpMarket"
        },
        {
          "name": "oracle",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updatePerpBidAskTwap",
      "discriminator": [
        247,
        23,
        255,
        65,
        212,
        90,
        221,
        194
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "keeperStats"
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "updateSpotMarketCumulativeInterest",
      "discriminator": [
        39,
        166,
        139,
        243,
        158,
        165,
        155,
        225
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "spotMarketVault"
        }
      ],
      "args": []
    },
    {
      "name": "updateAmms",
      "discriminator": [
        201,
        106,
        217,
        253,
        4,
        175,
        228,
        97
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "marketIndexes",
          "type": {
            "array": [
              "u16",
              5
            ]
          }
        }
      ]
    },
    {
      "name": "updateSpotMarketExpiry",
      "discriminator": [
        208,
        11,
        211,
        159,
        226,
        24,
        11,
        247
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "expiryTs",
          "type": "i64"
        }
      ]
    },
    {
      "name": "updateUserQuoteAssetInsuranceStake",
      "discriminator": [
        251,
        101,
        156,
        7,
        2,
        63,
        30,
        23
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updateUserGovTokenInsuranceStake",
      "discriminator": [
        143,
        99,
        235,
        187,
        20,
        159,
        184,
        84
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "updateUserGovTokenInsuranceStakeDevnet",
      "discriminator": [
        129,
        185,
        243,
        183,
        228,
        111,
        64,
        175
      ],
      "accounts": [
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "signer",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "govStakeAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initializeInsuranceFundStake",
      "discriminator": [
        187,
        179,
        243,
        70,
        248,
        90,
        92,
        147
      ],
      "accounts": [
        {
          "name": "spotMarket"
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "state"
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
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
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "addInsuranceFundStake",
      "discriminator": [
        251,
        144,
        115,
        11,
        222,
        47,
        62,
        236
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "userTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "amount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "requestRemoveInsuranceFundStake",
      "discriminator": [
        142,
        70,
        204,
        92,
        73,
        106,
        180,
        52
      ],
      "accounts": [
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "amount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "cancelRequestRemoveInsuranceFundStake",
      "discriminator": [
        97,
        235,
        78,
        62,
        212,
        42,
        241,
        127
      ],
      "accounts": [
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "removeInsuranceFundStake",
      "discriminator": [
        128,
        166,
        142,
        9,
        254,
        187,
        143,
        174
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "userTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "transferProtocolIfShares",
      "discriminator": [
        94,
        93,
        226,
        240,
        195,
        201,
        184,
        109
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "transferConfig",
          "writable": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "insuranceFundStake",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "insuranceFundVault"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "shares",
          "type": "u128"
        }
      ]
    },
    {
      "name": "updatePythPullOracle",
      "discriminator": [
        230,
        191,
        189,
        94,
        108,
        59,
        74,
        197
      ],
      "accounts": [
        {
          "name": "keeper",
          "writable": true,
          "signer": true
        },
        {
          "name": "pythSolanaReceiver"
        },
        {
          "name": "encodedVaa"
        },
        {
          "name": "priceFeed",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feedId",
          "type": {
            "array": [
              "u8",
              32
            ]
          }
        },
        {
          "name": "params",
          "type": "bytes"
        }
      ]
    },
    {
      "name": "postPythPullOracleUpdateAtomic",
      "discriminator": [
        116,
        122,
        137,
        158,
        224,
        195,
        173,
        119
      ],
      "accounts": [
        {
          "name": "keeper",
          "writable": true,
          "signer": true
        },
        {
          "name": "pythSolanaReceiver"
        },
        {
          "name": "guardianSet"
        },
        {
          "name": "priceFeed",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feedId",
          "type": {
            "array": [
              "u8",
              32
            ]
          }
        },
        {
          "name": "params",
          "type": "bytes"
        }
      ]
    },
    {
      "name": "postMultiPythPullOracleUpdatesAtomic",
      "discriminator": [
        243,
        79,
        204,
        228,
        227,
        208,
        100,
        244
      ],
      "accounts": [
        {
          "name": "keeper",
          "writable": true,
          "signer": true
        },
        {
          "name": "pythSolanaReceiver"
        },
        {
          "name": "guardianSet"
        }
      ],
      "args": [
        {
          "name": "params",
          "type": "bytes"
        }
      ]
    },
    {
      "name": "pauseSpotMarketDepositWithdraw",
      "discriminator": [
        183,
        119,
        59,
        170,
        137,
        35,
        242,
        86
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "keeper",
          "signer": true
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "spotMarketVault"
        }
      ],
      "args": []
    },
    {
      "name": "initialize",
      "discriminator": [
        175,
        175,
        109,
        31,
        13,
        152,
        155,
        237
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "quoteAssetMint"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "initializeSpotMarket",
      "discriminator": [
        234,
        196,
        128,
        44,
        94,
        15,
        48,
        201
      ],
      "accounts": [
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "spotMarketMint"
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "rent"
        },
        {
          "name": "systemProgram"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "optimalUtilization",
          "type": "u32"
        },
        {
          "name": "optimalBorrowRate",
          "type": "u32"
        },
        {
          "name": "maxBorrowRate",
          "type": "u32"
        },
        {
          "name": "oracleSource",
          "type": {
            "defined": {
              "name": "oracleSource"
            }
          }
        },
        {
          "name": "initialAssetWeight",
          "type": "u32"
        },
        {
          "name": "maintenanceAssetWeight",
          "type": "u32"
        },
        {
          "name": "initialLiabilityWeight",
          "type": "u32"
        },
        {
          "name": "maintenanceLiabilityWeight",
          "type": "u32"
        },
        {
          "name": "imfFactor",
          "type": "u32"
        },
        {
          "name": "liquidatorFee",
          "type": "u32"
        },
        {
          "name": "ifLiquidationFee",
          "type": "u32"
        },
        {
          "name": "activeStatus",
          "type": "bool"
        },
        {
          "name": "assetTier",
          "type": {
            "defined": {
              "name": "assetTier"
            }
          }
        },
        {
          "name": "scaleInitialAssetWeightStart",
          "type": "u64"
        },
        {
          "name": "withdrawGuardThreshold",
          "type": "u64"
        },
        {
          "name": "orderTickSize",
          "type": "u64"
        },
        {
          "name": "orderStepSize",
          "type": "u64"
        },
        {
          "name": "ifTotalFactor",
          "type": "u32"
        },
        {
          "name": "name",
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
      "name": "deleteInitializedSpotMarket",
      "discriminator": [
        31,
        140,
        67,
        191,
        189,
        20,
        101,
        221
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "insuranceFundVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "initializeSerumFulfillmentConfig",
      "discriminator": [
        193,
        211,
        132,
        172,
        70,
        171,
        7,
        94
      ],
      "accounts": [
        {
          "name": "baseSpotMarket"
        },
        {
          "name": "quoteSpotMarket"
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "serumProgram"
        },
        {
          "name": "serumMarket"
        },
        {
          "name": "serumOpenOrders",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "serumFulfillmentConfig",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
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
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateSerumFulfillmentConfigStatus",
      "discriminator": [
        171,
        109,
        240,
        251,
        95,
        1,
        149,
        89
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "serumFulfillmentConfig",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "status",
          "type": {
            "defined": {
              "name": "spotFulfillmentConfigStatus"
            }
          }
        }
      ]
    },
    {
      "name": "initializeOpenbookV2FulfillmentConfig",
      "discriminator": [
        7,
        221,
        103,
        153,
        107,
        57,
        27,
        197
      ],
      "accounts": [
        {
          "name": "baseSpotMarket"
        },
        {
          "name": "quoteSpotMarket"
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "openbookV2Program"
        },
        {
          "name": "openbookV2Market"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "openbookV2FulfillmentConfig",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
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
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "openbookV2FulfillmentConfigStatus",
      "discriminator": [
        25,
        173,
        19,
        189,
        4,
        211,
        64,
        238
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "openbookV2FulfillmentConfig",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "status",
          "type": {
            "defined": {
              "name": "spotFulfillmentConfigStatus"
            }
          }
        }
      ]
    },
    {
      "name": "initializePhoenixFulfillmentConfig",
      "discriminator": [
        135,
        132,
        110,
        107,
        185,
        160,
        169,
        154
      ],
      "accounts": [
        {
          "name": "baseSpotMarket"
        },
        {
          "name": "quoteSpotMarket"
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "phoenixProgram"
        },
        {
          "name": "phoenixMarket"
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "phoenixFulfillmentConfig",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
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
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "phoenixFulfillmentConfigStatus",
      "discriminator": [
        96,
        31,
        113,
        32,
        12,
        203,
        7,
        154
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "phoenixFulfillmentConfig",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "status",
          "type": {
            "defined": {
              "name": "spotFulfillmentConfigStatus"
            }
          }
        }
      ]
    },
    {
      "name": "updateSerumVault",
      "discriminator": [
        219,
        8,
        246,
        96,
        169,
        121,
        91,
        110
      ],
      "accounts": [
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "srmVault"
        }
      ],
      "args": []
    },
    {
      "name": "initializePerpMarket",
      "discriminator": [
        132,
        9,
        229,
        118,
        117,
        118,
        117,
        62
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
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
          "name": "marketIndex",
          "type": "u16"
        },
        {
          "name": "ammBaseAssetReserve",
          "type": "u128"
        },
        {
          "name": "ammQuoteAssetReserve",
          "type": "u128"
        },
        {
          "name": "ammPeriodicity",
          "type": "i64"
        },
        {
          "name": "ammPegMultiplier",
          "type": "u128"
        },
        {
          "name": "oracleSource",
          "type": {
            "defined": {
              "name": "oracleSource"
            }
          }
        },
        {
          "name": "contractTier",
          "type": {
            "defined": {
              "name": "contractTier"
            }
          }
        },
        {
          "name": "marginRatioInitial",
          "type": "u32"
        },
        {
          "name": "marginRatioMaintenance",
          "type": "u32"
        },
        {
          "name": "liquidatorFee",
          "type": "u32"
        },
        {
          "name": "ifLiquidationFee",
          "type": "u32"
        },
        {
          "name": "imfFactor",
          "type": "u32"
        },
        {
          "name": "activeStatus",
          "type": "bool"
        },
        {
          "name": "baseSpread",
          "type": "u32"
        },
        {
          "name": "maxSpread",
          "type": "u32"
        },
        {
          "name": "maxOpenInterest",
          "type": "u128"
        },
        {
          "name": "maxRevenueWithdrawPerPeriod",
          "type": "u64"
        },
        {
          "name": "quoteMaxInsurance",
          "type": "u64"
        },
        {
          "name": "orderStepSize",
          "type": "u64"
        },
        {
          "name": "orderTickSize",
          "type": "u64"
        },
        {
          "name": "minOrderSize",
          "type": "u64"
        },
        {
          "name": "concentrationCoefScale",
          "type": "u128"
        },
        {
          "name": "curveUpdateIntensity",
          "type": "u8"
        },
        {
          "name": "ammJitIntensity",
          "type": "u8"
        },
        {
          "name": "name",
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
      "name": "initializePredictionMarket",
      "discriminator": [
        248,
        70,
        198,
        224,
        224,
        105,
        125,
        195
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "deleteInitializedPerpMarket",
      "discriminator": [
        91,
        154,
        24,
        87,
        106,
        59,
        190,
        66
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "moveAmmPrice",
      "discriminator": [
        235,
        109,
        2,
        82,
        219,
        118,
        6,
        159
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "baseAssetReserve",
          "type": "u128"
        },
        {
          "name": "quoteAssetReserve",
          "type": "u128"
        },
        {
          "name": "sqrtK",
          "type": "u128"
        }
      ]
    },
    {
      "name": "recenterPerpMarketAmm",
      "discriminator": [
        24,
        87,
        10,
        115,
        165,
        190,
        80,
        139
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "pegMultiplier",
          "type": "u128"
        },
        {
          "name": "sqrtK",
          "type": "u128"
        }
      ]
    },
    {
      "name": "updatePerpMarketAmmSummaryStats",
      "discriminator": [
        122,
        101,
        249,
        238,
        209,
        9,
        241,
        245
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "spotMarket"
        },
        {
          "name": "oracle"
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "updatePerpMarketSummaryStatsParams"
            }
          }
        }
      ]
    },
    {
      "name": "updatePerpMarketExpiry",
      "discriminator": [
        44,
        221,
        227,
        151,
        131,
        140,
        22,
        110
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "expiryTs",
          "type": "i64"
        }
      ]
    },
    {
      "name": "settleExpiredMarketPoolsToRevenuePool",
      "discriminator": [
        55,
        19,
        238,
        169,
        227,
        90,
        200,
        184
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "depositIntoPerpMarketFeePool",
      "discriminator": [
        34,
        58,
        57,
        68,
        97,
        80,
        244,
        6
      ],
      "accounts": [
        {
          "name": "state",
          "writable": true
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "sourceVault",
          "writable": true
        },
        {
          "name": "driftSigner"
        },
        {
          "name": "quoteSpotMarket",
          "writable": true
        },
        {
          "name": "spotMarketVault",
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
      "name": "depositIntoSpotMarketVault",
      "discriminator": [
        48,
        252,
        119,
        73,
        255,
        205,
        174,
        247
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "sourceVault",
          "writable": true
        },
        {
          "name": "spotMarketVault",
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
      "name": "depositIntoSpotMarketRevenuePool",
      "discriminator": [
        92,
        40,
        151,
        42,
        122,
        254,
        139,
        246
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "spotMarketVault",
          "writable": true
        },
        {
          "name": "userTokenAccount",
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
      "name": "repegAmmCurve",
      "discriminator": [
        3,
        36,
        102,
        89,
        180,
        128,
        120,
        213
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "admin",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "newPegCandidate",
          "type": "u128"
        }
      ]
    },
    {
      "name": "updatePerpMarketAmmOracleTwap",
      "discriminator": [
        241,
        74,
        114,
        123,
        206,
        153,
        24,
        202
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "admin",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "resetPerpMarketAmmOracleTwap",
      "discriminator": [
        127,
        10,
        55,
        164,
        123,
        226,
        47,
        24
      ],
      "accounts": [
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "admin",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "updateK",
      "discriminator": [
        72,
        98,
        9,
        139,
        129,
        229,
        172,
        56
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        }
      ],
      "args": [
        {
          "name": "sqrtK",
          "type": "u128"
        }
      ]
    },
    {
      "name": "updatePerpMarketMarginRatio",
      "discriminator": [
        130,
        173,
        107,
        45,
        119,
        105,
        26,
        113
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marginRatioInitial",
          "type": "u32"
        },
        {
          "name": "marginRatioMaintenance",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updatePerpMarketHighLeverageMarginRatio",
      "discriminator": [
        88,
        112,
        86,
        49,
        24,
        116,
        74,
        157
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "marginRatioInitial",
          "type": "u16"
        },
        {
          "name": "marginRatioMaintenance",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updatePerpMarketFundingPeriod",
      "discriminator": [
        171,
        161,
        69,
        91,
        129,
        139,
        161,
        28
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "fundingPeriod",
          "type": "i64"
        }
      ]
    },
    {
      "name": "updatePerpMarketMaxImbalances",
      "discriminator": [
        15,
        206,
        73,
        133,
        60,
        8,
        86,
        89
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "unrealizedMaxImbalance",
          "type": "u64"
        },
        {
          "name": "maxRevenueWithdrawPerPeriod",
          "type": "u64"
        },
        {
          "name": "quoteMaxInsurance",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updatePerpMarketLiquidationFee",
      "discriminator": [
        90,
        137,
        9,
        145,
        41,
        8,
        148,
        117
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "liquidatorFee",
          "type": "u32"
        },
        {
          "name": "ifLiquidationFee",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateInsuranceFundUnstakingPeriod",
      "discriminator": [
        44,
        69,
        43,
        226,
        204,
        223,
        202,
        52
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "insuranceFundUnstakingPeriod",
          "type": "i64"
        }
      ]
    },
    {
      "name": "updateSpotMarketPoolId",
      "discriminator": [
        22,
        213,
        197,
        160,
        139,
        193,
        81,
        149
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "poolId",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateSpotMarketLiquidationFee",
      "discriminator": [
        11,
        13,
        255,
        53,
        56,
        136,
        104,
        177
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "liquidatorFee",
          "type": "u32"
        },
        {
          "name": "ifLiquidationFee",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateWithdrawGuardThreshold",
      "discriminator": [
        56,
        18,
        39,
        61,
        155,
        211,
        44,
        133
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "withdrawGuardThreshold",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updateSpotMarketIfFactor",
      "discriminator": [
        147,
        30,
        224,
        34,
        18,
        230,
        105,
        4
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "spotMarketIndex",
          "type": "u16"
        },
        {
          "name": "userIfFactor",
          "type": "u32"
        },
        {
          "name": "totalIfFactor",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateSpotMarketRevenueSettlePeriod",
      "discriminator": [
        81,
        92,
        126,
        41,
        250,
        225,
        156,
        219
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "revenueSettlePeriod",
          "type": "i64"
        }
      ]
    },
    {
      "name": "updateSpotMarketStatus",
      "discriminator": [
        78,
        94,
        16,
        188,
        193,
        110,
        231,
        31
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "status",
          "type": {
            "defined": {
              "name": "marketStatus"
            }
          }
        }
      ]
    },
    {
      "name": "updateSpotMarketPausedOperations",
      "discriminator": [
        100,
        61,
        153,
        81,
        180,
        12,
        6,
        248
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "pausedOperations",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateSpotMarketAssetTier",
      "discriminator": [
        253,
        209,
        231,
        14,
        242,
        208,
        243,
        130
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "assetTier",
          "type": {
            "defined": {
              "name": "assetTier"
            }
          }
        }
      ]
    },
    {
      "name": "updateSpotMarketMarginWeights",
      "discriminator": [
        109,
        33,
        87,
        195,
        255,
        36,
        6,
        81
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "initialAssetWeight",
          "type": "u32"
        },
        {
          "name": "maintenanceAssetWeight",
          "type": "u32"
        },
        {
          "name": "initialLiabilityWeight",
          "type": "u32"
        },
        {
          "name": "maintenanceLiabilityWeight",
          "type": "u32"
        },
        {
          "name": "imfFactor",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateSpotMarketBorrowRate",
      "discriminator": [
        71,
        239,
        236,
        153,
        210,
        62,
        254,
        76
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "optimalUtilization",
          "type": "u32"
        },
        {
          "name": "optimalBorrowRate",
          "type": "u32"
        },
        {
          "name": "maxBorrowRate",
          "type": "u32"
        },
        {
          "name": "minBorrowRate",
          "type": {
            "option": "u8"
          }
        }
      ]
    },
    {
      "name": "updateSpotMarketMaxTokenDeposits",
      "discriminator": [
        56,
        191,
        79,
        18,
        26,
        121,
        80,
        208
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxTokenDeposits",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updateSpotMarketMaxTokenBorrows",
      "discriminator": [
        57,
        102,
        204,
        212,
        253,
        95,
        13,
        199
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxTokenBorrowsFraction",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateSpotMarketScaleInitialAssetWeightStart",
      "discriminator": [
        217,
        204,
        204,
        118,
        204,
        130,
        225,
        147
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "scaleInitialAssetWeightStart",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updateSpotMarketOracle",
      "discriminator": [
        114,
        184,
        102,
        37,
        246,
        186,
        180,
        99
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "oldOracle"
        }
      ],
      "args": [
        {
          "name": "oracle",
          "type": "pubkey"
        },
        {
          "name": "oracleSource",
          "type": {
            "defined": {
              "name": "oracleSource"
            }
          }
        },
        {
          "name": "skipInvariantCheck",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updateSpotMarketStepSizeAndTickSize",
      "discriminator": [
        238,
        153,
        137,
        80,
        206,
        59,
        250,
        61
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "stepSize",
          "type": "u64"
        },
        {
          "name": "tickSize",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updateSpotMarketMinOrderSize",
      "discriminator": [
        93,
        128,
        11,
        119,
        26,
        20,
        181,
        50
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "orderSize",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updateSpotMarketOrdersEnabled",
      "discriminator": [
        190,
        79,
        206,
        15,
        26,
        229,
        229,
        43
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "ordersEnabled",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updateSpotMarketIfPausedOperations",
      "discriminator": [
        101,
        215,
        79,
        74,
        59,
        41,
        79,
        12
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "pausedOperations",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateSpotMarketName",
      "discriminator": [
        17,
        208,
        1,
        1,
        162,
        211,
        188,
        224
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "name",
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
      "name": "updatePerpMarketStatus",
      "discriminator": [
        71,
        201,
        175,
        122,
        255,
        207,
        196,
        207
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "status",
          "type": {
            "defined": {
              "name": "marketStatus"
            }
          }
        }
      ]
    },
    {
      "name": "updatePerpMarketPausedOperations",
      "discriminator": [
        53,
        16,
        136,
        132,
        30,
        220,
        121,
        85
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "pausedOperations",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updatePerpMarketContractTier",
      "discriminator": [
        236,
        128,
        15,
        95,
        203,
        214,
        68,
        117
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "contractTier",
          "type": {
            "defined": {
              "name": "contractTier"
            }
          }
        }
      ]
    },
    {
      "name": "updatePerpMarketImfFactor",
      "discriminator": [
        207,
        194,
        56,
        132,
        35,
        67,
        71,
        244
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "imfFactor",
          "type": "u32"
        },
        {
          "name": "unrealizedPnlImfFactor",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updatePerpMarketUnrealizedAssetWeight",
      "discriminator": [
        135,
        132,
        205,
        165,
        109,
        150,
        166,
        106
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "unrealizedInitialAssetWeight",
          "type": "u32"
        },
        {
          "name": "unrealizedMaintenanceAssetWeight",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updatePerpMarketConcentrationCoef",
      "discriminator": [
        24,
        78,
        232,
        126,
        169,
        176,
        230,
        16
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "concentrationScale",
          "type": "u128"
        }
      ]
    },
    {
      "name": "updatePerpMarketCurveUpdateIntensity",
      "discriminator": [
        50,
        131,
        6,
        156,
        226,
        231,
        189,
        72
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "curveUpdateIntensity",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updatePerpMarketTargetBaseAssetAmountPerLp",
      "discriminator": [
        62,
        87,
        68,
        115,
        29,
        150,
        150,
        165
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "targetBaseAssetAmountPerLp",
          "type": "i32"
        }
      ]
    },
    {
      "name": "updatePerpMarketPerLpBase",
      "discriminator": [
        103,
        152,
        103,
        102,
        89,
        144,
        193,
        71
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "perLpBase",
          "type": "i8"
        }
      ]
    },
    {
      "name": "updateLpCooldownTime",
      "discriminator": [
        198,
        133,
        88,
        41,
        241,
        119,
        61,
        14
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "lpCooldownTime",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updatePerpFeeStructure",
      "discriminator": [
        23,
        178,
        111,
        203,
        73,
        22,
        140,
        75
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feeStructure",
          "type": {
            "defined": {
              "name": "feeStructure"
            }
          }
        }
      ]
    },
    {
      "name": "updateSpotFeeStructure",
      "discriminator": [
        97,
        216,
        105,
        131,
        113,
        246,
        142,
        141
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feeStructure",
          "type": {
            "defined": {
              "name": "feeStructure"
            }
          }
        }
      ]
    },
    {
      "name": "updateInitialPctToLiquidate",
      "discriminator": [
        210,
        133,
        225,
        128,
        194,
        50,
        13,
        109
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "initialPctToLiquidate",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateLiquidationDuration",
      "discriminator": [
        28,
        154,
        20,
        249,
        102,
        192,
        73,
        71
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "liquidationDuration",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateLiquidationMarginBufferRatio",
      "discriminator": [
        132,
        224,
        243,
        160,
        154,
        82,
        97,
        215
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "liquidationMarginBufferRatio",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateOracleGuardRails",
      "discriminator": [
        131,
        112,
        10,
        59,
        32,
        54,
        40,
        164
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "oracleGuardRails",
          "type": {
            "defined": {
              "name": "oracleGuardRails"
            }
          }
        }
      ]
    },
    {
      "name": "updateStateSettlementDuration",
      "discriminator": [
        97,
        68,
        199,
        235,
        131,
        80,
        61,
        173
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "settlementDuration",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateStateMaxNumberOfSubAccounts",
      "discriminator": [
        155,
        123,
        214,
        2,
        221,
        166,
        204,
        85
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxNumberOfSubAccounts",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateStateMaxInitializeUserFee",
      "discriminator": [
        237,
        225,
        25,
        237,
        193,
        45,
        77,
        97
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxInitializeUserFee",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updatePerpMarketOracle",
      "discriminator": [
        182,
        113,
        111,
        160,
        67,
        174,
        89,
        191
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "oracle"
        },
        {
          "name": "oldOracle"
        }
      ],
      "args": [
        {
          "name": "oracle",
          "type": "pubkey"
        },
        {
          "name": "oracleSource",
          "type": {
            "defined": {
              "name": "oracleSource"
            }
          }
        },
        {
          "name": "skipInvariantCheck",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updatePerpMarketBaseSpread",
      "discriminator": [
        71,
        95,
        84,
        168,
        9,
        157,
        198,
        65
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "baseSpread",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateAmmJitIntensity",
      "discriminator": [
        181,
        191,
        53,
        109,
        166,
        249,
        55,
        142
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "ammJitIntensity",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updatePerpMarketMaxSpread",
      "discriminator": [
        80,
        252,
        122,
        62,
        40,
        218,
        91,
        100
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxSpread",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updatePerpMarketStepSizeAndTickSize",
      "discriminator": [
        231,
        255,
        97,
        25,
        146,
        139,
        174,
        4
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "stepSize",
          "type": "u64"
        },
        {
          "name": "tickSize",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updatePerpMarketName",
      "discriminator": [
        211,
        31,
        21,
        210,
        64,
        108,
        66,
        201
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "name",
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
      "name": "updatePerpMarketMinOrderSize",
      "discriminator": [
        226,
        74,
        5,
        89,
        108,
        223,
        46,
        141
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "orderSize",
          "type": "u64"
        }
      ]
    },
    {
      "name": "updatePerpMarketMaxSlippageRatio",
      "discriminator": [
        235,
        37,
        40,
        196,
        70,
        146,
        54,
        201
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxSlippageRatio",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updatePerpMarketMaxFillReserveFraction",
      "discriminator": [
        19,
        172,
        114,
        154,
        42,
        135,
        161,
        133
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxFillReserveFraction",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updatePerpMarketMaxOpenInterest",
      "discriminator": [
        194,
        79,
        149,
        224,
        246,
        102,
        186,
        140
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "maxOpenInterest",
          "type": "u128"
        }
      ]
    },
    {
      "name": "updatePerpMarketNumberOfUsers",
      "discriminator": [
        35,
        62,
        144,
        177,
        180,
        62,
        215,
        196
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "numberOfUsers",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "numberOfUsersWithBase",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "updatePerpMarketFeeAdjustment",
      "discriminator": [
        194,
        174,
        87,
        102,
        43,
        148,
        32,
        112
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feeAdjustment",
          "type": "i16"
        }
      ]
    },
    {
      "name": "updateSpotMarketFeeAdjustment",
      "discriminator": [
        148,
        182,
        3,
        126,
        157,
        114,
        220,
        99
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feeAdjustment",
          "type": "i16"
        }
      ]
    },
    {
      "name": "updatePerpMarketFuel",
      "discriminator": [
        252,
        141,
        110,
        101,
        27,
        99,
        182,
        21
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "perpMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "fuelBoostTaker",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "fuelBoostMaker",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "fuelBoostPosition",
          "type": {
            "option": "u8"
          }
        }
      ]
    },
    {
      "name": "updateSpotMarketFuel",
      "discriminator": [
        226,
        253,
        76,
        71,
        17,
        2,
        171,
        169
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "spotMarket",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "fuelBoostDeposits",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "fuelBoostBorrows",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "fuelBoostTaker",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "fuelBoostMaker",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "fuelBoostInsurance",
          "type": {
            "option": "u8"
          }
        }
      ]
    },
    {
      "name": "initUserFuel",
      "discriminator": [
        132,
        191,
        228,
        141,
        201,
        138,
        60,
        48
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state"
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "userStats",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "fuelBoostDeposits",
          "type": {
            "option": "i32"
          }
        },
        {
          "name": "fuelBoostBorrows",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "fuelBoostTaker",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "fuelBoostMaker",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "fuelBoostInsurance",
          "type": {
            "option": "u32"
          }
        }
      ]
    },
    {
      "name": "updateAdmin",
      "discriminator": [
        161,
        176,
        40,
        213,
        60,
        184,
        179,
        228
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "admin",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "updateWhitelistMint",
      "discriminator": [
        161,
        15,
        162,
        19,
        148,
        120,
        144,
        151
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "whitelistMint",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "updateDiscountMint",
      "discriminator": [
        32,
        252,
        122,
        211,
        66,
        31,
        47,
        241
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "discountMint",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "updateExchangeStatus",
      "discriminator": [
        83,
        160,
        252,
        250,
        129,
        116,
        49,
        223
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "exchangeStatus",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updatePerpAuctionDuration",
      "discriminator": [
        126,
        110,
        52,
        174,
        30,
        206,
        215,
        90
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "minPerpAuctionDuration",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateSpotAuctionDuration",
      "discriminator": [
        182,
        178,
        203,
        72,
        187,
        143,
        157,
        107
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "state",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "defaultSpotAuctionDuration",
          "type": "u8"
        }
      ]
    },
    {
      "name": "initializeProtocolIfSharesTransferConfig",
      "discriminator": [
        89,
        131,
        239,
        200,
        178,
        141,
        106,
        194
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "protocolIfSharesTransferConfig",
          "writable": true
        },
        {
          "name": "state"
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
      "name": "updateProtocolIfSharesTransferConfig",
      "discriminator": [
        34,
        135,
        47,
        91,
        220,
        24,
        212,
        53
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "protocolIfSharesTransferConfig",
          "writable": true
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "whitelistedSigners",
          "type": {
            "option": {
              "array": [
                "pubkey",
                4
              ]
            }
          }
        },
        {
          "name": "maxTransferPerEpoch",
          "type": {
            "option": "u128"
          }
        }
      ]
    },
    {
      "name": "initializePrelaunchOracle",
      "discriminator": [
        169,
        178,
        84,
        25,
        175,
        62,
        29,
        247
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "prelaunchOracle",
          "writable": true
        },
        {
          "name": "state"
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
          "name": "params",
          "type": {
            "defined": {
              "name": "prelaunchOracleParams"
            }
          }
        }
      ]
    },
    {
      "name": "updatePrelaunchOracleParams",
      "discriminator": [
        98,
        205,
        147,
        243,
        18,
        75,
        83,
        207
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "prelaunchOracle",
          "writable": true
        },
        {
          "name": "perpMarket",
          "writable": true
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "params",
          "type": {
            "defined": {
              "name": "prelaunchOracleParams"
            }
          }
        }
      ]
    },
    {
      "name": "deletePrelaunchOracle",
      "discriminator": [
        59,
        169,
        100,
        49,
        69,
        17,
        173,
        253
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "prelaunchOracle",
          "writable": true
        },
        {
          "name": "perpMarket"
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "perpMarketIndex",
          "type": "u16"
        }
      ]
    },
    {
      "name": "initializePythPullOracle",
      "discriminator": [
        249,
        140,
        253,
        243,
        248,
        74,
        240,
        238
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "pythSolanaReceiver"
        },
        {
          "name": "priceFeed",
          "writable": true
        },
        {
          "name": "systemProgram"
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "feedId",
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
      "name": "initializePythLazerOracle",
      "discriminator": [
        140,
        107,
        33,
        214,
        235,
        219,
        103,
        20
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "lazerOracle",
          "writable": true
        },
        {
          "name": "state"
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
          "name": "feedId",
          "type": "u32"
        }
      ]
    },
    {
      "name": "postPythLazerOracleUpdate",
      "discriminator": [
        218,
        237,
        170,
        245,
        39,
        143,
        166,
        33
      ],
      "accounts": [
        {
          "name": "keeper",
          "writable": true,
          "signer": true
        },
        {
          "name": "pythLazerStorage"
        },
        {
          "name": "ixSysvar"
        }
      ],
      "args": [
        {
          "name": "pythMessage",
          "type": "bytes"
        }
      ]
    },
    {
      "name": "initializeHighLeverageModeConfig",
      "discriminator": [
        213,
        167,
        93,
        246,
        208,
        130,
        90,
        248
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "highLeverageModeConfig",
          "writable": true
        },
        {
          "name": "state"
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
          "name": "maxUsers",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateHighLeverageModeConfig",
      "discriminator": [
        64,
        122,
        212,
        93,
        141,
        217,
        202,
        55
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "highLeverageModeConfig",
          "writable": true
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "maxUsers",
          "type": "u32"
        },
        {
          "name": "reduceOnly",
          "type": "bool"
        }
      ]
    },
    {
      "name": "initializeProtectedMakerModeConfig",
      "discriminator": [
        67,
        103,
        220,
        67,
        88,
        32,
        252,
        8
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "protectedMakerModeConfig",
          "writable": true
        },
        {
          "name": "state"
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
          "name": "maxUsers",
          "type": "u32"
        }
      ]
    },
    {
      "name": "updateProtectedMakerModeConfig",
      "discriminator": [
        86,
        166,
        235,
        253,
        67,
        202,
        223,
        17
      ],
      "accounts": [
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "protectedMakerModeConfig",
          "writable": true
        },
        {
          "name": "state"
        }
      ],
      "args": [
        {
          "name": "maxUsers",
          "type": "u32"
        },
        {
          "name": "reduceOnly",
          "type": "bool"
        },
        {
          "name": "currentUsers",
          "type": {
            "option": "u32"
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "openbookV2FulfillmentConfig",
      "discriminator": [
        3,
        43,
        58,
        106,
        131,
        132,
        199,
        171
      ]
    },
    {
      "name": "phoenixV1FulfillmentConfig",
      "discriminator": [
        233,
        45,
        62,
        40,
        35,
        129,
        48,
        72
      ]
    },
    {
      "name": "serumV3FulfillmentConfig",
      "discriminator": [
        65,
        160,
        197,
        112,
        239,
        168,
        103,
        185
      ]
    },
    {
      "name": "highLeverageModeConfig",
      "discriminator": [
        3,
        196,
        90,
        189,
        193,
        64,
        228,
        234
      ]
    },
    {
      "name": "insuranceFundStake",
      "discriminator": [
        110,
        202,
        14,
        42,
        95,
        73,
        90,
        95
      ]
    },
    {
      "name": "protocolIfSharesTransferConfig",
      "discriminator": [
        188,
        1,
        213,
        98,
        23,
        148,
        30,
        1
      ]
    },
    {
      "name": "prelaunchOracle",
      "discriminator": [
        92,
        14,
        139,
        234,
        72,
        244,
        68,
        26
      ]
    },
    {
      "name": "perpMarket",
      "discriminator": [
        10,
        223,
        12,
        44,
        107,
        245,
        55,
        247
      ]
    },
    {
      "name": "protectedMakerModeConfig",
      "discriminator": [
        47,
        86,
        90,
        9,
        224,
        255,
        10,
        69
      ]
    },
    {
      "name": "pythLazerOracle",
      "discriminator": [
        159,
        7,
        161,
        249,
        34,
        81,
        121,
        133
      ]
    },
    {
      "name": "signedMsgUserOrders",
      "discriminator": [
        70,
        6,
        50,
        248,
        222,
        1,
        143,
        49
      ]
    },
    {
      "name": "signedMsgWsDelegates",
      "discriminator": [
        190,
        115,
        111,
        44,
        216,
        252,
        108,
        85
      ]
    },
    {
      "name": "spotMarket",
      "discriminator": [
        100,
        177,
        8,
        107,
        168,
        65,
        65,
        39
      ]
    },
    {
      "name": "state",
      "discriminator": [
        216,
        146,
        107,
        94,
        104,
        75,
        182,
        177
      ]
    },
    {
      "name": "user",
      "discriminator": [
        159,
        117,
        95,
        227,
        239,
        151,
        58,
        236
      ]
    },
    {
      "name": "userStats",
      "discriminator": [
        176,
        223,
        136,
        27,
        122,
        79,
        32,
        227
      ]
    },
    {
      "name": "referrerName",
      "discriminator": [
        105,
        133,
        170,
        110,
        52,
        42,
        28,
        182
      ]
    },
    {
      "name": "fuelOverflow",
      "discriminator": [
        182,
        64,
        231,
        177,
        226,
        142,
        69,
        58
      ]
    }
  ],
  "events": [
    {
      "name": "newUserRecord",
      "discriminator": [
        236,
        186,
        113,
        219,
        42,
        51,
        149,
        249
      ]
    },
    {
      "name": "depositRecord",
      "discriminator": [
        180,
        241,
        218,
        207,
        102,
        135,
        44,
        134
      ]
    },
    {
      "name": "spotInterestRecord",
      "discriminator": [
        183,
        186,
        203,
        186,
        225,
        187,
        95,
        130
      ]
    },
    {
      "name": "fundingPaymentRecord",
      "discriminator": [
        8,
        59,
        96,
        20,
        137,
        201,
        56,
        95
      ]
    },
    {
      "name": "fundingRateRecord",
      "discriminator": [
        68,
        3,
        255,
        26,
        133,
        91,
        147,
        254
      ]
    },
    {
      "name": "curveRecord",
      "discriminator": [
        101,
        238,
        40,
        228,
        70,
        46,
        61,
        117
      ]
    },
    {
      "name": "signedMsgOrderRecord",
      "discriminator": [
        211,
        197,
        25,
        18,
        142,
        86,
        113,
        27
      ]
    },
    {
      "name": "orderRecord",
      "discriminator": [
        104,
        19,
        64,
        56,
        89,
        21,
        2,
        90
      ]
    },
    {
      "name": "orderActionRecord",
      "discriminator": [
        224,
        52,
        67,
        71,
        194,
        237,
        109,
        1
      ]
    },
    {
      "name": "lpRecord",
      "discriminator": [
        101,
        22,
        54,
        38,
        178,
        13,
        142,
        111
      ]
    },
    {
      "name": "liquidationRecord",
      "discriminator": [
        127,
        17,
        0,
        108,
        182,
        13,
        231,
        53
      ]
    },
    {
      "name": "settlePnlRecord",
      "discriminator": [
        57,
        68,
        105,
        26,
        119,
        198,
        213,
        89
      ]
    },
    {
      "name": "insuranceFundRecord",
      "discriminator": [
        56,
        222,
        215,
        235,
        78,
        197,
        99,
        146
      ]
    },
    {
      "name": "insuranceFundStakeRecord",
      "discriminator": [
        68,
        66,
        156,
        7,
        216,
        148,
        250,
        114
      ]
    },
    {
      "name": "swapRecord",
      "discriminator": [
        162,
        187,
        123,
        194,
        138,
        56,
        250,
        241
      ]
    },
    {
      "name": "spotMarketVaultDepositRecord",
      "discriminator": [
        178,
        217,
        23,
        188,
        127,
        190,
        32,
        73
      ]
    },
    {
      "name": "deleteUserRecord",
      "discriminator": [
        71,
        111,
        190,
        118,
        7,
        3,
        132,
        222
      ]
    },
    {
      "name": "fuelSweepRecord",
      "discriminator": [
        41,
        84,
        37,
        246,
        132,
        240,
        131,
        8
      ]
    },
    {
      "name": "fuelSeasonRecord",
      "discriminator": [
        19,
        137,
        119,
        33,
        224,
        249,
        6,
        87
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "invalidSpotMarketAuthority",
      "msg": "Invalid Spot Market Authority"
    },
    {
      "code": 6001,
      "name": "invalidInsuranceFundAuthority",
      "msg": "Clearing house not insurance fund authority"
    },
    {
      "code": 6002,
      "name": "insufficientDeposit",
      "msg": "Insufficient deposit"
    },
    {
      "code": 6003,
      "name": "insufficientCollateral",
      "msg": "Insufficient collateral"
    },
    {
      "code": 6004,
      "name": "sufficientCollateral",
      "msg": "Sufficient collateral"
    },
    {
      "code": 6005,
      "name": "maxNumberOfPositions",
      "msg": "Max number of positions taken"
    },
    {
      "code": 6006,
      "name": "adminControlsPricesDisabled",
      "msg": "Admin Controls Prices Disabled"
    },
    {
      "code": 6007,
      "name": "marketDelisted",
      "msg": "Market Delisted"
    },
    {
      "code": 6008,
      "name": "marketIndexAlreadyInitialized",
      "msg": "Market Index Already Initialized"
    },
    {
      "code": 6009,
      "name": "userAccountAndUserPositionsAccountMismatch",
      "msg": "User Account And User Positions Account Mismatch"
    },
    {
      "code": 6010,
      "name": "userHasNoPositionInMarket",
      "msg": "User Has No Position In Market"
    },
    {
      "code": 6011,
      "name": "invalidInitialPeg",
      "msg": "Invalid Initial Peg"
    },
    {
      "code": 6012,
      "name": "invalidRepegRedundant",
      "msg": "AMM repeg already configured with amt given"
    },
    {
      "code": 6013,
      "name": "invalidRepegDirection",
      "msg": "AMM repeg incorrect repeg direction"
    },
    {
      "code": 6014,
      "name": "invalidRepegProfitability",
      "msg": "AMM repeg out of bounds pnl"
    },
    {
      "code": 6015,
      "name": "slippageOutsideLimit",
      "msg": "Slippage Outside Limit Price"
    },
    {
      "code": 6016,
      "name": "orderSizeTooSmall",
      "msg": "Order Size Too Small"
    },
    {
      "code": 6017,
      "name": "invalidUpdateK",
      "msg": "Price change too large when updating K"
    },
    {
      "code": 6018,
      "name": "adminWithdrawTooLarge",
      "msg": "Admin tried to withdraw amount larger than fees collected"
    },
    {
      "code": 6019,
      "name": "mathError",
      "msg": "Math Error"
    },
    {
      "code": 6020,
      "name": "bnConversionError",
      "msg": "Conversion to u128/u64 failed with an overflow or underflow"
    },
    {
      "code": 6021,
      "name": "clockUnavailable",
      "msg": "Clock unavailable"
    },
    {
      "code": 6022,
      "name": "unableToLoadOracle",
      "msg": "Unable To Load Oracles"
    },
    {
      "code": 6023,
      "name": "priceBandsBreached",
      "msg": "Price Bands Breached"
    },
    {
      "code": 6024,
      "name": "exchangePaused",
      "msg": "Exchange is paused"
    },
    {
      "code": 6025,
      "name": "invalidWhitelistToken",
      "msg": "Invalid whitelist token"
    },
    {
      "code": 6026,
      "name": "whitelistTokenNotFound",
      "msg": "Whitelist token not found"
    },
    {
      "code": 6027,
      "name": "invalidDiscountToken",
      "msg": "Invalid discount token"
    },
    {
      "code": 6028,
      "name": "discountTokenNotFound",
      "msg": "Discount token not found"
    },
    {
      "code": 6029,
      "name": "referrerNotFound",
      "msg": "Referrer not found"
    },
    {
      "code": 6030,
      "name": "referrerStatsNotFound",
      "msg": "referrerNotFound"
    },
    {
      "code": 6031,
      "name": "referrerMustBeWritable",
      "msg": "referrerMustBeWritable"
    },
    {
      "code": 6032,
      "name": "referrerStatsMustBeWritable",
      "msg": "referrerMustBeWritable"
    },
    {
      "code": 6033,
      "name": "referrerAndReferrerStatsAuthorityUnequal",
      "msg": "referrerAndReferrerStatsAuthorityUnequal"
    },
    {
      "code": 6034,
      "name": "invalidReferrer",
      "msg": "invalidReferrer"
    },
    {
      "code": 6035,
      "name": "invalidOracle",
      "msg": "invalidOracle"
    },
    {
      "code": 6036,
      "name": "oracleNotFound",
      "msg": "oracleNotFound"
    },
    {
      "code": 6037,
      "name": "liquidationsBlockedByOracle",
      "msg": "Liquidations Blocked By Oracle"
    },
    {
      "code": 6038,
      "name": "maxDeposit",
      "msg": "Can not deposit more than max deposit"
    },
    {
      "code": 6039,
      "name": "cantDeleteUserWithCollateral",
      "msg": "Can not delete user that still has collateral"
    },
    {
      "code": 6040,
      "name": "invalidFundingProfitability",
      "msg": "AMM funding out of bounds pnl"
    },
    {
      "code": 6041,
      "name": "castingFailure",
      "msg": "Casting Failure"
    },
    {
      "code": 6042,
      "name": "invalidOrder",
      "msg": "invalidOrder"
    },
    {
      "code": 6043,
      "name": "invalidOrderMaxTs",
      "msg": "invalidOrderMaxTs"
    },
    {
      "code": 6044,
      "name": "invalidOrderMarketType",
      "msg": "invalidOrderMarketType"
    },
    {
      "code": 6045,
      "name": "invalidOrderForInitialMarginReq",
      "msg": "invalidOrderForInitialMarginReq"
    },
    {
      "code": 6046,
      "name": "invalidOrderNotRiskReducing",
      "msg": "invalidOrderNotRiskReducing"
    },
    {
      "code": 6047,
      "name": "invalidOrderSizeTooSmall",
      "msg": "invalidOrderSizeTooSmall"
    },
    {
      "code": 6048,
      "name": "invalidOrderNotStepSizeMultiple",
      "msg": "invalidOrderNotStepSizeMultiple"
    },
    {
      "code": 6049,
      "name": "invalidOrderBaseQuoteAsset",
      "msg": "invalidOrderBaseQuoteAsset"
    },
    {
      "code": 6050,
      "name": "invalidOrderIoc",
      "msg": "invalidOrderIoc"
    },
    {
      "code": 6051,
      "name": "invalidOrderPostOnly",
      "msg": "invalidOrderPostOnly"
    },
    {
      "code": 6052,
      "name": "invalidOrderIocPostOnly",
      "msg": "invalidOrderIocPostOnly"
    },
    {
      "code": 6053,
      "name": "invalidOrderTrigger",
      "msg": "invalidOrderTrigger"
    },
    {
      "code": 6054,
      "name": "invalidOrderAuction",
      "msg": "invalidOrderAuction"
    },
    {
      "code": 6055,
      "name": "invalidOrderOracleOffset",
      "msg": "invalidOrderOracleOffset"
    },
    {
      "code": 6056,
      "name": "invalidOrderMinOrderSize",
      "msg": "invalidOrderMinOrderSize"
    },
    {
      "code": 6057,
      "name": "placePostOnlyLimitFailure",
      "msg": "Failed to Place Post-Only Limit Order"
    },
    {
      "code": 6058,
      "name": "userHasNoOrder",
      "msg": "User has no order"
    },
    {
      "code": 6059,
      "name": "orderAmountTooSmall",
      "msg": "Order Amount Too Small"
    },
    {
      "code": 6060,
      "name": "maxNumberOfOrders",
      "msg": "Max number of orders taken"
    },
    {
      "code": 6061,
      "name": "orderDoesNotExist",
      "msg": "Order does not exist"
    },
    {
      "code": 6062,
      "name": "orderNotOpen",
      "msg": "Order not open"
    },
    {
      "code": 6063,
      "name": "fillOrderDidNotUpdateState",
      "msg": "fillOrderDidNotUpdateState"
    },
    {
      "code": 6064,
      "name": "reduceOnlyOrderIncreasedRisk",
      "msg": "Reduce only order increased risk"
    },
    {
      "code": 6065,
      "name": "unableToLoadAccountLoader",
      "msg": "Unable to load AccountLoader"
    },
    {
      "code": 6066,
      "name": "tradeSizeTooLarge",
      "msg": "Trade Size Too Large"
    },
    {
      "code": 6067,
      "name": "userCantReferThemselves",
      "msg": "User cant refer themselves"
    },
    {
      "code": 6068,
      "name": "didNotReceiveExpectedReferrer",
      "msg": "Did not receive expected referrer"
    },
    {
      "code": 6069,
      "name": "couldNotDeserializeReferrer",
      "msg": "Could not deserialize referrer"
    },
    {
      "code": 6070,
      "name": "couldNotDeserializeReferrerStats",
      "msg": "Could not deserialize referrer stats"
    },
    {
      "code": 6071,
      "name": "userOrderIdAlreadyInUse",
      "msg": "User Order Id Already In Use"
    },
    {
      "code": 6072,
      "name": "noPositionsLiquidatable",
      "msg": "No positions liquidatable"
    },
    {
      "code": 6073,
      "name": "invalidMarginRatio",
      "msg": "Invalid Margin Ratio"
    },
    {
      "code": 6074,
      "name": "cantCancelPostOnlyOrder",
      "msg": "Cant Cancel Post Only Order"
    },
    {
      "code": 6075,
      "name": "invalidOracleOffset",
      "msg": "invalidOracleOffset"
    },
    {
      "code": 6076,
      "name": "cantExpireOrders",
      "msg": "cantExpireOrders"
    },
    {
      "code": 6077,
      "name": "couldNotLoadMarketData",
      "msg": "couldNotLoadMarketData"
    },
    {
      "code": 6078,
      "name": "perpMarketNotFound",
      "msg": "perpMarketNotFound"
    },
    {
      "code": 6079,
      "name": "invalidMarketAccount",
      "msg": "invalidMarketAccount"
    },
    {
      "code": 6080,
      "name": "unableToLoadPerpMarketAccount",
      "msg": "unableToLoadMarketAccount"
    },
    {
      "code": 6081,
      "name": "marketWrongMutability",
      "msg": "marketWrongMutability"
    },
    {
      "code": 6082,
      "name": "unableToCastUnixTime",
      "msg": "unableToCastUnixTime"
    },
    {
      "code": 6083,
      "name": "couldNotFindSpotPosition",
      "msg": "couldNotFindSpotPosition"
    },
    {
      "code": 6084,
      "name": "noSpotPositionAvailable",
      "msg": "noSpotPositionAvailable"
    },
    {
      "code": 6085,
      "name": "invalidSpotMarketInitialization",
      "msg": "invalidSpotMarketInitialization"
    },
    {
      "code": 6086,
      "name": "couldNotLoadSpotMarketData",
      "msg": "couldNotLoadSpotMarketData"
    },
    {
      "code": 6087,
      "name": "spotMarketNotFound",
      "msg": "spotMarketNotFound"
    },
    {
      "code": 6088,
      "name": "invalidSpotMarketAccount",
      "msg": "invalidSpotMarketAccount"
    },
    {
      "code": 6089,
      "name": "unableToLoadSpotMarketAccount",
      "msg": "unableToLoadSpotMarketAccount"
    },
    {
      "code": 6090,
      "name": "spotMarketWrongMutability",
      "msg": "spotMarketWrongMutability"
    },
    {
      "code": 6091,
      "name": "spotMarketInterestNotUpToDate",
      "msg": "spotInterestNotUpToDate"
    },
    {
      "code": 6092,
      "name": "spotMarketInsufficientDeposits",
      "msg": "spotMarketInsufficientDeposits"
    },
    {
      "code": 6093,
      "name": "userMustSettleTheirOwnPositiveUnsettledPnl",
      "msg": "userMustSettleTheirOwnPositiveUnsettledPnl"
    },
    {
      "code": 6094,
      "name": "cantUpdatePoolBalanceType",
      "msg": "cantUpdatePoolBalanceType"
    },
    {
      "code": 6095,
      "name": "insufficientCollateralForSettlingPnl",
      "msg": "insufficientCollateralForSettlingPnl"
    },
    {
      "code": 6096,
      "name": "ammNotUpdatedInSameSlot",
      "msg": "ammNotUpdatedInSameSlot"
    },
    {
      "code": 6097,
      "name": "auctionNotComplete",
      "msg": "auctionNotComplete"
    },
    {
      "code": 6098,
      "name": "makerNotFound",
      "msg": "makerNotFound"
    },
    {
      "code": 6099,
      "name": "makerStatsNotFound",
      "msg": "makerNotFound"
    },
    {
      "code": 6100,
      "name": "makerMustBeWritable",
      "msg": "makerMustBeWritable"
    },
    {
      "code": 6101,
      "name": "makerStatsMustBeWritable",
      "msg": "makerMustBeWritable"
    },
    {
      "code": 6102,
      "name": "makerOrderNotFound",
      "msg": "makerOrderNotFound"
    },
    {
      "code": 6103,
      "name": "couldNotDeserializeMaker",
      "msg": "couldNotDeserializeMaker"
    },
    {
      "code": 6104,
      "name": "couldNotDeserializeMakerStats",
      "msg": "couldNotDeserializeMaker"
    },
    {
      "code": 6105,
      "name": "auctionPriceDoesNotSatisfyMaker",
      "msg": "auctionPriceDoesNotSatisfyMaker"
    },
    {
      "code": 6106,
      "name": "makerCantFulfillOwnOrder",
      "msg": "makerCantFulfillOwnOrder"
    },
    {
      "code": 6107,
      "name": "makerOrderMustBePostOnly",
      "msg": "makerOrderMustBePostOnly"
    },
    {
      "code": 6108,
      "name": "cantMatchTwoPostOnlys",
      "msg": "cantMatchTwoPostOnlys"
    },
    {
      "code": 6109,
      "name": "orderBreachesOraclePriceLimits",
      "msg": "orderBreachesOraclePriceLimits"
    },
    {
      "code": 6110,
      "name": "orderMustBeTriggeredFirst",
      "msg": "orderMustBeTriggeredFirst"
    },
    {
      "code": 6111,
      "name": "orderNotTriggerable",
      "msg": "orderNotTriggerable"
    },
    {
      "code": 6112,
      "name": "orderDidNotSatisfyTriggerCondition",
      "msg": "orderDidNotSatisfyTriggerCondition"
    },
    {
      "code": 6113,
      "name": "positionAlreadyBeingLiquidated",
      "msg": "positionAlreadyBeingLiquidated"
    },
    {
      "code": 6114,
      "name": "positionDoesntHaveOpenPositionOrOrders",
      "msg": "positionDoesntHaveOpenPositionOrOrders"
    },
    {
      "code": 6115,
      "name": "allOrdersAreAlreadyLiquidations",
      "msg": "allOrdersAreAlreadyLiquidations"
    },
    {
      "code": 6116,
      "name": "cantCancelLiquidationOrder",
      "msg": "cantCancelLiquidationOrder"
    },
    {
      "code": 6117,
      "name": "userIsBeingLiquidated",
      "msg": "userIsBeingLiquidated"
    },
    {
      "code": 6118,
      "name": "liquidationsOngoing",
      "msg": "liquidationsOngoing"
    },
    {
      "code": 6119,
      "name": "wrongSpotBalanceType",
      "msg": "wrongSpotBalanceType"
    },
    {
      "code": 6120,
      "name": "userCantLiquidateThemself",
      "msg": "userCantLiquidateThemself"
    },
    {
      "code": 6121,
      "name": "invalidPerpPositionToLiquidate",
      "msg": "invalidPerpPositionToLiquidate"
    },
    {
      "code": 6122,
      "name": "invalidBaseAssetAmountForLiquidatePerp",
      "msg": "invalidBaseAssetAmountForLiquidatePerp"
    },
    {
      "code": 6123,
      "name": "invalidPositionLastFundingRate",
      "msg": "invalidPositionLastFundingRate"
    },
    {
      "code": 6124,
      "name": "invalidPositionDelta",
      "msg": "invalidPositionDelta"
    },
    {
      "code": 6125,
      "name": "userBankrupt",
      "msg": "userBankrupt"
    },
    {
      "code": 6126,
      "name": "userNotBankrupt",
      "msg": "userNotBankrupt"
    },
    {
      "code": 6127,
      "name": "userHasInvalidBorrow",
      "msg": "userHasInvalidBorrow"
    },
    {
      "code": 6128,
      "name": "dailyWithdrawLimit",
      "msg": "dailyWithdrawLimit"
    },
    {
      "code": 6129,
      "name": "defaultError",
      "msg": "defaultError"
    },
    {
      "code": 6130,
      "name": "insufficientLpTokens",
      "msg": "Insufficient LP tokens"
    },
    {
      "code": 6131,
      "name": "cantLpWithPerpPosition",
      "msg": "Cant LP with a market position"
    },
    {
      "code": 6132,
      "name": "unableToBurnLpTokens",
      "msg": "Unable to burn LP tokens"
    },
    {
      "code": 6133,
      "name": "tryingToRemoveLiquidityTooFast",
      "msg": "Trying to remove liqudity too fast after adding it"
    },
    {
      "code": 6134,
      "name": "invalidSpotMarketVault",
      "msg": "Invalid Spot Market Vault"
    },
    {
      "code": 6135,
      "name": "invalidSpotMarketState",
      "msg": "Invalid Spot Market State"
    },
    {
      "code": 6136,
      "name": "invalidSerumProgram",
      "msg": "invalidSerumProgram"
    },
    {
      "code": 6137,
      "name": "invalidSerumMarket",
      "msg": "invalidSerumMarket"
    },
    {
      "code": 6138,
      "name": "invalidSerumBids",
      "msg": "invalidSerumBids"
    },
    {
      "code": 6139,
      "name": "invalidSerumAsks",
      "msg": "invalidSerumAsks"
    },
    {
      "code": 6140,
      "name": "invalidSerumOpenOrders",
      "msg": "invalidSerumOpenOrders"
    },
    {
      "code": 6141,
      "name": "failedSerumCpi",
      "msg": "failedSerumCpi"
    },
    {
      "code": 6142,
      "name": "failedToFillOnExternalMarket",
      "msg": "failedToFillOnExternalMarket"
    },
    {
      "code": 6143,
      "name": "invalidFulfillmentConfig",
      "msg": "invalidFulfillmentConfig"
    },
    {
      "code": 6144,
      "name": "invalidFeeStructure",
      "msg": "invalidFeeStructure"
    },
    {
      "code": 6145,
      "name": "insufficientIfShares",
      "msg": "Insufficient IF shares"
    },
    {
      "code": 6146,
      "name": "marketActionPaused",
      "msg": "the Market has paused this action"
    },
    {
      "code": 6147,
      "name": "marketPlaceOrderPaused",
      "msg": "the Market status doesnt allow placing orders"
    },
    {
      "code": 6148,
      "name": "marketFillOrderPaused",
      "msg": "the Market status doesnt allow filling orders"
    },
    {
      "code": 6149,
      "name": "marketWithdrawPaused",
      "msg": "the Market status doesnt allow withdraws"
    },
    {
      "code": 6150,
      "name": "protectedAssetTierViolation",
      "msg": "Action violates the Protected Asset Tier rules"
    },
    {
      "code": 6151,
      "name": "isolatedAssetTierViolation",
      "msg": "Action violates the Isolated Asset Tier rules"
    },
    {
      "code": 6152,
      "name": "userCantBeDeleted",
      "msg": "User Cant Be Deleted"
    },
    {
      "code": 6153,
      "name": "reduceOnlyWithdrawIncreasedRisk",
      "msg": "Reduce Only Withdraw Increased Risk"
    },
    {
      "code": 6154,
      "name": "maxOpenInterest",
      "msg": "Max Open Interest"
    },
    {
      "code": 6155,
      "name": "cantResolvePerpBankruptcy",
      "msg": "Cant Resolve Perp Bankruptcy"
    },
    {
      "code": 6156,
      "name": "liquidationDoesntSatisfyLimitPrice",
      "msg": "Liquidation Doesnt Satisfy Limit Price"
    },
    {
      "code": 6157,
      "name": "marginTradingDisabled",
      "msg": "Margin Trading Disabled"
    },
    {
      "code": 6158,
      "name": "invalidMarketStatusToSettlePnl",
      "msg": "Invalid Market Status to Settle Perp Pnl"
    },
    {
      "code": 6159,
      "name": "perpMarketNotInSettlement",
      "msg": "perpMarketNotInSettlement"
    },
    {
      "code": 6160,
      "name": "perpMarketNotInReduceOnly",
      "msg": "perpMarketNotInReduceOnly"
    },
    {
      "code": 6161,
      "name": "perpMarketSettlementBufferNotReached",
      "msg": "perpMarketSettlementBufferNotReached"
    },
    {
      "code": 6162,
      "name": "perpMarketSettlementUserHasOpenOrders",
      "msg": "perpMarketSettlementUserHasOpenOrders"
    },
    {
      "code": 6163,
      "name": "perpMarketSettlementUserHasActiveLp",
      "msg": "perpMarketSettlementUserHasActiveLp"
    },
    {
      "code": 6164,
      "name": "unableToSettleExpiredUserPosition",
      "msg": "unableToSettleExpiredUserPosition"
    },
    {
      "code": 6165,
      "name": "unequalMarketIndexForSpotTransfer",
      "msg": "unequalMarketIndexForSpotTransfer"
    },
    {
      "code": 6166,
      "name": "invalidPerpPositionDetected",
      "msg": "invalidPerpPositionDetected"
    },
    {
      "code": 6167,
      "name": "invalidSpotPositionDetected",
      "msg": "invalidSpotPositionDetected"
    },
    {
      "code": 6168,
      "name": "invalidAmmDetected",
      "msg": "invalidAmmDetected"
    },
    {
      "code": 6169,
      "name": "invalidAmmForFillDetected",
      "msg": "invalidAmmForFillDetected"
    },
    {
      "code": 6170,
      "name": "invalidAmmLimitPriceOverride",
      "msg": "invalidAmmLimitPriceOverride"
    },
    {
      "code": 6171,
      "name": "invalidOrderFillPrice",
      "msg": "invalidOrderFillPrice"
    },
    {
      "code": 6172,
      "name": "spotMarketBalanceInvariantViolated",
      "msg": "spotMarketBalanceInvariantViolated"
    },
    {
      "code": 6173,
      "name": "spotMarketVaultInvariantViolated",
      "msg": "spotMarketVaultInvariantViolated"
    },
    {
      "code": 6174,
      "name": "invalidPda",
      "msg": "invalidPda"
    },
    {
      "code": 6175,
      "name": "invalidPdaSigner",
      "msg": "invalidPdaSigner"
    },
    {
      "code": 6176,
      "name": "revenueSettingsCannotSettleToIf",
      "msg": "revenueSettingsCannotSettleToIf"
    },
    {
      "code": 6177,
      "name": "noRevenueToSettleToIf",
      "msg": "noRevenueToSettleToIf"
    },
    {
      "code": 6178,
      "name": "noAmmPerpPnlDeficit",
      "msg": "noAmmPerpPnlDeficit"
    },
    {
      "code": 6179,
      "name": "sufficientPerpPnlPool",
      "msg": "sufficientPerpPnlPool"
    },
    {
      "code": 6180,
      "name": "insufficientPerpPnlPool",
      "msg": "insufficientPerpPnlPool"
    },
    {
      "code": 6181,
      "name": "perpPnlDeficitBelowThreshold",
      "msg": "perpPnlDeficitBelowThreshold"
    },
    {
      "code": 6182,
      "name": "maxRevenueWithdrawPerPeriodReached",
      "msg": "maxRevenueWithdrawPerPeriodReached"
    },
    {
      "code": 6183,
      "name": "maxIfWithdrawReached",
      "msg": "invalidSpotPositionDetected"
    },
    {
      "code": 6184,
      "name": "noIfWithdrawAvailable",
      "msg": "noIfWithdrawAvailable"
    },
    {
      "code": 6185,
      "name": "invalidIfUnstake",
      "msg": "invalidIfUnstake"
    },
    {
      "code": 6186,
      "name": "invalidIfUnstakeSize",
      "msg": "invalidIfUnstakeSize"
    },
    {
      "code": 6187,
      "name": "invalidIfUnstakeCancel",
      "msg": "invalidIfUnstakeCancel"
    },
    {
      "code": 6188,
      "name": "invalidIfForNewStakes",
      "msg": "invalidIfForNewStakes"
    },
    {
      "code": 6189,
      "name": "invalidIfRebase",
      "msg": "invalidIfRebase"
    },
    {
      "code": 6190,
      "name": "invalidInsuranceUnstakeSize",
      "msg": "invalidInsuranceUnstakeSize"
    },
    {
      "code": 6191,
      "name": "invalidOrderLimitPrice",
      "msg": "invalidOrderLimitPrice"
    },
    {
      "code": 6192,
      "name": "invalidIfDetected",
      "msg": "invalidIfDetected"
    },
    {
      "code": 6193,
      "name": "invalidAmmMaxSpreadDetected",
      "msg": "invalidAmmMaxSpreadDetected"
    },
    {
      "code": 6194,
      "name": "invalidConcentrationCoef",
      "msg": "invalidConcentrationCoef"
    },
    {
      "code": 6195,
      "name": "invalidSrmVault",
      "msg": "invalidSrmVault"
    },
    {
      "code": 6196,
      "name": "invalidVaultOwner",
      "msg": "invalidVaultOwner"
    },
    {
      "code": 6197,
      "name": "invalidMarketStatusForFills",
      "msg": "invalidMarketStatusForFills"
    },
    {
      "code": 6198,
      "name": "ifWithdrawRequestInProgress",
      "msg": "ifWithdrawRequestInProgress"
    },
    {
      "code": 6199,
      "name": "noIfWithdrawRequestInProgress",
      "msg": "noIfWithdrawRequestInProgress"
    },
    {
      "code": 6200,
      "name": "ifWithdrawRequestTooSmall",
      "msg": "ifWithdrawRequestTooSmall"
    },
    {
      "code": 6201,
      "name": "incorrectSpotMarketAccountPassed",
      "msg": "incorrectSpotMarketAccountPassed"
    },
    {
      "code": 6202,
      "name": "blockchainClockInconsistency",
      "msg": "blockchainClockInconsistency"
    },
    {
      "code": 6203,
      "name": "invalidIfSharesDetected",
      "msg": "invalidIfSharesDetected"
    },
    {
      "code": 6204,
      "name": "newLpSizeTooSmall",
      "msg": "newLpSizeTooSmall"
    },
    {
      "code": 6205,
      "name": "marketStatusInvalidForNewLp",
      "msg": "marketStatusInvalidForNewLp"
    },
    {
      "code": 6206,
      "name": "invalidMarkTwapUpdateDetected",
      "msg": "invalidMarkTwapUpdateDetected"
    },
    {
      "code": 6207,
      "name": "marketSettlementAttemptOnActiveMarket",
      "msg": "marketSettlementAttemptOnActiveMarket"
    },
    {
      "code": 6208,
      "name": "marketSettlementRequiresSettledLp",
      "msg": "marketSettlementRequiresSettledLp"
    },
    {
      "code": 6209,
      "name": "marketSettlementAttemptTooEarly",
      "msg": "marketSettlementAttemptTooEarly"
    },
    {
      "code": 6210,
      "name": "marketSettlementTargetPriceInvalid",
      "msg": "marketSettlementTargetPriceInvalid"
    },
    {
      "code": 6211,
      "name": "unsupportedSpotMarket",
      "msg": "unsupportedSpotMarket"
    },
    {
      "code": 6212,
      "name": "spotOrdersDisabled",
      "msg": "spotOrdersDisabled"
    },
    {
      "code": 6213,
      "name": "marketBeingInitialized",
      "msg": "Market Being Initialized"
    },
    {
      "code": 6214,
      "name": "invalidUserSubAccountId",
      "msg": "Invalid Sub Account Id"
    },
    {
      "code": 6215,
      "name": "invalidTriggerOrderCondition",
      "msg": "Invalid Trigger Order Condition"
    },
    {
      "code": 6216,
      "name": "invalidSpotPosition",
      "msg": "Invalid Spot Position"
    },
    {
      "code": 6217,
      "name": "cantTransferBetweenSameUserAccount",
      "msg": "Cant transfer between same user account"
    },
    {
      "code": 6218,
      "name": "invalidPerpPosition",
      "msg": "Invalid Perp Position"
    },
    {
      "code": 6219,
      "name": "unableToGetLimitPrice",
      "msg": "Unable To Get Limit Price"
    },
    {
      "code": 6220,
      "name": "invalidLiquidation",
      "msg": "Invalid Liquidation"
    },
    {
      "code": 6221,
      "name": "spotFulfillmentConfigDisabled",
      "msg": "Spot Fulfillment Config Disabled"
    },
    {
      "code": 6222,
      "name": "invalidMaker",
      "msg": "Invalid Maker"
    },
    {
      "code": 6223,
      "name": "failedUnwrap",
      "msg": "Failed Unwrap"
    },
    {
      "code": 6224,
      "name": "maxNumberOfUsers",
      "msg": "Max Number Of Users"
    },
    {
      "code": 6225,
      "name": "invalidOracleForSettlePnl",
      "msg": "invalidOracleForSettlePnl"
    },
    {
      "code": 6226,
      "name": "marginOrdersOpen",
      "msg": "marginOrdersOpen"
    },
    {
      "code": 6227,
      "name": "tierViolationLiquidatingPerpPnl",
      "msg": "tierViolationLiquidatingPerpPnl"
    },
    {
      "code": 6228,
      "name": "couldNotLoadUserData",
      "msg": "couldNotLoadUserData"
    },
    {
      "code": 6229,
      "name": "userWrongMutability",
      "msg": "userWrongMutability"
    },
    {
      "code": 6230,
      "name": "invalidUserAccount",
      "msg": "invalidUserAccount"
    },
    {
      "code": 6231,
      "name": "couldNotLoadUserStatsData",
      "msg": "couldNotLoadUserData"
    },
    {
      "code": 6232,
      "name": "userStatsWrongMutability",
      "msg": "userWrongMutability"
    },
    {
      "code": 6233,
      "name": "invalidUserStatsAccount",
      "msg": "invalidUserAccount"
    },
    {
      "code": 6234,
      "name": "userNotFound",
      "msg": "userNotFound"
    },
    {
      "code": 6235,
      "name": "unableToLoadUserAccount",
      "msg": "unableToLoadUserAccount"
    },
    {
      "code": 6236,
      "name": "userStatsNotFound",
      "msg": "userStatsNotFound"
    },
    {
      "code": 6237,
      "name": "unableToLoadUserStatsAccount",
      "msg": "unableToLoadUserStatsAccount"
    },
    {
      "code": 6238,
      "name": "userNotInactive",
      "msg": "User Not Inactive"
    },
    {
      "code": 6239,
      "name": "revertFill",
      "msg": "revertFill"
    },
    {
      "code": 6240,
      "name": "invalidMarketAccountforDeletion",
      "msg": "Invalid MarketAccount for Deletion"
    },
    {
      "code": 6241,
      "name": "invalidSpotFulfillmentParams",
      "msg": "Invalid Spot Fulfillment Params"
    },
    {
      "code": 6242,
      "name": "failedToGetMint",
      "msg": "Failed to Get Mint"
    },
    {
      "code": 6243,
      "name": "failedPhoenixCpi",
      "msg": "failedPhoenixCpi"
    },
    {
      "code": 6244,
      "name": "failedToDeserializePhoenixMarket",
      "msg": "failedToDeserializePhoenixMarket"
    },
    {
      "code": 6245,
      "name": "invalidPricePrecision",
      "msg": "invalidPricePrecision"
    },
    {
      "code": 6246,
      "name": "invalidPhoenixProgram",
      "msg": "invalidPhoenixProgram"
    },
    {
      "code": 6247,
      "name": "invalidPhoenixMarket",
      "msg": "invalidPhoenixMarket"
    },
    {
      "code": 6248,
      "name": "invalidSwap",
      "msg": "invalidSwap"
    },
    {
      "code": 6249,
      "name": "swapLimitPriceBreached",
      "msg": "swapLimitPriceBreached"
    },
    {
      "code": 6250,
      "name": "spotMarketReduceOnly",
      "msg": "spotMarketReduceOnly"
    },
    {
      "code": 6251,
      "name": "fundingWasNotUpdated",
      "msg": "fundingWasNotUpdated"
    },
    {
      "code": 6252,
      "name": "impossibleFill",
      "msg": "impossibleFill"
    },
    {
      "code": 6253,
      "name": "cantUpdatePerpBidAskTwap",
      "msg": "cantUpdatePerpBidAskTwap"
    },
    {
      "code": 6254,
      "name": "userReduceOnly",
      "msg": "userReduceOnly"
    },
    {
      "code": 6255,
      "name": "invalidMarginCalculation",
      "msg": "invalidMarginCalculation"
    },
    {
      "code": 6256,
      "name": "cantPayUserInitFee",
      "msg": "cantPayUserInitFee"
    },
    {
      "code": 6257,
      "name": "cantReclaimRent",
      "msg": "cantReclaimRent"
    },
    {
      "code": 6258,
      "name": "insuranceFundOperationPaused",
      "msg": "insuranceFundOperationPaused"
    },
    {
      "code": 6259,
      "name": "noUnsettledPnl",
      "msg": "noUnsettledPnl"
    },
    {
      "code": 6260,
      "name": "pnlPoolCantSettleUser",
      "msg": "pnlPoolCantSettleUser"
    },
    {
      "code": 6261,
      "name": "oracleNonPositive",
      "msg": "oracleInvalid"
    },
    {
      "code": 6262,
      "name": "oracleTooVolatile",
      "msg": "oracleTooVolatile"
    },
    {
      "code": 6263,
      "name": "oracleTooUncertain",
      "msg": "oracleTooUncertain"
    },
    {
      "code": 6264,
      "name": "oracleStaleForMargin",
      "msg": "oracleStaleForMargin"
    },
    {
      "code": 6265,
      "name": "oracleInsufficientDataPoints",
      "msg": "oracleInsufficientDataPoints"
    },
    {
      "code": 6266,
      "name": "oracleStaleForAmm",
      "msg": "oracleStaleForAmm"
    },
    {
      "code": 6267,
      "name": "unableToParsePullOracleMessage",
      "msg": "Unable to parse pull oracle message"
    },
    {
      "code": 6268,
      "name": "maxBorrows",
      "msg": "Can not borow more than max borrows"
    },
    {
      "code": 6269,
      "name": "oracleUpdatesNotMonotonic",
      "msg": "Updates must be monotonically increasing"
    },
    {
      "code": 6270,
      "name": "oraclePriceFeedMessageMismatch",
      "msg": "Trying to update price feed with the wrong feed id"
    },
    {
      "code": 6271,
      "name": "oracleUnsupportedMessageType",
      "msg": "The message in the update must be a PriceFeedMessage"
    },
    {
      "code": 6272,
      "name": "oracleDeserializeMessageFailed",
      "msg": "Could not deserialize the message in the update"
    },
    {
      "code": 6273,
      "name": "oracleWrongGuardianSetOwner",
      "msg": "Wrong guardian set owner in update price atomic"
    },
    {
      "code": 6274,
      "name": "oracleWrongWriteAuthority",
      "msg": "Oracle post update atomic price feed account must be drift program"
    },
    {
      "code": 6275,
      "name": "oracleWrongVaaOwner",
      "msg": "Oracle vaa owner must be wormhole program"
    },
    {
      "code": 6276,
      "name": "oracleTooManyPriceAccountUpdates",
      "msg": "Multi updates must have 2 or fewer accounts passed in remaining accounts"
    },
    {
      "code": 6277,
      "name": "oracleMismatchedVaaAndPriceUpdates",
      "msg": "Don't have the same remaining accounts number and pyth updates left"
    },
    {
      "code": 6278,
      "name": "oracleBadRemainingAccountPublicKey",
      "msg": "Remaining account passed does not match oracle update derived pda"
    },
    {
      "code": 6279,
      "name": "failedOpenbookV2cpi",
      "msg": "failedOpenbookV2cpi"
    },
    {
      "code": 6280,
      "name": "invalidOpenbookV2Program",
      "msg": "invalidOpenbookV2Program"
    },
    {
      "code": 6281,
      "name": "invalidOpenbookV2Market",
      "msg": "invalidOpenbookV2Market"
    },
    {
      "code": 6282,
      "name": "nonZeroTransferFee",
      "msg": "Non zero transfer fee"
    },
    {
      "code": 6283,
      "name": "liquidationOrderFailedToFill",
      "msg": "Liquidation order failed to fill"
    },
    {
      "code": 6284,
      "name": "invalidPredictionMarketOrder",
      "msg": "Invalid prediction market order"
    },
    {
      "code": 6285,
      "name": "invalidVerificationIxIndex",
      "msg": "Ed25519 Ix must be before place and make SignedMsg order ix"
    },
    {
      "code": 6286,
      "name": "sigVerificationFailed",
      "msg": "SignedMsg message verificaiton failed"
    },
    {
      "code": 6287,
      "name": "mismatchedSignedMsgOrderParamsMarketIndex",
      "msg": "Market index mismatched b/w taker and maker SignedMsg order params"
    },
    {
      "code": 6288,
      "name": "invalidSignedMsgOrderParam",
      "msg": "Invalid SignedMsg order param"
    },
    {
      "code": 6289,
      "name": "placeAndTakeOrderSuccessConditionFailed",
      "msg": "Place and take order success condition failed"
    },
    {
      "code": 6290,
      "name": "invalidHighLeverageModeConfig",
      "msg": "Invalid High Leverage Mode Config"
    },
    {
      "code": 6291,
      "name": "invalidRfqUserAccount",
      "msg": "Invalid RFQ User Account"
    },
    {
      "code": 6292,
      "name": "rfqUserAccountWrongMutability",
      "msg": "RFQUserAccount should be mutable"
    },
    {
      "code": 6293,
      "name": "rfqUserAccountFull",
      "msg": "RFQUserAccount has too many active RFQs"
    },
    {
      "code": 6294,
      "name": "rfqOrderNotFilled",
      "msg": "RFQ order not filled as expected"
    },
    {
      "code": 6295,
      "name": "invalidRfqOrder",
      "msg": "RFQ orders must be jit makers"
    },
    {
      "code": 6296,
      "name": "invalidRfqMatch",
      "msg": "RFQ matches must be valid"
    },
    {
      "code": 6297,
      "name": "invalidSignedMsgUserAccount",
      "msg": "Invalid SignedMsg user account"
    },
    {
      "code": 6298,
      "name": "signedMsgUserAccountWrongMutability",
      "msg": "SignedMsg account wrong mutability"
    },
    {
      "code": 6299,
      "name": "signedMsgUserOrdersAccountFull",
      "msg": "SignedMsgUserAccount has too many active orders"
    },
    {
      "code": 6300,
      "name": "signedMsgOrderDoesNotExist",
      "msg": "Order with SignedMsg uuid does not exist"
    },
    {
      "code": 6301,
      "name": "invalidSignedMsgOrderId",
      "msg": "SignedMsg order id cannot be 0s"
    },
    {
      "code": 6302,
      "name": "invalidPoolId",
      "msg": "Invalid pool id"
    },
    {
      "code": 6303,
      "name": "invalidProtectedMakerModeConfig",
      "msg": "Invalid Protected Maker Mode Config"
    },
    {
      "code": 6304,
      "name": "invalidPythLazerStorageOwner",
      "msg": "Invalid pyth lazer storage owner"
    },
    {
      "code": 6305,
      "name": "unverifiedPythLazerMessage",
      "msg": "Verification of pyth lazer message failed"
    },
    {
      "code": 6306,
      "name": "invalidPythLazerMessage",
      "msg": "Invalid pyth lazer message"
    },
    {
      "code": 6307,
      "name": "pythLazerMessagePriceFeedMismatch",
      "msg": "Pyth lazer message does not correspond to correct fed id"
    },
    {
      "code": 6308,
      "name": "invalidLiquidateSpotWithSwap",
      "msg": "invalidLiquidateSpotWithSwap"
    },
    {
      "code": 6309,
      "name": "signedMsgUserContextUserMismatch",
      "msg": "User in SignedMsg message does not match user in ix context"
    },
    {
      "code": 6310,
      "name": "userFuelOverflowThresholdNotMet",
      "msg": "User fuel overflow threshold not met"
    },
    {
      "code": 6311,
      "name": "fuelOverflowAccountNotFound",
      "msg": "FuelOverflow account not found"
    },
    {
      "code": 6312,
      "name": "invalidTransferPerpPosition",
      "msg": "Invalid Transfer Perp Position"
    },
    {
      "code": 6313,
      "name": "invalidSignedMsgUserOrdersResize",
      "msg": "Invalid SignedMsgUserOrders resize"
    }
  ],
  "types": [
    {
      "name": "updatePerpMarketSummaryStatsParams",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "quoteAssetAmountWithUnsettledLp",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "netUnsettledFundingPnl",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "updateAmmSummaryStats",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "excludeTotalLiqFee",
            "type": {
              "option": "bool"
            }
          }
        ]
      }
    },
    {
      "name": "liquidatePerpRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "oraclePrice",
            "type": "i64"
          },
          {
            "name": "baseAssetAmount",
            "type": "i64"
          },
          {
            "name": "quoteAssetAmount",
            "type": "i64"
          },
          {
            "name": "lpShares",
            "docs": [
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "fillRecordId",
            "type": "u64"
          },
          {
            "name": "userOrderId",
            "type": "u32"
          },
          {
            "name": "liquidatorOrderId",
            "type": "u32"
          },
          {
            "name": "liquidatorFee",
            "docs": [
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "ifFee",
            "docs": [
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "liquidateSpotRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetMarketIndex",
            "type": "u16"
          },
          {
            "name": "assetPrice",
            "type": "i64"
          },
          {
            "name": "assetTransfer",
            "type": "u128"
          },
          {
            "name": "liabilityMarketIndex",
            "type": "u16"
          },
          {
            "name": "liabilityPrice",
            "type": "i64"
          },
          {
            "name": "liabilityTransfer",
            "docs": [
              "precision: token mint precision"
            ],
            "type": "u128"
          },
          {
            "name": "ifFee",
            "docs": [
              "precision: token mint precision"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "liquidateBorrowForPerpPnlRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "perpMarketIndex",
            "type": "u16"
          },
          {
            "name": "marketOraclePrice",
            "type": "i64"
          },
          {
            "name": "pnlTransfer",
            "type": "u128"
          },
          {
            "name": "liabilityMarketIndex",
            "type": "u16"
          },
          {
            "name": "liabilityPrice",
            "type": "i64"
          },
          {
            "name": "liabilityTransfer",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "liquidatePerpPnlForDepositRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "perpMarketIndex",
            "type": "u16"
          },
          {
            "name": "marketOraclePrice",
            "type": "i64"
          },
          {
            "name": "pnlTransfer",
            "type": "u128"
          },
          {
            "name": "assetMarketIndex",
            "type": "u16"
          },
          {
            "name": "assetPrice",
            "type": "i64"
          },
          {
            "name": "assetTransfer",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "perpBankruptcyRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "pnl",
            "type": "i128"
          },
          {
            "name": "ifPayment",
            "type": "u128"
          },
          {
            "name": "clawbackUser",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "clawbackUserPayment",
            "type": {
              "option": "u128"
            }
          },
          {
            "name": "cumulativeFundingRateDelta",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "spotBankruptcyRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "borrowAmount",
            "type": "u128"
          },
          {
            "name": "ifPayment",
            "type": "u128"
          },
          {
            "name": "cumulativeDepositInterestDelta",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "marketIdentifier",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketType",
            "type": {
              "defined": {
                "name": "marketType"
              }
            }
          },
          {
            "name": "marketIndex",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "historicalOracleData",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lastOraclePrice",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastOracleConf",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastOracleDelay",
            "docs": [
              "number of slots since last update"
            ],
            "type": "i64"
          },
          {
            "name": "lastOraclePriceTwap",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastOraclePriceTwap5min",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastOraclePriceTwapTs",
            "docs": [
              "unix_timestamp of last snapshot"
            ],
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "historicalIndexData",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lastIndexBidPrice",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastIndexAskPrice",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastIndexPriceTwap",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastIndexPriceTwap5min",
            "docs": [
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastIndexPriceTwapTs",
            "docs": [
              "unix_timestamp of last snapshot"
            ],
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "prelaunchOracleParams",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "perpMarketIndex",
            "type": "u16"
          },
          {
            "name": "price",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "maxPrice",
            "type": {
              "option": "i64"
            }
          }
        ]
      }
    },
    {
      "name": "orderParams",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "orderType",
            "type": {
              "defined": {
                "name": "orderType"
              }
            }
          },
          {
            "name": "marketType",
            "type": {
              "defined": {
                "name": "marketType"
              }
            }
          },
          {
            "name": "direction",
            "type": {
              "defined": {
                "name": "positionDirection"
              }
            }
          },
          {
            "name": "userOrderId",
            "type": "u8"
          },
          {
            "name": "baseAssetAmount",
            "type": "u64"
          },
          {
            "name": "price",
            "type": "u64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "reduceOnly",
            "type": "bool"
          },
          {
            "name": "postOnly",
            "type": {
              "defined": {
                "name": "postOnlyParam"
              }
            }
          },
          {
            "name": "immediateOrCancel",
            "type": "bool"
          },
          {
            "name": "maxTs",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "triggerPrice",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "triggerCondition",
            "type": {
              "defined": {
                "name": "orderTriggerCondition"
              }
            }
          },
          {
            "name": "oraclePriceOffset",
            "type": {
              "option": "i32"
            }
          },
          {
            "name": "auctionDuration",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "auctionStartPrice",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "auctionEndPrice",
            "type": {
              "option": "i64"
            }
          }
        ]
      }
    },
    {
      "name": "signedMsgOrderParamsMessage",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "signedMsgOrderParams",
            "type": {
              "defined": {
                "name": "orderParams"
              }
            }
          },
          {
            "name": "subAccountId",
            "type": "u16"
          },
          {
            "name": "slot",
            "type": "u64"
          },
          {
            "name": "uuid",
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "takeProfitOrderParams",
            "type": {
              "option": {
                "defined": {
                  "name": "signedMsgTriggerOrderParams"
                }
              }
            }
          },
          {
            "name": "stopLossOrderParams",
            "type": {
              "option": {
                "defined": {
                  "name": "signedMsgTriggerOrderParams"
                }
              }
            }
          }
        ]
      }
    },
    {
      "name": "signedMsgTriggerOrderParams",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "triggerPrice",
            "type": "u64"
          },
          {
            "name": "baseAssetAmount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "modifyOrderParams",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "direction",
            "type": {
              "option": {
                "defined": {
                  "name": "positionDirection"
                }
              }
            }
          },
          {
            "name": "baseAssetAmount",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "price",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "reduceOnly",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "postOnly",
            "type": {
              "option": {
                "defined": {
                  "name": "postOnlyParam"
                }
              }
            }
          },
          {
            "name": "immediateOrCancel",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "maxTs",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "triggerPrice",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "triggerCondition",
            "type": {
              "option": {
                "defined": {
                  "name": "orderTriggerCondition"
                }
              }
            }
          },
          {
            "name": "oraclePriceOffset",
            "type": {
              "option": "i32"
            }
          },
          {
            "name": "auctionDuration",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "auctionStartPrice",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "auctionEndPrice",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "policy",
            "type": {
              "option": "u8"
            }
          }
        ]
      }
    },
    {
      "name": "insuranceClaim",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "revenueWithdrawSinceLastSettle",
            "docs": [
              "The amount of revenue last settled",
              "Positive if funds left the perp market,",
              "negative if funds were pulled into the perp market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "maxRevenueWithdrawPerPeriod",
            "docs": [
              "The max amount of revenue that can be withdrawn per period",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "quoteMaxInsurance",
            "docs": [
              "The max amount of insurance that perp market can use to resolve bankruptcy and pnl deficits",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "quoteSettledInsurance",
            "docs": [
              "The amount of insurance that has been used to resolve bankruptcy and pnl deficits",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastRevenueWithdrawTs",
            "docs": [
              "The last time revenue was settled in/out of market"
            ],
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "poolBalance",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "scaledBalance",
            "docs": [
              "To get the pool's token amount, you must multiply the scaled balance by the market's cumulative",
              "deposit interest",
              "precision: SPOT_BALANCE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "marketIndex",
            "docs": [
              "The spot market the pool is for"
            ],
            "type": "u16"
          },
          {
            "name": "padding",
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
      "name": "amm",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "docs": [
              "oracle price data public key"
            ],
            "type": "pubkey"
          },
          {
            "name": "historicalOracleData",
            "docs": [
              "stores historically witnessed oracle data"
            ],
            "type": {
              "defined": {
                "name": "historicalOracleData"
              }
            }
          },
          {
            "name": "baseAssetAmountPerLp",
            "docs": [
              "accumulated base asset amount since inception per lp share",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "quoteAssetAmountPerLp",
            "docs": [
              "accumulated quote asset amount since inception per lp share",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "feePool",
            "docs": [
              "partition of fees from perp market trading moved from pnl settlements"
            ],
            "type": {
              "defined": {
                "name": "poolBalance"
              }
            }
          },
          {
            "name": "baseAssetReserve",
            "docs": [
              "`x` reserves for constant product mm formula (x * y = k)",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "quoteAssetReserve",
            "docs": [
              "`y` reserves for constant product mm formula (x * y = k)",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "concentrationCoef",
            "docs": [
              "determines how close the min/max base asset reserve sit vs base reserves",
              "allow for decreasing slippage without increasing liquidity and v.v.",
              "precision: PERCENTAGE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "minBaseAssetReserve",
            "docs": [
              "minimum base_asset_reserve allowed before AMM is unavailable",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "maxBaseAssetReserve",
            "docs": [
              "maximum base_asset_reserve allowed before AMM is unavailable",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "sqrtK",
            "docs": [
              "`sqrt(k)` in constant product mm formula (x * y = k). stored to avoid drift caused by integer math issues",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "pegMultiplier",
            "docs": [
              "normalizing numerical factor for y, its use offers lowest slippage in cp-curve when market is balanced",
              "precision: PEG_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "terminalQuoteAssetReserve",
            "docs": [
              "y when market is balanced. stored to save computation",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "baseAssetAmountLong",
            "docs": [
              "always non-negative. tracks number of total longs in market (regardless of counterparty)",
              "precision: BASE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "baseAssetAmountShort",
            "docs": [
              "always non-positive. tracks number of total shorts in market (regardless of counterparty)",
              "precision: BASE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "baseAssetAmountWithAmm",
            "docs": [
              "tracks net position (longs-shorts) in market with AMM as counterparty",
              "precision: BASE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "baseAssetAmountWithUnsettledLp",
            "docs": [
              "tracks net position (longs-shorts) in market with LPs as counterparty",
              "precision: BASE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "maxOpenInterest",
            "docs": [
              "max allowed open interest, blocks trades that breach this value",
              "precision: BASE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "quoteAssetAmount",
            "docs": [
              "sum of all user's perp quote_asset_amount in market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "quoteEntryAmountLong",
            "docs": [
              "sum of all long user's quote_entry_amount in market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "quoteEntryAmountShort",
            "docs": [
              "sum of all short user's quote_entry_amount in market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "quoteBreakEvenAmountLong",
            "docs": [
              "sum of all long user's quote_break_even_amount in market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "quoteBreakEvenAmountShort",
            "docs": [
              "sum of all short user's quote_break_even_amount in market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "userLpShares",
            "docs": [
              "total user lp shares of sqrt_k (protocol owned liquidity = sqrt_k - last_funding_rate)",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "lastFundingRate",
            "docs": [
              "last funding rate in this perp market (unit is quote per base)",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastFundingRateLong",
            "docs": [
              "last funding rate for longs in this perp market (unit is quote per base)",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastFundingRateShort",
            "docs": [
              "last funding rate for shorts in this perp market (unit is quote per base)",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "last24hAvgFundingRate",
            "docs": [
              "estimate of last 24h of funding rate perp market (unit is quote per base)",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "totalFee",
            "docs": [
              "total fees collected by this perp market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "totalMmFee",
            "docs": [
              "total fees collected by the vAMM's bid/ask spread",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "totalExchangeFee",
            "docs": [
              "total fees collected by exchange fee schedule",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "totalFeeMinusDistributions",
            "docs": [
              "total fees minus any recognized upnl and pool withdraws",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i128"
          },
          {
            "name": "totalFeeWithdrawn",
            "docs": [
              "sum of all fees from fee pool withdrawn to revenue pool",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "totalLiquidationFee",
            "docs": [
              "all fees collected by market for liquidations",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "cumulativeFundingRateLong",
            "docs": [
              "accumulated funding rate for longs since inception in market"
            ],
            "type": "i128"
          },
          {
            "name": "cumulativeFundingRateShort",
            "docs": [
              "accumulated funding rate for shorts since inception in market"
            ],
            "type": "i128"
          },
          {
            "name": "totalSocialLoss",
            "docs": [
              "accumulated social loss paid by users since inception in market"
            ],
            "type": "u128"
          },
          {
            "name": "askBaseAssetReserve",
            "docs": [
              "transformed base_asset_reserve for users going long",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "askQuoteAssetReserve",
            "docs": [
              "transformed quote_asset_reserve for users going long",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "bidBaseAssetReserve",
            "docs": [
              "transformed base_asset_reserve for users going short",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "bidQuoteAssetReserve",
            "docs": [
              "transformed quote_asset_reserve for users going short",
              "precision: AMM_RESERVE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "lastOracleNormalisedPrice",
            "docs": [
              "the last seen oracle price partially shrunk toward the amm reserve price",
              "precision: PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastOracleReservePriceSpreadPct",
            "docs": [
              "the gap between the oracle price and the reserve price = y * peg_multiplier / x"
            ],
            "type": "i64"
          },
          {
            "name": "lastBidPriceTwap",
            "docs": [
              "average estimate of bid price over funding_period",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastAskPriceTwap",
            "docs": [
              "average estimate of ask price over funding_period",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastMarkPriceTwap",
            "docs": [
              "average estimate of (bid+ask)/2 price over funding_period",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastMarkPriceTwap5min",
            "docs": [
              "average estimate of (bid+ask)/2 price over FIVE_MINUTES"
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdateSlot",
            "docs": [
              "the last blockchain slot the amm was updated"
            ],
            "type": "u64"
          },
          {
            "name": "lastOracleConfPct",
            "docs": [
              "the pct size of the oracle confidence interval",
              "precision: PERCENTAGE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "netRevenueSinceLastFunding",
            "docs": [
              "the total_fee_minus_distribution change since the last funding update",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastFundingRateTs",
            "docs": [
              "the last funding rate update unix_timestamp"
            ],
            "type": "i64"
          },
          {
            "name": "fundingPeriod",
            "docs": [
              "the peridocity of the funding rate updates"
            ],
            "type": "i64"
          },
          {
            "name": "orderStepSize",
            "docs": [
              "the base step size (increment) of orders",
              "precision: BASE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "orderTickSize",
            "docs": [
              "the price tick size of orders",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "minOrderSize",
            "docs": [
              "the minimum base size of an order",
              "precision: BASE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "maxPositionSize",
            "docs": [
              "the max base size a single user can have",
              "precision: BASE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "volume24h",
            "docs": [
              "estimated total of volume in market",
              "QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "longIntensityVolume",
            "docs": [
              "the volume intensity of long fills against AMM"
            ],
            "type": "u64"
          },
          {
            "name": "shortIntensityVolume",
            "docs": [
              "the volume intensity of short fills against AMM"
            ],
            "type": "u64"
          },
          {
            "name": "lastTradeTs",
            "docs": [
              "the blockchain unix timestamp at the time of the last trade"
            ],
            "type": "i64"
          },
          {
            "name": "markStd",
            "docs": [
              "estimate of standard deviation of the fill (mark) prices",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "oracleStd",
            "docs": [
              "estimate of standard deviation of the oracle price at each update",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastMarkPriceTwapTs",
            "docs": [
              "the last unix_timestamp the mark twap was updated"
            ],
            "type": "i64"
          },
          {
            "name": "baseSpread",
            "docs": [
              "the minimum spread the AMM can quote. also used as step size for some spread logic increases."
            ],
            "type": "u32"
          },
          {
            "name": "maxSpread",
            "docs": [
              "the maximum spread the AMM can quote"
            ],
            "type": "u32"
          },
          {
            "name": "longSpread",
            "docs": [
              "the spread for asks vs the reserve price"
            ],
            "type": "u32"
          },
          {
            "name": "shortSpread",
            "docs": [
              "the spread for bids vs the reserve price"
            ],
            "type": "u32"
          },
          {
            "name": "longIntensityCount",
            "docs": [
              "the count intensity of long fills against AMM"
            ],
            "type": "u32"
          },
          {
            "name": "shortIntensityCount",
            "docs": [
              "the count intensity of short fills against AMM"
            ],
            "type": "u32"
          },
          {
            "name": "maxFillReserveFraction",
            "docs": [
              "the fraction of total available liquidity a single fill on the AMM can consume"
            ],
            "type": "u16"
          },
          {
            "name": "maxSlippageRatio",
            "docs": [
              "the maximum slippage a single fill on the AMM can push"
            ],
            "type": "u16"
          },
          {
            "name": "curveUpdateIntensity",
            "docs": [
              "the update intensity of AMM formulaic updates (adjusting k). 0-100"
            ],
            "type": "u8"
          },
          {
            "name": "ammJitIntensity",
            "docs": [
              "the jit intensity of AMM. larger intensity means larger participation in jit. 0 means no jit participation.",
              "(0, 100] is intensity for protocol-owned AMM. (100, 200] is intensity for user LP-owned AMM."
            ],
            "type": "u8"
          },
          {
            "name": "oracleSource",
            "docs": [
              "the oracle provider information. used to decode/scale the oracle public key"
            ],
            "type": {
              "defined": {
                "name": "oracleSource"
              }
            }
          },
          {
            "name": "lastOracleValid",
            "docs": [
              "tracks whether the oracle was considered valid at the last AMM update"
            ],
            "type": "bool"
          },
          {
            "name": "targetBaseAssetAmountPerLp",
            "docs": [
              "the target value for `base_asset_amount_per_lp`, used during AMM JIT with LP split",
              "precision: BASE_PRECISION"
            ],
            "type": "i32"
          },
          {
            "name": "perLpBase",
            "docs": [
              "expo for unit of per_lp, base 10 (if per_lp_base=X, then per_lp unit is 10^X)"
            ],
            "type": "i8"
          },
          {
            "name": "padding1",
            "type": "u8"
          },
          {
            "name": "padding2",
            "type": "u16"
          },
          {
            "name": "totalFeeEarnedPerLp",
            "type": "u64"
          },
          {
            "name": "netUnsettledFundingPnl",
            "type": "i64"
          },
          {
            "name": "quoteAssetAmountWithUnsettledLp",
            "type": "i64"
          },
          {
            "name": "referencePriceOffset",
            "type": "i32"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          }
        ]
      }
    },
    {
      "name": "signedMsgOrderId",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "uuid",
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "maxSlot",
            "type": "u64"
          },
          {
            "name": "orderId",
            "type": "u32"
          },
          {
            "name": "padding",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "signedMsgUserOrdersFixed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "userPubkey",
            "type": "pubkey"
          },
          {
            "name": "padding",
            "type": "u32"
          },
          {
            "name": "len",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "insuranceFund",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "vault",
            "type": "pubkey"
          },
          {
            "name": "totalShares",
            "type": "u128"
          },
          {
            "name": "userShares",
            "type": "u128"
          },
          {
            "name": "sharesBase",
            "type": "u128"
          },
          {
            "name": "unstakingPeriod",
            "type": "i64"
          },
          {
            "name": "lastRevenueSettleTs",
            "type": "i64"
          },
          {
            "name": "revenueSettlePeriod",
            "type": "i64"
          },
          {
            "name": "totalFactor",
            "type": "u32"
          },
          {
            "name": "userFactor",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "oracleGuardRails",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "priceDivergence",
            "type": {
              "defined": {
                "name": "priceDivergenceGuardRails"
              }
            }
          },
          {
            "name": "validity",
            "type": {
              "defined": {
                "name": "validityGuardRails"
              }
            }
          }
        ]
      }
    },
    {
      "name": "priceDivergenceGuardRails",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "markOraclePercentDivergence",
            "type": "u64"
          },
          {
            "name": "oracleTwap5minPercentDivergence",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "validityGuardRails",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "slotsBeforeStaleForAmm",
            "type": "i64"
          },
          {
            "name": "slotsBeforeStaleForMargin",
            "type": "i64"
          },
          {
            "name": "confidenceIntervalMaxSize",
            "type": "u64"
          },
          {
            "name": "tooVolatileRatio",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "feeStructure",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "feeTiers",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "feeTier"
                  }
                },
                10
              ]
            }
          },
          {
            "name": "fillerRewardStructure",
            "type": {
              "defined": {
                "name": "orderFillerRewardStructure"
              }
            }
          },
          {
            "name": "referrerRewardEpochUpperBound",
            "type": "u64"
          },
          {
            "name": "flatFillerFee",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "feeTier",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "feeNumerator",
            "type": "u32"
          },
          {
            "name": "feeDenominator",
            "type": "u32"
          },
          {
            "name": "makerRebateNumerator",
            "type": "u32"
          },
          {
            "name": "makerRebateDenominator",
            "type": "u32"
          },
          {
            "name": "referrerRewardNumerator",
            "type": "u32"
          },
          {
            "name": "referrerRewardDenominator",
            "type": "u32"
          },
          {
            "name": "refereeFeeNumerator",
            "type": "u32"
          },
          {
            "name": "refereeFeeDenominator",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "orderFillerRewardStructure",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "rewardNumerator",
            "type": "u32"
          },
          {
            "name": "rewardDenominator",
            "type": "u32"
          },
          {
            "name": "timeBasedRewardLowerBound",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "userFees",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "totalFeePaid",
            "docs": [
              "Total taker fee paid",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "totalFeeRebate",
            "docs": [
              "Total maker fee rebate",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "totalTokenDiscount",
            "docs": [
              "Total discount from holding token",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "totalRefereeDiscount",
            "docs": [
              "Total discount from being referred",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "totalReferrerReward",
            "docs": [
              "Total reward to referrer",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "currentEpochReferrerReward",
            "docs": [
              "Total reward to referrer this epoch",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "spotPosition",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "scaledBalance",
            "docs": [
              "The scaled balance of the position. To get the token amount, multiply by the cumulative deposit/borrow",
              "interest of corresponding market.",
              "precision: SPOT_BALANCE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "openBids",
            "docs": [
              "How many spot bids the user has open",
              "precision: token mint precision"
            ],
            "type": "i64"
          },
          {
            "name": "openAsks",
            "docs": [
              "How many spot asks the user has open",
              "precision: token mint precision"
            ],
            "type": "i64"
          },
          {
            "name": "cumulativeDeposits",
            "docs": [
              "The cumulative deposits/borrows a user has made into a market",
              "precision: token mint precision"
            ],
            "type": "i64"
          },
          {
            "name": "marketIndex",
            "docs": [
              "The market index of the corresponding spot market"
            ],
            "type": "u16"
          },
          {
            "name": "balanceType",
            "docs": [
              "Whether the position is deposit or borrow"
            ],
            "type": {
              "defined": {
                "name": "spotBalanceType"
              }
            }
          },
          {
            "name": "openOrders",
            "docs": [
              "Number of open orders"
            ],
            "type": "u8"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          }
        ]
      }
    },
    {
      "name": "perpPosition",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lastCumulativeFundingRate",
            "docs": [
              "The perp market's last cumulative funding rate. Used to calculate the funding payment owed to user",
              "precision: FUNDING_RATE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "baseAssetAmount",
            "docs": [
              "the size of the users perp position",
              "precision: BASE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "quoteAssetAmount",
            "docs": [
              "Used to calculate the users pnl. Upon entry, is equal to base_asset_amount * avg entry price - fees",
              "Updated when the user open/closes position or settles pnl. Includes fees/funding",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "quoteBreakEvenAmount",
            "docs": [
              "The amount of quote the user would need to exit their position at to break even",
              "Updated when the user open/closes position or settles pnl. Includes fees/funding",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "quoteEntryAmount",
            "docs": [
              "The amount quote the user entered the position with. Equal to base asset amount * avg entry price",
              "Updated when the user open/closes position. Excludes fees/funding",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "openBids",
            "docs": [
              "The amount of open bids the user has in this perp market",
              "precision: BASE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "openAsks",
            "docs": [
              "The amount of open asks the user has in this perp market",
              "precision: BASE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "settledPnl",
            "docs": [
              "The amount of pnl settled in this market since opening the position",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lpShares",
            "docs": [
              "The number of lp (liquidity provider) shares the user has in this perp market",
              "LP shares allow users to provide liquidity via the AMM",
              "precision: BASE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastBaseAssetAmountPerLp",
            "docs": [
              "The last base asset amount per lp the amm had",
              "Used to settle the users lp position",
              "precision: BASE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "lastQuoteAssetAmountPerLp",
            "docs": [
              "The last quote asset amount per lp the amm had",
              "Used to settle the users lp position",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "remainderBaseAssetAmount",
            "docs": [
              "Settling LP position can lead to a small amount of base asset being left over smaller than step size",
              "This records that remainder so it can be settled later on",
              "precision: BASE_PRECISION"
            ],
            "type": "i32"
          },
          {
            "name": "marketIndex",
            "docs": [
              "The market index for the perp market"
            ],
            "type": "u16"
          },
          {
            "name": "openOrders",
            "docs": [
              "The number of open orders"
            ],
            "type": "u8"
          },
          {
            "name": "perLpBase",
            "type": "i8"
          }
        ]
      }
    },
    {
      "name": "order",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "slot",
            "docs": [
              "The slot the order was placed"
            ],
            "type": "u64"
          },
          {
            "name": "price",
            "docs": [
              "The limit price for the order (can be 0 for market orders)",
              "For orders with an auction, this price isn't used until the auction is complete",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "baseAssetAmount",
            "docs": [
              "The size of the order",
              "precision for perps: BASE_PRECISION",
              "precision for spot: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "baseAssetAmountFilled",
            "docs": [
              "The amount of the order filled",
              "precision for perps: BASE_PRECISION",
              "precision for spot: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "quoteAssetAmountFilled",
            "docs": [
              "The amount of quote filled for the order",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "triggerPrice",
            "docs": [
              "At what price the order will be triggered. Only relevant for trigger orders",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "auctionStartPrice",
            "docs": [
              "The start price for the auction. Only relevant for market/oracle orders",
              "precision: PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "auctionEndPrice",
            "docs": [
              "The end price for the auction. Only relevant for market/oracle orders",
              "precision: PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "maxTs",
            "docs": [
              "The time when the order will expire"
            ],
            "type": "i64"
          },
          {
            "name": "oraclePriceOffset",
            "docs": [
              "If set, the order limit price is the oracle price + this offset",
              "precision: PRICE_PRECISION"
            ],
            "type": "i32"
          },
          {
            "name": "orderId",
            "docs": [
              "The id for the order. Each users has their own order id space"
            ],
            "type": "u32"
          },
          {
            "name": "marketIndex",
            "docs": [
              "The perp/spot market index"
            ],
            "type": "u16"
          },
          {
            "name": "status",
            "docs": [
              "Whether the order is open or unused"
            ],
            "type": {
              "defined": {
                "name": "orderStatus"
              }
            }
          },
          {
            "name": "orderType",
            "docs": [
              "The type of order"
            ],
            "type": {
              "defined": {
                "name": "orderType"
              }
            }
          },
          {
            "name": "marketType",
            "docs": [
              "Whether market is spot or perp"
            ],
            "type": {
              "defined": {
                "name": "marketType"
              }
            }
          },
          {
            "name": "userOrderId",
            "docs": [
              "User generated order id. Can make it easier to place/cancel orders"
            ],
            "type": "u8"
          },
          {
            "name": "existingPositionDirection",
            "docs": [
              "What the users position was when the order was placed"
            ],
            "type": {
              "defined": {
                "name": "positionDirection"
              }
            }
          },
          {
            "name": "direction",
            "docs": [
              "Whether the user is going long or short. LONG = bid, SHORT = ask"
            ],
            "type": {
              "defined": {
                "name": "positionDirection"
              }
            }
          },
          {
            "name": "reduceOnly",
            "docs": [
              "Whether the order is allowed to only reduce position size"
            ],
            "type": "bool"
          },
          {
            "name": "postOnly",
            "docs": [
              "Whether the order must be a maker"
            ],
            "type": "bool"
          },
          {
            "name": "immediateOrCancel",
            "docs": [
              "Whether the order must be canceled the same slot it is placed"
            ],
            "type": "bool"
          },
          {
            "name": "triggerCondition",
            "docs": [
              "Whether the order is triggered above or below the trigger price. Only relevant for trigger orders"
            ],
            "type": {
              "defined": {
                "name": "orderTriggerCondition"
              }
            }
          },
          {
            "name": "auctionDuration",
            "docs": [
              "How many slots the auction lasts"
            ],
            "type": "u8"
          },
          {
            "name": "postedSlotTail",
            "docs": [
              "Last 8 bits of the slot the order was posted on-chain (not order slot for signed msg orders)"
            ],
            "type": "u8"
          },
          {
            "name": "bitFlags",
            "docs": [
              "Bitflags for further classification",
              "0: is_signed_message"
            ],
            "type": "u8"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                1
              ]
            }
          }
        ]
      }
    },
    {
      "name": "swapDirection",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "add"
          },
          {
            "name": "remove"
          }
        ]
      }
    },
    {
      "name": "modifyOrderId",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "userOrderId",
            "fields": [
              "u8"
            ]
          },
          {
            "name": "orderId",
            "fields": [
              "u32"
            ]
          }
        ]
      }
    },
    {
      "name": "positionDirection",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "long"
          },
          {
            "name": "short"
          }
        ]
      }
    },
    {
      "name": "spotFulfillmentType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "serumV3"
          },
          {
            "name": "match"
          },
          {
            "name": "phoenixV1"
          },
          {
            "name": "openbookV2"
          }
        ]
      }
    },
    {
      "name": "swapReduceOnly",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "in"
          },
          {
            "name": "out"
          }
        ]
      }
    },
    {
      "name": "twapPeriod",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "fundingPeriod"
          },
          {
            "name": "fiveMin"
          }
        ]
      }
    },
    {
      "name": "liquidationMultiplierType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "discount"
          },
          {
            "name": "premium"
          }
        ]
      }
    },
    {
      "name": "marginRequirementType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "initial"
          },
          {
            "name": "fill"
          },
          {
            "name": "maintenance"
          }
        ]
      }
    },
    {
      "name": "oracleValidity",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "nonPositive"
          },
          {
            "name": "tooVolatile"
          },
          {
            "name": "tooUncertain"
          },
          {
            "name": "staleForMargin"
          },
          {
            "name": "insufficientDataPoints"
          },
          {
            "name": "staleForAmm"
          },
          {
            "name": "valid"
          }
        ]
      }
    },
    {
      "name": "driftAction",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "updateFunding"
          },
          {
            "name": "settlePnl"
          },
          {
            "name": "triggerOrder"
          },
          {
            "name": "fillOrderMatch"
          },
          {
            "name": "fillOrderAmm"
          },
          {
            "name": "liquidate"
          },
          {
            "name": "marginCalc"
          },
          {
            "name": "updateTwap"
          },
          {
            "name": "updateAmmCurve"
          },
          {
            "name": "oracleOrderPrice"
          }
        ]
      }
    },
    {
      "name": "positionUpdateType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "open"
          },
          {
            "name": "increase"
          },
          {
            "name": "reduce"
          },
          {
            "name": "close"
          },
          {
            "name": "flip"
          }
        ]
      }
    },
    {
      "name": "depositExplanation",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "none"
          },
          {
            "name": "transfer"
          },
          {
            "name": "borrow"
          },
          {
            "name": "repayBorrow"
          }
        ]
      }
    },
    {
      "name": "depositDirection",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "deposit"
          },
          {
            "name": "withdraw"
          }
        ]
      }
    },
    {
      "name": "orderAction",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "place"
          },
          {
            "name": "cancel"
          },
          {
            "name": "fill"
          },
          {
            "name": "trigger"
          },
          {
            "name": "expire"
          }
        ]
      }
    },
    {
      "name": "orderActionExplanation",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "none"
          },
          {
            "name": "insufficientFreeCollateral"
          },
          {
            "name": "oraclePriceBreachedLimitPrice"
          },
          {
            "name": "marketOrderFilledToLimitPrice"
          },
          {
            "name": "orderExpired"
          },
          {
            "name": "liquidation"
          },
          {
            "name": "orderFilledWithAmm"
          },
          {
            "name": "orderFilledWithAmmJit"
          },
          {
            "name": "orderFilledWithMatch"
          },
          {
            "name": "orderFilledWithMatchJit"
          },
          {
            "name": "marketExpired"
          },
          {
            "name": "riskingIncreasingOrder"
          },
          {
            "name": "reduceOnlyOrderIncreasedPosition"
          },
          {
            "name": "orderFillWithSerum"
          },
          {
            "name": "noBorrowLiquidity"
          },
          {
            "name": "orderFillWithPhoenix"
          },
          {
            "name": "orderFilledWithAmmJitLpSplit"
          },
          {
            "name": "orderFilledWithLpJit"
          },
          {
            "name": "deriskLp"
          },
          {
            "name": "orderFilledWithOpenbookV2"
          },
          {
            "name": "transferPerpPosition"
          }
        ]
      }
    },
    {
      "name": "lpAction",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "addLiquidity"
          },
          {
            "name": "removeLiquidity"
          },
          {
            "name": "settleLiquidity"
          },
          {
            "name": "removeLiquidityDerisk"
          }
        ]
      }
    },
    {
      "name": "liquidationType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "liquidatePerp"
          },
          {
            "name": "liquidateSpot"
          },
          {
            "name": "liquidateBorrowForPerpPnl"
          },
          {
            "name": "liquidatePerpPnlForDeposit"
          },
          {
            "name": "perpBankruptcy"
          },
          {
            "name": "spotBankruptcy"
          }
        ]
      }
    },
    {
      "name": "settlePnlExplanation",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "none"
          },
          {
            "name": "expiredPosition"
          }
        ]
      }
    },
    {
      "name": "stakeAction",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "stake"
          },
          {
            "name": "unstakeRequest"
          },
          {
            "name": "unstakeCancelRequest"
          },
          {
            "name": "unstake"
          },
          {
            "name": "unstakeTransfer"
          },
          {
            "name": "stakeTransfer"
          }
        ]
      }
    },
    {
      "name": "fillMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "fill"
          },
          {
            "name": "placeAndMake"
          },
          {
            "name": "placeAndTake",
            "fields": [
              "bool",
              "u8"
            ]
          },
          {
            "name": "liquidation"
          }
        ]
      }
    },
    {
      "name": "perpFulfillmentMethod",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "amm",
            "fields": [
              {
                "option": "u64"
              }
            ]
          },
          {
            "name": "match",
            "fields": [
              "pubkey",
              "u16",
              "u64"
            ]
          }
        ]
      }
    },
    {
      "name": "spotFulfillmentMethod",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "externalMarket"
          },
          {
            "name": "match",
            "fields": [
              "pubkey",
              "u16"
            ]
          }
        ]
      }
    },
    {
      "name": "marginCalculationMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "standard",
            "fields": [
              {
                "name": "trackOpenOrdersFraction",
                "type": "bool"
              }
            ]
          },
          {
            "name": "liquidation",
            "fields": [
              {
                "name": "marketToTrackMarginRequirement",
                "type": {
                  "option": {
                    "defined": {
                      "name": "marketIdentifier"
                    }
                  }
                }
              }
            ]
          }
        ]
      }
    },
    {
      "name": "oracleSource",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "pyth"
          },
          {
            "name": "switchboard"
          },
          {
            "name": "quoteAsset"
          },
          {
            "name": "pyth1K"
          },
          {
            "name": "pyth1M"
          },
          {
            "name": "pythStableCoin"
          },
          {
            "name": "prelaunch"
          },
          {
            "name": "pythPull"
          },
          {
            "name": "pyth1KPull"
          },
          {
            "name": "pyth1MPull"
          },
          {
            "name": "pythStableCoinPull"
          },
          {
            "name": "switchboardOnDemand"
          },
          {
            "name": "pythLazer"
          },
          {
            "name": "pythLazer1K"
          },
          {
            "name": "pythLazer1M"
          },
          {
            "name": "pythLazerStableCoin"
          }
        ]
      }
    },
    {
      "name": "postOnlyParam",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "none"
          },
          {
            "name": "mustPostOnly"
          },
          {
            "name": "tryPostOnly"
          },
          {
            "name": "slide"
          }
        ]
      }
    },
    {
      "name": "modifyOrderPolicy",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "mustModify"
          },
          {
            "name": "excludePreviousFill"
          }
        ]
      }
    },
    {
      "name": "placeAndTakeOrderSuccessCondition",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "partialFill"
          },
          {
            "name": "fullFill"
          }
        ]
      }
    },
    {
      "name": "perpOperation",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "updateFunding"
          },
          {
            "name": "ammFill"
          },
          {
            "name": "fill"
          },
          {
            "name": "settlePnl"
          },
          {
            "name": "settlePnlWithPosition"
          },
          {
            "name": "liquidation"
          },
          {
            "name": "ammImmediateFill"
          }
        ]
      }
    },
    {
      "name": "spotOperation",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "updateCumulativeInterest"
          },
          {
            "name": "fill"
          },
          {
            "name": "deposit"
          },
          {
            "name": "withdraw"
          },
          {
            "name": "liquidation"
          }
        ]
      }
    },
    {
      "name": "insuranceFundOperation",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "init"
          },
          {
            "name": "add"
          },
          {
            "name": "requestRemove"
          },
          {
            "name": "remove"
          }
        ]
      }
    },
    {
      "name": "marketStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "initialized"
          },
          {
            "name": "active"
          },
          {
            "name": "fundingPaused"
          },
          {
            "name": "ammPaused"
          },
          {
            "name": "fillPaused"
          },
          {
            "name": "withdrawPaused"
          },
          {
            "name": "reduceOnly"
          },
          {
            "name": "settlement"
          },
          {
            "name": "delisted"
          }
        ]
      }
    },
    {
      "name": "contractType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "perpetual"
          },
          {
            "name": "future"
          },
          {
            "name": "prediction"
          }
        ]
      }
    },
    {
      "name": "contractTier",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "a"
          },
          {
            "name": "b"
          },
          {
            "name": "c"
          },
          {
            "name": "speculative"
          },
          {
            "name": "highlySpeculative"
          },
          {
            "name": "isolated"
          }
        ]
      }
    },
    {
      "name": "ammLiquiditySplit",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "protocolOwned"
          },
          {
            "name": "lpOwned"
          },
          {
            "name": "shared"
          }
        ]
      }
    },
    {
      "name": "ammAvailability",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "immediate"
          },
          {
            "name": "afterMinDuration"
          },
          {
            "name": "unavailable"
          }
        ]
      }
    },
    {
      "name": "settlePnlMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "mustSettle"
          },
          {
            "name": "trySettle"
          }
        ]
      }
    },
    {
      "name": "spotBalanceType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "deposit"
          },
          {
            "name": "borrow"
          }
        ]
      }
    },
    {
      "name": "spotFulfillmentConfigStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "enabled"
          },
          {
            "name": "disabled"
          }
        ]
      }
    },
    {
      "name": "assetTier",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "collateral"
          },
          {
            "name": "protected"
          },
          {
            "name": "cross"
          },
          {
            "name": "isolated"
          },
          {
            "name": "unlisted"
          }
        ]
      }
    },
    {
      "name": "exchangeStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "depositPaused"
          },
          {
            "name": "withdrawPaused"
          },
          {
            "name": "ammPaused"
          },
          {
            "name": "fillPaused"
          },
          {
            "name": "liqPaused"
          },
          {
            "name": "fundingPaused"
          },
          {
            "name": "settlePnlPaused"
          },
          {
            "name": "ammImmediateFillPaused"
          }
        ]
      }
    },
    {
      "name": "userStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "beingLiquidated"
          },
          {
            "name": "bankrupt"
          },
          {
            "name": "reduceOnly"
          },
          {
            "name": "advancedLp"
          },
          {
            "name": "protectedMakerOrders"
          }
        ]
      }
    },
    {
      "name": "assetType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "base"
          },
          {
            "name": "quote"
          }
        ]
      }
    },
    {
      "name": "orderStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "init"
          },
          {
            "name": "open"
          },
          {
            "name": "filled"
          },
          {
            "name": "canceled"
          }
        ]
      }
    },
    {
      "name": "orderType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "market"
          },
          {
            "name": "limit"
          },
          {
            "name": "triggerMarket"
          },
          {
            "name": "triggerLimit"
          },
          {
            "name": "oracle"
          }
        ]
      }
    },
    {
      "name": "orderTriggerCondition",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "above"
          },
          {
            "name": "below"
          },
          {
            "name": "triggeredAbove"
          },
          {
            "name": "triggeredBelow"
          }
        ]
      }
    },
    {
      "name": "marketType",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "spot"
          },
          {
            "name": "perp"
          }
        ]
      }
    },
    {
      "name": "referrerStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "isReferrer"
          },
          {
            "name": "isReferred"
          }
        ]
      }
    },
    {
      "name": "marginMode",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "default"
          },
          {
            "name": "highLeverage"
          }
        ]
      }
    },
    {
      "name": "fuelOverflowStatus",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "exists"
          }
        ]
      }
    },
    {
      "name": "signatureVerificationError",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "invalidEd25519InstructionProgramId"
          },
          {
            "name": "invalidEd25519InstructionDataLength"
          },
          {
            "name": "invalidSignatureIndex"
          },
          {
            "name": "invalidSignatureOffset"
          },
          {
            "name": "invalidPublicKeyOffset"
          },
          {
            "name": "invalidMessageOffset"
          },
          {
            "name": "invalidMessageDataSize"
          },
          {
            "name": "invalidInstructionIndex"
          },
          {
            "name": "messageOffsetOverflow"
          },
          {
            "name": "invalidMessageHex"
          },
          {
            "name": "invalidMessageData"
          },
          {
            "name": "loadInstructionAtFailed"
          }
        ]
      }
    },
    {
      "name": "openbookV2FulfillmentConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pubkey",
            "type": "pubkey"
          },
          {
            "name": "openbookV2ProgramId",
            "type": "pubkey"
          },
          {
            "name": "openbookV2Market",
            "type": "pubkey"
          },
          {
            "name": "openbookV2MarketAuthority",
            "type": "pubkey"
          },
          {
            "name": "openbookV2EventHeap",
            "type": "pubkey"
          },
          {
            "name": "openbookV2Bids",
            "type": "pubkey"
          },
          {
            "name": "openbookV2Asks",
            "type": "pubkey"
          },
          {
            "name": "openbookV2BaseVault",
            "type": "pubkey"
          },
          {
            "name": "openbookV2QuoteVault",
            "type": "pubkey"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "fulfillmentType",
            "type": {
              "defined": {
                "name": "spotFulfillmentType"
              }
            }
          },
          {
            "name": "status",
            "type": {
              "defined": {
                "name": "spotFulfillmentConfigStatus"
              }
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          }
        ]
      }
    },
    {
      "name": "phoenixV1FulfillmentConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pubkey",
            "type": "pubkey"
          },
          {
            "name": "phoenixProgramId",
            "type": "pubkey"
          },
          {
            "name": "phoenixLogAuthority",
            "type": "pubkey"
          },
          {
            "name": "phoenixMarket",
            "type": "pubkey"
          },
          {
            "name": "phoenixBaseVault",
            "type": "pubkey"
          },
          {
            "name": "phoenixQuoteVault",
            "type": "pubkey"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "fulfillmentType",
            "type": {
              "defined": {
                "name": "spotFulfillmentType"
              }
            }
          },
          {
            "name": "status",
            "type": {
              "defined": {
                "name": "spotFulfillmentConfigStatus"
              }
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          }
        ]
      }
    },
    {
      "name": "serumV3FulfillmentConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pubkey",
            "type": "pubkey"
          },
          {
            "name": "serumProgramId",
            "type": "pubkey"
          },
          {
            "name": "serumMarket",
            "type": "pubkey"
          },
          {
            "name": "serumRequestQueue",
            "type": "pubkey"
          },
          {
            "name": "serumEventQueue",
            "type": "pubkey"
          },
          {
            "name": "serumBids",
            "type": "pubkey"
          },
          {
            "name": "serumAsks",
            "type": "pubkey"
          },
          {
            "name": "serumBaseVault",
            "type": "pubkey"
          },
          {
            "name": "serumQuoteVault",
            "type": "pubkey"
          },
          {
            "name": "serumOpenOrders",
            "type": "pubkey"
          },
          {
            "name": "serumSignerNonce",
            "type": "u64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "fulfillmentType",
            "type": {
              "defined": {
                "name": "spotFulfillmentType"
              }
            }
          },
          {
            "name": "status",
            "type": {
              "defined": {
                "name": "spotFulfillmentConfigStatus"
              }
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          }
        ]
      }
    },
    {
      "name": "highLeverageModeConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "maxUsers",
            "type": "u32"
          },
          {
            "name": "currentUsers",
            "type": "u32"
          },
          {
            "name": "reduceOnly",
            "type": "u8"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                31
              ]
            }
          }
        ]
      }
    },
    {
      "name": "insuranceFundStake",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "ifShares",
            "type": "u128"
          },
          {
            "name": "lastWithdrawRequestShares",
            "type": "u128"
          },
          {
            "name": "ifBase",
            "type": "u128"
          },
          {
            "name": "lastValidTs",
            "type": "i64"
          },
          {
            "name": "lastWithdrawRequestValue",
            "type": "u64"
          },
          {
            "name": "lastWithdrawRequestTs",
            "type": "i64"
          },
          {
            "name": "costBasis",
            "type": "i64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                14
              ]
            }
          }
        ]
      }
    },
    {
      "name": "protocolIfSharesTransferConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "whitelistedSigners",
            "type": {
              "array": [
                "pubkey",
                4
              ]
            }
          },
          {
            "name": "maxTransferPerEpoch",
            "type": "u128"
          },
          {
            "name": "currentEpochTransfer",
            "type": "u128"
          },
          {
            "name": "nextEpochTs",
            "type": "i64"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u128",
                8
              ]
            }
          }
        ]
      }
    },
    {
      "name": "prelaunchOracle",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "price",
            "type": "i64"
          },
          {
            "name": "maxPrice",
            "type": "i64"
          },
          {
            "name": "confidence",
            "type": "u64"
          },
          {
            "name": "lastUpdateSlot",
            "type": "u64"
          },
          {
            "name": "ammLastUpdateSlot",
            "type": "u64"
          },
          {
            "name": "perpMarketIndex",
            "type": "u16"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                70
              ]
            }
          }
        ]
      }
    },
    {
      "name": "perpMarket",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pubkey",
            "docs": [
              "The perp market's address. It is a pda of the market index"
            ],
            "type": "pubkey"
          },
          {
            "name": "amm",
            "docs": [
              "The automated market maker"
            ],
            "type": {
              "defined": {
                "name": "amm"
              }
            }
          },
          {
            "name": "pnlPool",
            "docs": [
              "The market's pnl pool. When users settle negative pnl, the balance increases.",
              "When users settle positive pnl, the balance decreases. Can not go negative."
            ],
            "type": {
              "defined": {
                "name": "poolBalance"
              }
            }
          },
          {
            "name": "name",
            "docs": [
              "Encoded display name for the perp market e.g. SOL-PERP"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "insuranceClaim",
            "docs": [
              "The perp market's claim on the insurance fund"
            ],
            "type": {
              "defined": {
                "name": "insuranceClaim"
              }
            }
          },
          {
            "name": "unrealizedPnlMaxImbalance",
            "docs": [
              "The max pnl imbalance before positive pnl asset weight is discounted",
              "pnl imbalance is the difference between long and short pnl. When it's greater than 0,",
              "the amm has negative pnl and the initial asset weight for positive pnl is discounted",
              "precision = QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "expiryTs",
            "docs": [
              "The ts when the market will be expired. Only set if market is in reduce only mode"
            ],
            "type": "i64"
          },
          {
            "name": "expiryPrice",
            "docs": [
              "The price at which positions will be settled. Only set if market is expired",
              "precision = PRICE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "nextFillRecordId",
            "docs": [
              "Every trade has a fill record id. This is the next id to be used"
            ],
            "type": "u64"
          },
          {
            "name": "nextFundingRateRecordId",
            "docs": [
              "Every funding rate update has a record id. This is the next id to be used"
            ],
            "type": "u64"
          },
          {
            "name": "nextCurveRecordId",
            "docs": [
              "Every amm k updated has a record id. This is the next id to be used"
            ],
            "type": "u64"
          },
          {
            "name": "imfFactor",
            "docs": [
              "The initial margin fraction factor. Used to increase margin ratio for large positions",
              "precision: MARGIN_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "unrealizedPnlImfFactor",
            "docs": [
              "The imf factor for unrealized pnl. Used to discount asset weight for large positive pnl",
              "precision: MARGIN_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "liquidatorFee",
            "docs": [
              "The fee the liquidator is paid for taking over perp position",
              "precision: LIQUIDATOR_FEE_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "ifLiquidationFee",
            "docs": [
              "The fee the insurance fund receives from liquidation",
              "precision: LIQUIDATOR_FEE_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "marginRatioInitial",
            "docs": [
              "The margin ratio which determines how much collateral is required to open a position",
              "e.g. margin ratio of .1 means a user must have $100 of total collateral to open a $1000 position",
              "precision: MARGIN_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "marginRatioMaintenance",
            "docs": [
              "The margin ratio which determines when a user will be liquidated",
              "e.g. margin ratio of .05 means a user must have $50 of total collateral to maintain a $1000 position",
              "else they will be liquidated",
              "precision: MARGIN_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "unrealizedPnlInitialAssetWeight",
            "docs": [
              "The initial asset weight for positive pnl. Negative pnl always has an asset weight of 1",
              "precision: SPOT_WEIGHT_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "unrealizedPnlMaintenanceAssetWeight",
            "docs": [
              "The maintenance asset weight for positive pnl. Negative pnl always has an asset weight of 1",
              "precision: SPOT_WEIGHT_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "numberOfUsersWithBase",
            "docs": [
              "number of users in a position (base)"
            ],
            "type": "u32"
          },
          {
            "name": "numberOfUsers",
            "docs": [
              "number of users in a position (pnl) or pnl (quote)"
            ],
            "type": "u32"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "status",
            "docs": [
              "Whether a market is active, reduce only, expired, etc",
              "Affects whether users can open/close positions"
            ],
            "type": {
              "defined": {
                "name": "marketStatus"
              }
            }
          },
          {
            "name": "contractType",
            "docs": [
              "Currently only Perpetual markets are supported"
            ],
            "type": {
              "defined": {
                "name": "contractType"
              }
            }
          },
          {
            "name": "contractTier",
            "docs": [
              "The contract tier determines how much insurance a market can receive, with more speculative markets receiving less insurance",
              "It also influences the order perp markets can be liquidated, with less speculative markets being liquidated first"
            ],
            "type": {
              "defined": {
                "name": "contractTier"
              }
            }
          },
          {
            "name": "pausedOperations",
            "type": "u8"
          },
          {
            "name": "quoteSpotMarketIndex",
            "docs": [
              "The spot market that pnl is settled in"
            ],
            "type": "u16"
          },
          {
            "name": "feeAdjustment",
            "docs": [
              "Between -100 and 100, represents what % to increase/decrease the fee by",
              "E.g. if this is -50 and the fee is 5bps, the new fee will be 2.5bps",
              "if this is 50 and the fee is 5bps, the new fee will be 7.5bps"
            ],
            "type": "i16"
          },
          {
            "name": "fuelBoostPosition",
            "docs": [
              "fuel multiplier for perp funding",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostTaker",
            "docs": [
              "fuel multiplier for perp taker",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostMaker",
            "docs": [
              "fuel multiplier for perp maker",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "poolId",
            "type": "u8"
          },
          {
            "name": "highLeverageMarginRatioInitial",
            "type": "u16"
          },
          {
            "name": "highLeverageMarginRatioMaintenance",
            "type": "u16"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                38
              ]
            }
          }
        ]
      }
    },
    {
      "name": "protectedMakerModeConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "maxUsers",
            "type": "u32"
          },
          {
            "name": "currentUsers",
            "type": "u32"
          },
          {
            "name": "reduceOnly",
            "type": "u8"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                31
              ]
            }
          }
        ]
      }
    },
    {
      "name": "pythLazerOracle",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "price",
            "type": "i64"
          },
          {
            "name": "publishTime",
            "type": "u64"
          },
          {
            "name": "postedSlot",
            "type": "u64"
          },
          {
            "name": "exponent",
            "type": "i32"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          },
          {
            "name": "conf",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "signedMsgUserOrders",
      "docs": [
        "* This struct is a duplicate of SignedMsgUserOrdersZeroCopy\n * It is used to give anchor an struct to generate the idl for clients\n * The struct SignedMsgUserOrdersZeroCopy is used to load the data in efficiently"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authorityPubkey",
            "type": "pubkey"
          },
          {
            "name": "padding",
            "type": "u32"
          },
          {
            "name": "signedMsgOrderData",
            "type": {
              "vec": {
                "defined": {
                  "name": "signedMsgOrderId"
                }
              }
            }
          }
        ]
      }
    },
    {
      "name": "signedMsgWsDelegates",
      "docs": [
        "* Used to store authenticated delegates for swift-like ws connections"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "delegates",
            "type": {
              "vec": "pubkey"
            }
          }
        ]
      }
    },
    {
      "name": "spotMarket",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pubkey",
            "docs": [
              "The address of the spot market. It is a pda of the market index"
            ],
            "type": "pubkey"
          },
          {
            "name": "oracle",
            "docs": [
              "The oracle used to price the markets deposits/borrows"
            ],
            "type": "pubkey"
          },
          {
            "name": "mint",
            "docs": [
              "The token mint of the market"
            ],
            "type": "pubkey"
          },
          {
            "name": "vault",
            "docs": [
              "The vault used to store the market's deposits",
              "The amount in the vault should be equal to or greater than deposits - borrows"
            ],
            "type": "pubkey"
          },
          {
            "name": "name",
            "docs": [
              "The encoded display name for the market e.g. SOL"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "historicalOracleData",
            "type": {
              "defined": {
                "name": "historicalOracleData"
              }
            }
          },
          {
            "name": "historicalIndexData",
            "type": {
              "defined": {
                "name": "historicalIndexData"
              }
            }
          },
          {
            "name": "revenuePool",
            "docs": [
              "Revenue the protocol has collected in this markets token",
              "e.g. for SOL-PERP, funds can be settled in usdc and will flow into the USDC revenue pool"
            ],
            "type": {
              "defined": {
                "name": "poolBalance"
              }
            }
          },
          {
            "name": "spotFeePool",
            "docs": [
              "The fees collected from swaps between this market and the quote market",
              "Is settled to the quote markets revenue pool"
            ],
            "type": {
              "defined": {
                "name": "poolBalance"
              }
            }
          },
          {
            "name": "insuranceFund",
            "docs": [
              "Details on the insurance fund covering bankruptcies in this markets token",
              "Covers bankruptcies for borrows with this markets token and perps settling in this markets token"
            ],
            "type": {
              "defined": {
                "name": "insuranceFund"
              }
            }
          },
          {
            "name": "totalSpotFee",
            "docs": [
              "The total spot fees collected for this market",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "depositBalance",
            "docs": [
              "The sum of the scaled balances for deposits across users and pool balances",
              "To convert to the deposit token amount, multiply by the cumulative deposit interest",
              "precision: SPOT_BALANCE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "borrowBalance",
            "docs": [
              "The sum of the scaled balances for borrows across users and pool balances",
              "To convert to the borrow token amount, multiply by the cumulative borrow interest",
              "precision: SPOT_BALANCE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "cumulativeDepositInterest",
            "docs": [
              "The cumulative interest earned by depositors",
              "Used to calculate the deposit token amount from the deposit balance",
              "precision: SPOT_CUMULATIVE_INTEREST_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "cumulativeBorrowInterest",
            "docs": [
              "The cumulative interest earned by borrowers",
              "Used to calculate the borrow token amount from the borrow balance",
              "precision: SPOT_CUMULATIVE_INTEREST_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "totalSocialLoss",
            "docs": [
              "The total socialized loss from borrows, in the mint's token",
              "precision: token mint precision"
            ],
            "type": "u128"
          },
          {
            "name": "totalQuoteSocialLoss",
            "docs": [
              "The total socialized loss from borrows, in the quote market's token",
              "preicision: QUOTE_PRECISION"
            ],
            "type": "u128"
          },
          {
            "name": "withdrawGuardThreshold",
            "docs": [
              "no withdraw limits/guards when deposits below this threshold",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "maxTokenDeposits",
            "docs": [
              "The max amount of token deposits in this market",
              "0 if there is no limit",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "depositTokenTwap",
            "docs": [
              "24hr average of deposit token amount",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "borrowTokenTwap",
            "docs": [
              "24hr average of borrow token amount",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "utilizationTwap",
            "docs": [
              "24hr average of utilization",
              "which is borrow amount over token amount",
              "precision: SPOT_UTILIZATION_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastInterestTs",
            "docs": [
              "Last time the cumulative deposit and borrow interest was updated"
            ],
            "type": "u64"
          },
          {
            "name": "lastTwapTs",
            "docs": [
              "Last time the deposit/borrow/utilization averages were updated"
            ],
            "type": "u64"
          },
          {
            "name": "expiryTs",
            "docs": [
              "The time the market is set to expire. Only set if market is in reduce only mode"
            ],
            "type": "i64"
          },
          {
            "name": "orderStepSize",
            "docs": [
              "Spot orders must be a multiple of the step size",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "orderTickSize",
            "docs": [
              "Spot orders must be a multiple of the tick size",
              "precision: PRICE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "minOrderSize",
            "docs": [
              "The minimum order size",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "maxPositionSize",
            "docs": [
              "The maximum spot position size",
              "if the limit is 0, there is no limit",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "nextFillRecordId",
            "docs": [
              "Every spot trade has a fill record id. This is the next id to use"
            ],
            "type": "u64"
          },
          {
            "name": "nextDepositRecordId",
            "docs": [
              "Every deposit has a deposit record id. This is the next id to use"
            ],
            "type": "u64"
          },
          {
            "name": "initialAssetWeight",
            "docs": [
              "The initial asset weight used to calculate a deposits contribution to a users initial total collateral",
              "e.g. if the asset weight is .8, $100 of deposits contributes $80 to the users initial total collateral",
              "precision: SPOT_WEIGHT_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "maintenanceAssetWeight",
            "docs": [
              "The maintenance asset weight used to calculate a deposits contribution to a users maintenance total collateral",
              "e.g. if the asset weight is .9, $100 of deposits contributes $90 to the users maintenance total collateral",
              "precision: SPOT_WEIGHT_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "initialLiabilityWeight",
            "docs": [
              "The initial liability weight used to calculate a borrows contribution to a users initial margin requirement",
              "e.g. if the liability weight is .9, $100 of borrows contributes $90 to the users initial margin requirement",
              "precision: SPOT_WEIGHT_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "maintenanceLiabilityWeight",
            "docs": [
              "The maintenance liability weight used to calculate a borrows contribution to a users maintenance margin requirement",
              "e.g. if the liability weight is .8, $100 of borrows contributes $80 to the users maintenance margin requirement",
              "precision: SPOT_WEIGHT_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "imfFactor",
            "docs": [
              "The initial margin fraction factor. Used to increase liability weight/decrease asset weight for large positions",
              "precision: MARGIN_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "liquidatorFee",
            "docs": [
              "The fee the liquidator is paid for taking over borrow/deposit",
              "precision: LIQUIDATOR_FEE_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "ifLiquidationFee",
            "docs": [
              "The fee the insurance fund receives from liquidation",
              "precision: LIQUIDATOR_FEE_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "optimalUtilization",
            "docs": [
              "The optimal utilization rate for this market.",
              "Used to determine the markets borrow rate",
              "precision: SPOT_UTILIZATION_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "optimalBorrowRate",
            "docs": [
              "The borrow rate for this market when the market has optimal utilization",
              "precision: SPOT_RATE_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "maxBorrowRate",
            "docs": [
              "The borrow rate for this market when the market has 1000 utilization",
              "precision: SPOT_RATE_PRECISION"
            ],
            "type": "u32"
          },
          {
            "name": "decimals",
            "docs": [
              "The market's token mint's decimals. To from decimals to a precision, 10^decimals"
            ],
            "type": "u32"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "ordersEnabled",
            "docs": [
              "Whether or not spot trading is enabled"
            ],
            "type": "bool"
          },
          {
            "name": "oracleSource",
            "type": {
              "defined": {
                "name": "oracleSource"
              }
            }
          },
          {
            "name": "status",
            "type": {
              "defined": {
                "name": "marketStatus"
              }
            }
          },
          {
            "name": "assetTier",
            "docs": [
              "The asset tier affects how a deposit can be used as collateral and the priority for a borrow being liquidated"
            ],
            "type": {
              "defined": {
                "name": "assetTier"
              }
            }
          },
          {
            "name": "pausedOperations",
            "type": "u8"
          },
          {
            "name": "ifPausedOperations",
            "type": "u8"
          },
          {
            "name": "feeAdjustment",
            "type": "i16"
          },
          {
            "name": "maxTokenBorrowsFraction",
            "docs": [
              "What fraction of max_token_deposits",
              "disabled when 0, 1 => 1/10000 => .01% of max_token_deposits",
              "precision: X/10000"
            ],
            "type": "u16"
          },
          {
            "name": "flashLoanAmount",
            "docs": [
              "For swaps, the amount of token loaned out in the begin_swap ix",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "flashLoanInitialTokenAmount",
            "docs": [
              "For swaps, the amount in the users token account in the begin_swap ix",
              "Used to calculate how much of the token left the system in end_swap ix",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "totalSwapFee",
            "docs": [
              "The total fees received from swaps",
              "precision: token mint precision"
            ],
            "type": "u64"
          },
          {
            "name": "scaleInitialAssetWeightStart",
            "docs": [
              "When to begin scaling down the initial asset weight",
              "disabled when 0",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "minBorrowRate",
            "docs": [
              "The min borrow rate for this market when the market regardless of utilization",
              "1 => 1/200 => .5%",
              "precision: X/200"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostDeposits",
            "docs": [
              "fuel multiplier for spot deposits",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostBorrows",
            "docs": [
              "fuel multiplier for spot borrows",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostTaker",
            "docs": [
              "fuel multiplier for spot taker",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostMaker",
            "docs": [
              "fuel multiplier for spot maker",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "fuelBoostInsurance",
            "docs": [
              "fuel multiplier for spot insurance stake",
              "precision: 10"
            ],
            "type": "u8"
          },
          {
            "name": "tokenProgram",
            "type": "u8"
          },
          {
            "name": "poolId",
            "type": "u8"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                40
              ]
            }
          }
        ]
      }
    },
    {
      "name": "state",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "admin",
            "type": "pubkey"
          },
          {
            "name": "whitelistMint",
            "type": "pubkey"
          },
          {
            "name": "discountMint",
            "type": "pubkey"
          },
          {
            "name": "signer",
            "type": "pubkey"
          },
          {
            "name": "srmVault",
            "type": "pubkey"
          },
          {
            "name": "perpFeeStructure",
            "type": {
              "defined": {
                "name": "feeStructure"
              }
            }
          },
          {
            "name": "spotFeeStructure",
            "type": {
              "defined": {
                "name": "feeStructure"
              }
            }
          },
          {
            "name": "oracleGuardRails",
            "type": {
              "defined": {
                "name": "oracleGuardRails"
              }
            }
          },
          {
            "name": "numberOfAuthorities",
            "type": "u64"
          },
          {
            "name": "numberOfSubAccounts",
            "type": "u64"
          },
          {
            "name": "lpCooldownTime",
            "type": "u64"
          },
          {
            "name": "liquidationMarginBufferRatio",
            "type": "u32"
          },
          {
            "name": "settlementDuration",
            "type": "u16"
          },
          {
            "name": "numberOfMarkets",
            "type": "u16"
          },
          {
            "name": "numberOfSpotMarkets",
            "type": "u16"
          },
          {
            "name": "signerNonce",
            "type": "u8"
          },
          {
            "name": "minPerpAuctionDuration",
            "type": "u8"
          },
          {
            "name": "defaultMarketOrderTimeInForce",
            "type": "u8"
          },
          {
            "name": "defaultSpotAuctionDuration",
            "type": "u8"
          },
          {
            "name": "exchangeStatus",
            "type": "u8"
          },
          {
            "name": "liquidationDuration",
            "type": "u8"
          },
          {
            "name": "initialPctToLiquidate",
            "type": "u16"
          },
          {
            "name": "maxNumberOfSubAccounts",
            "type": "u16"
          },
          {
            "name": "maxInitializeUserFee",
            "type": "u16"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                10
              ]
            }
          }
        ]
      }
    },
    {
      "name": "user",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "docs": [
              "The owner/authority of the account"
            ],
            "type": "pubkey"
          },
          {
            "name": "delegate",
            "docs": [
              "An addresses that can control the account on the authority's behalf. Has limited power, cant withdraw"
            ],
            "type": "pubkey"
          },
          {
            "name": "name",
            "docs": [
              "Encoded display name e.g. \"toly\""
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "spotPositions",
            "docs": [
              "The user's spot positions"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "spotPosition"
                  }
                },
                8
              ]
            }
          },
          {
            "name": "perpPositions",
            "docs": [
              "The user's perp positions"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "perpPosition"
                  }
                },
                8
              ]
            }
          },
          {
            "name": "orders",
            "docs": [
              "The user's orders"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "order"
                  }
                },
                32
              ]
            }
          },
          {
            "name": "lastAddPerpLpSharesTs",
            "docs": [
              "The last time the user added perp lp positions"
            ],
            "type": "i64"
          },
          {
            "name": "totalDeposits",
            "docs": [
              "The total values of deposits the user has made",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "totalWithdraws",
            "docs": [
              "The total values of withdrawals the user has made",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "totalSocialLoss",
            "docs": [
              "The total socialized loss the users has incurred upon the protocol",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "settledPerpPnl",
            "docs": [
              "Fees (taker fees, maker rebate, referrer reward, filler reward) and pnl for perps",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "cumulativeSpotFees",
            "docs": [
              "Fees (taker fees, maker rebate, filler reward) for spot",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "cumulativePerpFunding",
            "docs": [
              "Cumulative funding paid/received for perps",
              "precision: QUOTE_PRECISION"
            ],
            "type": "i64"
          },
          {
            "name": "liquidationMarginFreed",
            "docs": [
              "The amount of margin freed during liquidation. Used to force the liquidation to occur over a period of time",
              "Defaults to zero when not being liquidated",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastActiveSlot",
            "docs": [
              "The last slot a user was active. Used to determine if a user is idle"
            ],
            "type": "u64"
          },
          {
            "name": "nextOrderId",
            "docs": [
              "Every user order has an order id. This is the next order id to be used"
            ],
            "type": "u32"
          },
          {
            "name": "maxMarginRatio",
            "docs": [
              "Custom max initial margin ratio for the user"
            ],
            "type": "u32"
          },
          {
            "name": "nextLiquidationId",
            "docs": [
              "The next liquidation id to be used for user"
            ],
            "type": "u16"
          },
          {
            "name": "subAccountId",
            "docs": [
              "The sub account id for this user"
            ],
            "type": "u16"
          },
          {
            "name": "status",
            "docs": [
              "Whether the user is active, being liquidated or bankrupt"
            ],
            "type": "u8"
          },
          {
            "name": "isMarginTradingEnabled",
            "docs": [
              "Whether the user has enabled margin trading"
            ],
            "type": "bool"
          },
          {
            "name": "idle",
            "docs": [
              "User is idle if they haven't interacted with the protocol in 1 week and they have no orders, perp positions or borrows",
              "Off-chain keeper bots can ignore users that are idle"
            ],
            "type": "bool"
          },
          {
            "name": "openOrders",
            "docs": [
              "number of open orders"
            ],
            "type": "u8"
          },
          {
            "name": "hasOpenOrder",
            "docs": [
              "Whether or not user has open order"
            ],
            "type": "bool"
          },
          {
            "name": "openAuctions",
            "docs": [
              "number of open orders with auction"
            ],
            "type": "u8"
          },
          {
            "name": "hasOpenAuction",
            "docs": [
              "Whether or not user has open order with auction"
            ],
            "type": "bool"
          },
          {
            "name": "marginMode",
            "type": {
              "defined": {
                "name": "marginMode"
              }
            }
          },
          {
            "name": "poolId",
            "type": "u8"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u8",
                3
              ]
            }
          },
          {
            "name": "lastFuelBonusUpdateTs",
            "type": "u32"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          }
        ]
      }
    },
    {
      "name": "userStats",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "docs": [
              "The authority for all of a users sub accounts"
            ],
            "type": "pubkey"
          },
          {
            "name": "referrer",
            "docs": [
              "The address that referred this user"
            ],
            "type": "pubkey"
          },
          {
            "name": "fees",
            "docs": [
              "Stats on the fees paid by the user"
            ],
            "type": {
              "defined": {
                "name": "userFees"
              }
            }
          },
          {
            "name": "nextEpochTs",
            "docs": [
              "The timestamp of the next epoch",
              "Epoch is used to limit referrer rewards earned in single epoch"
            ],
            "type": "i64"
          },
          {
            "name": "makerVolume30d",
            "docs": [
              "Rolling 30day maker volume for user",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "takerVolume30d",
            "docs": [
              "Rolling 30day taker volume for user",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "fillerVolume30d",
            "docs": [
              "Rolling 30day filler volume for user",
              "precision: QUOTE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "lastMakerVolume30dTs",
            "docs": [
              "last time the maker volume was updated"
            ],
            "type": "i64"
          },
          {
            "name": "lastTakerVolume30dTs",
            "docs": [
              "last time the taker volume was updated"
            ],
            "type": "i64"
          },
          {
            "name": "lastFillerVolume30dTs",
            "docs": [
              "last time the filler volume was updated"
            ],
            "type": "i64"
          },
          {
            "name": "ifStakedQuoteAssetAmount",
            "docs": [
              "The amount of tokens staked in the quote spot markets if"
            ],
            "type": "u64"
          },
          {
            "name": "numberOfSubAccounts",
            "docs": [
              "The current number of sub accounts"
            ],
            "type": "u16"
          },
          {
            "name": "numberOfSubAccountsCreated",
            "docs": [
              "The number of sub accounts created. Can be greater than the number of sub accounts if user",
              "has deleted sub accounts"
            ],
            "type": "u16"
          },
          {
            "name": "referrerStatus",
            "docs": [
              "Flags for referrer status:",
              "First bit (LSB): 1 if user is a referrer, 0 otherwise",
              "Second bit: 1 if user was referred, 0 otherwise"
            ],
            "type": "u8"
          },
          {
            "name": "disableUpdatePerpBidAskTwap",
            "type": "bool"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u8",
                1
              ]
            }
          },
          {
            "name": "fuelOverflowStatus",
            "docs": [
              "whether the user has a FuelOverflow account"
            ],
            "type": "u8"
          },
          {
            "name": "fuelInsurance",
            "docs": [
              "accumulated fuel for token amounts of insurance"
            ],
            "type": "u32"
          },
          {
            "name": "fuelDeposits",
            "docs": [
              "accumulated fuel for notional of deposits"
            ],
            "type": "u32"
          },
          {
            "name": "fuelBorrows",
            "docs": [
              "accumulate fuel bonus for notional of borrows"
            ],
            "type": "u32"
          },
          {
            "name": "fuelPositions",
            "docs": [
              "accumulated fuel for perp open interest"
            ],
            "type": "u32"
          },
          {
            "name": "fuelTaker",
            "docs": [
              "accumulate fuel bonus for taker volume"
            ],
            "type": "u32"
          },
          {
            "name": "fuelMaker",
            "docs": [
              "accumulate fuel bonus for maker volume"
            ],
            "type": "u32"
          },
          {
            "name": "ifStakedGovTokenAmount",
            "docs": [
              "The amount of tokens staked in the governance spot markets if"
            ],
            "type": "u64"
          },
          {
            "name": "lastFuelIfBonusUpdateTs",
            "docs": [
              "last unix ts user stats data was used to update if fuel (u32 to save space)"
            ],
            "type": "u32"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          }
        ]
      }
    },
    {
      "name": "referrerName",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "userStats",
            "type": "pubkey"
          },
          {
            "name": "name",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          }
        ]
      }
    },
    {
      "name": "fuelOverflow",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "docs": [
              "The authority of this overflow account"
            ],
            "type": "pubkey"
          },
          {
            "name": "fuelInsurance",
            "type": "u128"
          },
          {
            "name": "fuelDeposits",
            "type": "u128"
          },
          {
            "name": "fuelBorrows",
            "type": "u128"
          },
          {
            "name": "fuelPositions",
            "type": "u128"
          },
          {
            "name": "fuelTaker",
            "type": "u128"
          },
          {
            "name": "fuelMaker",
            "type": "u128"
          },
          {
            "name": "lastFuelSweepTs",
            "type": "u32"
          },
          {
            "name": "lastResetTs",
            "type": "u32"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u128",
                6
              ]
            }
          }
        ]
      }
    },
    {
      "name": "newUserRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "userAuthority",
            "type": "pubkey"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "subAccountId",
            "type": "u16"
          },
          {
            "name": "name",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "referrer",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "depositRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "userAuthority",
            "type": "pubkey"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "direction",
            "type": {
              "defined": {
                "name": "depositDirection"
              }
            }
          },
          {
            "name": "depositRecordId",
            "type": "u64"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "oraclePrice",
            "type": "i64"
          },
          {
            "name": "marketDepositBalance",
            "type": "u128"
          },
          {
            "name": "marketWithdrawBalance",
            "type": "u128"
          },
          {
            "name": "marketCumulativeDepositInterest",
            "type": "u128"
          },
          {
            "name": "marketCumulativeBorrowInterest",
            "type": "u128"
          },
          {
            "name": "totalDepositsAfter",
            "type": "u64"
          },
          {
            "name": "totalWithdrawsAfter",
            "type": "u64"
          },
          {
            "name": "explanation",
            "type": {
              "defined": {
                "name": "depositExplanation"
              }
            }
          },
          {
            "name": "transferUser",
            "type": {
              "option": "pubkey"
            }
          }
        ]
      }
    },
    {
      "name": "spotInterestRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "depositBalance",
            "type": "u128"
          },
          {
            "name": "cumulativeDepositInterest",
            "type": "u128"
          },
          {
            "name": "borrowBalance",
            "type": "u128"
          },
          {
            "name": "cumulativeBorrowInterest",
            "type": "u128"
          },
          {
            "name": "optimalUtilization",
            "type": "u32"
          },
          {
            "name": "optimalBorrowRate",
            "type": "u32"
          },
          {
            "name": "maxBorrowRate",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "fundingPaymentRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "userAuthority",
            "type": "pubkey"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "fundingPayment",
            "type": "i64"
          },
          {
            "name": "baseAssetAmount",
            "type": "i64"
          },
          {
            "name": "userLastCumulativeFunding",
            "type": "i64"
          },
          {
            "name": "ammCumulativeFundingLong",
            "type": "i128"
          },
          {
            "name": "ammCumulativeFundingShort",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "fundingRateRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "recordId",
            "type": "u64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "fundingRate",
            "type": "i64"
          },
          {
            "name": "fundingRateLong",
            "type": "i128"
          },
          {
            "name": "fundingRateShort",
            "type": "i128"
          },
          {
            "name": "cumulativeFundingRateLong",
            "type": "i128"
          },
          {
            "name": "cumulativeFundingRateShort",
            "type": "i128"
          },
          {
            "name": "oraclePriceTwap",
            "type": "i64"
          },
          {
            "name": "markPriceTwap",
            "type": "u64"
          },
          {
            "name": "periodRevenue",
            "type": "i64"
          },
          {
            "name": "baseAssetAmountWithAmm",
            "type": "i128"
          },
          {
            "name": "baseAssetAmountWithUnsettledLp",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "curveRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "recordId",
            "type": "u64"
          },
          {
            "name": "pegMultiplierBefore",
            "type": "u128"
          },
          {
            "name": "baseAssetReserveBefore",
            "type": "u128"
          },
          {
            "name": "quoteAssetReserveBefore",
            "type": "u128"
          },
          {
            "name": "sqrtKBefore",
            "type": "u128"
          },
          {
            "name": "pegMultiplierAfter",
            "type": "u128"
          },
          {
            "name": "baseAssetReserveAfter",
            "type": "u128"
          },
          {
            "name": "quoteAssetReserveAfter",
            "type": "u128"
          },
          {
            "name": "sqrtKAfter",
            "type": "u128"
          },
          {
            "name": "baseAssetAmountLong",
            "type": "u128"
          },
          {
            "name": "baseAssetAmountShort",
            "type": "u128"
          },
          {
            "name": "baseAssetAmountWithAmm",
            "type": "i128"
          },
          {
            "name": "totalFee",
            "type": "i128"
          },
          {
            "name": "totalFeeMinusDistributions",
            "type": "i128"
          },
          {
            "name": "adjustmentCost",
            "type": "i128"
          },
          {
            "name": "oraclePrice",
            "type": "i64"
          },
          {
            "name": "fillRecord",
            "type": "u128"
          },
          {
            "name": "numberOfUsers",
            "type": "u32"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "signedMsgOrderRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "hash",
            "type": "string"
          },
          {
            "name": "matchingOrderParams",
            "type": {
              "defined": {
                "name": "orderParams"
              }
            }
          },
          {
            "name": "userOrderId",
            "type": "u32"
          },
          {
            "name": "signedMsgOrderMaxSlot",
            "type": "u64"
          },
          {
            "name": "signedMsgOrderUuid",
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "ts",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "orderRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "order",
            "type": {
              "defined": {
                "name": "order"
              }
            }
          }
        ]
      }
    },
    {
      "name": "orderActionRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "action",
            "type": {
              "defined": {
                "name": "orderAction"
              }
            }
          },
          {
            "name": "actionExplanation",
            "type": {
              "defined": {
                "name": "orderActionExplanation"
              }
            }
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "marketType",
            "type": {
              "defined": {
                "name": "marketType"
              }
            }
          },
          {
            "name": "filler",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "fillerReward",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "fillRecordId",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "baseAssetAmountFilled",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "quoteAssetAmountFilled",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "takerFee",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "makerFee",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "referrerReward",
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "quoteAssetAmountSurplus",
            "type": {
              "option": "i64"
            }
          },
          {
            "name": "spotFulfillmentMethodFee",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "taker",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "takerOrderId",
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "takerOrderDirection",
            "type": {
              "option": {
                "defined": {
                  "name": "positionDirection"
                }
              }
            }
          },
          {
            "name": "takerOrderBaseAssetAmount",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "takerOrderCumulativeBaseAssetAmountFilled",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "takerOrderCumulativeQuoteAssetAmountFilled",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "maker",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "makerOrderId",
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "makerOrderDirection",
            "type": {
              "option": {
                "defined": {
                  "name": "positionDirection"
                }
              }
            }
          },
          {
            "name": "makerOrderBaseAssetAmount",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "makerOrderCumulativeBaseAssetAmountFilled",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "makerOrderCumulativeQuoteAssetAmountFilled",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "oraclePrice",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lpRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "action",
            "type": {
              "defined": {
                "name": "lpAction"
              }
            }
          },
          {
            "name": "nShares",
            "type": "u64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "deltaBaseAssetAmount",
            "type": "i64"
          },
          {
            "name": "deltaQuoteAssetAmount",
            "type": "i64"
          },
          {
            "name": "pnl",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "liquidationRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "liquidationType",
            "type": {
              "defined": {
                "name": "liquidationType"
              }
            }
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "liquidator",
            "type": "pubkey"
          },
          {
            "name": "marginRequirement",
            "type": "u128"
          },
          {
            "name": "totalCollateral",
            "type": "i128"
          },
          {
            "name": "marginFreed",
            "type": "u64"
          },
          {
            "name": "liquidationId",
            "type": "u16"
          },
          {
            "name": "bankrupt",
            "type": "bool"
          },
          {
            "name": "canceledOrderIds",
            "type": {
              "vec": "u32"
            }
          },
          {
            "name": "liquidatePerp",
            "type": {
              "defined": {
                "name": "liquidatePerpRecord"
              }
            }
          },
          {
            "name": "liquidateSpot",
            "type": {
              "defined": {
                "name": "liquidateSpotRecord"
              }
            }
          },
          {
            "name": "liquidateBorrowForPerpPnl",
            "type": {
              "defined": {
                "name": "liquidateBorrowForPerpPnlRecord"
              }
            }
          },
          {
            "name": "liquidatePerpPnlForDeposit",
            "type": {
              "defined": {
                "name": "liquidatePerpPnlForDepositRecord"
              }
            }
          },
          {
            "name": "perpBankruptcy",
            "type": {
              "defined": {
                "name": "perpBankruptcyRecord"
              }
            }
          },
          {
            "name": "spotBankruptcy",
            "type": {
              "defined": {
                "name": "spotBankruptcyRecord"
              }
            }
          }
        ]
      }
    },
    {
      "name": "settlePnlRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "pnl",
            "type": "i128"
          },
          {
            "name": "baseAssetAmount",
            "type": "i64"
          },
          {
            "name": "quoteAssetAmountAfter",
            "type": "i64"
          },
          {
            "name": "quoteEntryAmount",
            "type": "i64"
          },
          {
            "name": "settlePrice",
            "type": "i64"
          },
          {
            "name": "explanation",
            "type": {
              "defined": {
                "name": "settlePnlExplanation"
              }
            }
          }
        ]
      }
    },
    {
      "name": "insuranceFundRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "spotMarketIndex",
            "type": "u16"
          },
          {
            "name": "perpMarketIndex",
            "type": "u16"
          },
          {
            "name": "userIfFactor",
            "type": "u32"
          },
          {
            "name": "totalIfFactor",
            "type": "u32"
          },
          {
            "name": "vaultAmountBefore",
            "type": "u64"
          },
          {
            "name": "insuranceVaultAmountBefore",
            "type": "u64"
          },
          {
            "name": "totalIfSharesBefore",
            "type": "u128"
          },
          {
            "name": "totalIfSharesAfter",
            "type": "u128"
          },
          {
            "name": "amount",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "insuranceFundStakeRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "userAuthority",
            "type": "pubkey"
          },
          {
            "name": "action",
            "type": {
              "defined": {
                "name": "stakeAction"
              }
            }
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "insuranceVaultAmountBefore",
            "type": "u64"
          },
          {
            "name": "ifSharesBefore",
            "type": "u128"
          },
          {
            "name": "userIfSharesBefore",
            "type": "u128"
          },
          {
            "name": "totalIfSharesBefore",
            "type": "u128"
          },
          {
            "name": "ifSharesAfter",
            "type": "u128"
          },
          {
            "name": "userIfSharesAfter",
            "type": "u128"
          },
          {
            "name": "totalIfSharesAfter",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "swapRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "amountOut",
            "type": "u64"
          },
          {
            "name": "amountIn",
            "type": "u64"
          },
          {
            "name": "outMarketIndex",
            "type": "u16"
          },
          {
            "name": "inMarketIndex",
            "type": "u16"
          },
          {
            "name": "outOraclePrice",
            "type": "i64"
          },
          {
            "name": "inOraclePrice",
            "type": "i64"
          },
          {
            "name": "fee",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "spotMarketVaultDepositRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "depositBalance",
            "type": "u128"
          },
          {
            "name": "cumulativeDepositInterestBefore",
            "type": "u128"
          },
          {
            "name": "cumulativeDepositInterestAfter",
            "type": "u128"
          },
          {
            "name": "depositTokenAmountBefore",
            "type": "u64"
          },
          {
            "name": "amount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "deleteUserRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "userAuthority",
            "type": "pubkey"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "subAccountId",
            "type": "u16"
          },
          {
            "name": "keeper",
            "type": {
              "option": "pubkey"
            }
          }
        ]
      }
    },
    {
      "name": "fuelSweepRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "userStatsFuelInsurance",
            "type": "u32"
          },
          {
            "name": "userStatsFuelDeposits",
            "type": "u32"
          },
          {
            "name": "userStatsFuelBorrows",
            "type": "u32"
          },
          {
            "name": "userStatsFuelPositions",
            "type": "u32"
          },
          {
            "name": "userStatsFuelTaker",
            "type": "u32"
          },
          {
            "name": "userStatsFuelMaker",
            "type": "u32"
          },
          {
            "name": "fuelOverflowFuelInsurance",
            "type": "u128"
          },
          {
            "name": "fuelOverflowFuelDeposits",
            "type": "u128"
          },
          {
            "name": "fuelOverflowFuelBorrows",
            "type": "u128"
          },
          {
            "name": "fuelOverflowFuelPositions",
            "type": "u128"
          },
          {
            "name": "fuelOverflowFuelTaker",
            "type": "u128"
          },
          {
            "name": "fuelOverflowFuelMaker",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "fuelSeasonRecord",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ts",
            "type": "i64"
          },
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "fuelInsurance",
            "type": "u128"
          },
          {
            "name": "fuelDeposits",
            "type": "u128"
          },
          {
            "name": "fuelBorrows",
            "type": "u128"
          },
          {
            "name": "fuelPositions",
            "type": "u128"
          },
          {
            "name": "fuelTaker",
            "type": "u128"
          },
          {
            "name": "fuelMaker",
            "type": "u128"
          },
          {
            "name": "fuelTotal",
            "type": "u128"
          }
        ]
      }
    }
  ]
};

