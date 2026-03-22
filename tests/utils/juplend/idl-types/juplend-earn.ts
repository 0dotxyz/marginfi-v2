/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/lending.json`.
 */
export type Lending = {
  "address": "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9",
  "metadata": {
    "name": "lending",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "Created with Anchor"
  },
  "instructions": [
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
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "depositorTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "assets",
          "type": "u64"
        }
      ],
      "returns": "u64"
    },
    {
      "name": "depositWithMinAmountOut",
      "discriminator": [
        116,
        144,
        16,
        97,
        118,
        109,
        40,
        119
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "depositorTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "assets",
          "type": "u64"
        },
        {
          "name": "minAmountOut",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initLending",
      "discriminator": [
        156,
        224,
        67,
        46,
        89,
        189,
        157,
        209
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingAdmin",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "tokenReservesLiquidity"
          ]
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  95,
                  116,
                  111,
                  107,
                  101,
                  110,
                  95,
                  109,
                  105,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ]
          }
        },
        {
          "name": "metadataAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "const",
                "value": [
                  11,
                  112,
                  101,
                  177,
                  227,
                  209,
                  124,
                  69,
                  56,
                  157,
                  82,
                  127,
                  107,
                  4,
                  195,
                  205,
                  88,
                  184,
                  108,
                  115,
                  26,
                  160,
                  253,
                  181,
                  73,
                  182,
                  209,
                  188,
                  3,
                  248,
                  41,
                  70
                ]
              },
              {
                "kind": "account",
                "path": "fTokenMint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                11,
                112,
                101,
                177,
                227,
                209,
                124,
                69,
                56,
                157,
                82,
                127,
                107,
                4,
                195,
                205,
                88,
                184,
                108,
                115,
                26,
                160,
                253,
                181,
                73,
                182,
                209,
                188,
                3,
                248,
                41,
                70
              ]
            }
          }
        },
        {
          "name": "lending",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  101,
                  110,
                  100,
                  105,
                  110,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              },
              {
                "kind": "account",
                "path": "fTokenMint"
              }
            ]
          }
        },
        {
          "name": "tokenReservesLiquidity"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "sysvarInstruction",
          "address": "Sysvar1nstructions1111111111111111111111111"
        },
        {
          "name": "metadataProgram",
          "address": "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
        },
        {
          "name": "rent",
          "address": "SysvarRent111111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "symbol",
          "type": "string"
        },
        {
          "name": "liquidityProgram",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "initLendingAdmin",
      "discriminator": [
        203,
        185,
        241,
        165,
        56,
        254,
        33,
        9
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "lendingAdmin",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  101,
                  110,
                  100,
                  105,
                  110,
                  103,
                  95,
                  97,
                  100,
                  109,
                  105,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "liquidityProgram",
          "type": "pubkey"
        },
        {
          "name": "rebalancer",
          "type": "pubkey"
        },
        {
          "name": "authority",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "mint",
      "discriminator": [
        51,
        57,
        225,
        47,
        182,
        146,
        137,
        166
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "depositorTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "shares",
          "type": "u64"
        }
      ],
      "returns": "u64"
    },
    {
      "name": "mintWithMaxAssets",
      "discriminator": [
        6,
        94,
        69,
        122,
        30,
        179,
        146,
        171
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "depositorTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "shares",
          "type": "u64"
        },
        {
          "name": "maxAssets",
          "type": "u64"
        }
      ],
      "returns": "u64"
    },
    {
      "name": "rebalance",
      "discriminator": [
        108,
        158,
        77,
        9,
        210,
        52,
        88,
        62
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "depositorTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "signer"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel",
          "writable": true
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "redeem",
      "discriminator": [
        184,
        12,
        86,
        149,
        70,
        196,
        97,
        225
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "ownerTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "claimAccount",
          "writable": true,
          "optional": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "shares",
          "type": "u64"
        }
      ],
      "returns": "u64"
    },
    {
      "name": "redeemWithMinAmountOut",
      "discriminator": [
        235,
        189,
        237,
        56,
        166,
        180,
        184,
        149
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "ownerTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "claimAccount",
          "writable": true,
          "optional": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "shares",
          "type": "u64"
        },
        {
          "name": "minAmountOut",
          "type": "u64"
        }
      ]
    },
    {
      "name": "setRewardsRateModel",
      "discriminator": [
        174,
        231,
        116,
        203,
        8,
        58,
        143,
        203
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "fTokenMint",
          "relations": [
            "lending"
          ]
        },
        {
          "name": "newRewardsRateModel"
        },
        {
          "name": "supplyTokenReservesLiquidity"
        }
      ],
      "args": [
        {
          "name": "mint",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "updateAuthority",
      "discriminator": [
        32,
        46,
        64,
        28,
        149,
        75,
        243,
        88
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "lendingAdmin",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "newAuthority",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "updateAuths",
      "discriminator": [
        93,
        96,
        178,
        156,
        57,
        117,
        253,
        209
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "lendingAdmin",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "authStatus",
          "type": {
            "vec": {
              "defined": {
                "name": "addressBool"
              }
            }
          }
        }
      ]
    },
    {
      "name": "updateRate",
      "discriminator": [
        24,
        225,
        53,
        189,
        72,
        212,
        225,
        178
      ],
      "accounts": [
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "supplyTokenReservesLiquidity"
          ]
        },
        {
          "name": "fTokenMint",
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity"
        },
        {
          "name": "rewardsRateModel"
        }
      ],
      "args": []
    },
    {
      "name": "updateRebalancer",
      "discriminator": [
        206,
        187,
        54,
        228,
        145,
        8,
        203,
        111
      ],
      "accounts": [
        {
          "name": "signer",
          "signer": true
        },
        {
          "name": "lendingAdmin",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "newRebalancer",
          "type": "pubkey"
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
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "ownerTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "claimAccount",
          "writable": true,
          "optional": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        }
      ],
      "returns": "u64"
    },
    {
      "name": "withdrawWithMaxSharesBurn",
      "discriminator": [
        47,
        197,
        183,
        171,
        239,
        18,
        245,
        171
      ],
      "accounts": [
        {
          "name": "signer",
          "writable": true,
          "signer": true
        },
        {
          "name": "ownerTokenAccount",
          "writable": true
        },
        {
          "name": "recipientTokenAccount",
          "writable": true
        },
        {
          "name": "lendingAdmin"
        },
        {
          "name": "lending",
          "writable": true
        },
        {
          "name": "mint",
          "relations": [
            "lending",
            "rewardsRateModel"
          ]
        },
        {
          "name": "fTokenMint",
          "writable": true,
          "relations": [
            "lending"
          ]
        },
        {
          "name": "supplyTokenReservesLiquidity",
          "writable": true
        },
        {
          "name": "lendingSupplyPositionOnLiquidity",
          "writable": true
        },
        {
          "name": "rateModel"
        },
        {
          "name": "vault",
          "writable": true
        },
        {
          "name": "claimAccount",
          "writable": true,
          "optional": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true,
          "relations": [
            "lendingAdmin"
          ]
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "maxSharesBurn",
          "type": "u64"
        }
      ],
      "returns": "u64"
    }
  ],
  "accounts": [
    {
      "name": "lending",
      "discriminator": [
        135,
        199,
        82,
        16,
        249,
        131,
        182,
        241
      ]
    },
    {
      "name": "lendingAdmin",
      "discriminator": [
        42,
        8,
        33,
        220,
        163,
        40,
        210,
        5
      ]
    },
    {
      "name": "lendingRewardsRateModel",
      "discriminator": [
        166,
        72,
        71,
        131,
        172,
        74,
        166,
        181
      ]
    }
  ],
  "events": [
    {
      "name": "logDeposit",
      "discriminator": [
        176,
        243,
        1,
        56,
        142,
        206,
        1,
        106
      ]
    },
    {
      "name": "logRebalance",
      "discriminator": [
        90,
        67,
        219,
        41,
        181,
        118,
        132,
        9
      ]
    },
    {
      "name": "logUpdateAuthority",
      "discriminator": [
        150,
        152,
        157,
        143,
        6,
        135,
        193,
        101
      ]
    },
    {
      "name": "logUpdateAuths",
      "discriminator": [
        88,
        80,
        109,
        48,
        111,
        203,
        76,
        251
      ]
    },
    {
      "name": "logUpdateRates",
      "discriminator": [
        222,
        11,
        113,
        60,
        147,
        15,
        68,
        217
      ]
    },
    {
      "name": "logUpdateRebalancer",
      "discriminator": [
        66,
        79,
        144,
        204,
        26,
        217,
        153,
        225
      ]
    },
    {
      "name": "logUpdateRewards",
      "discriminator": [
        37,
        13,
        111,
        186,
        47,
        245,
        162,
        121
      ]
    },
    {
      "name": "logWithdraw",
      "discriminator": [
        49,
        9,
        176,
        179,
        222,
        190,
        6,
        117
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "fTokenDepositInsignificant",
      "msg": "fTokenDepositInsignificant"
    },
    {
      "code": 6001,
      "name": "fTokenMinAmountOut",
      "msg": "fTokenMinAmountOut"
    },
    {
      "code": 6002,
      "name": "fTokenMaxAmount",
      "msg": "fTokenMaxAmount"
    },
    {
      "code": 6003,
      "name": "fTokenInvalidParams",
      "msg": "fTokenInvalidParams"
    },
    {
      "code": 6004,
      "name": "fTokenRewardsRateModelAlreadySet",
      "msg": "fTokenRewardsRateModelAlreadySet"
    },
    {
      "code": 6005,
      "name": "fTokenMaxAuthCountReached",
      "msg": "fTokenMaxAuthCount"
    },
    {
      "code": 6006,
      "name": "fTokenLiquidityExchangePriceUnexpected",
      "msg": "fTokenLiquidityExchangePriceUnexpected"
    },
    {
      "code": 6007,
      "name": "fTokenCpiToLiquidityFailed",
      "msg": "fTokenCpiToLiquidityFailed"
    },
    {
      "code": 6008,
      "name": "fTokenOnlyAuth",
      "msg": "fTokenOnlyAuth"
    },
    {
      "code": 6009,
      "name": "fTokenOnlyAuthority",
      "msg": "fTokenOnlyAuthority"
    },
    {
      "code": 6010,
      "name": "fTokenOnlyRebalancer",
      "msg": "fTokenOnlyRebalancer"
    },
    {
      "code": 6011,
      "name": "fTokenUserSupplyPositionRequired",
      "msg": "fTokenUserSupplyPositionRequired"
    },
    {
      "code": 6012,
      "name": "fTokenLiquidityProgramMismatch",
      "msg": "fTokenLiquidityProgramMismatch"
    }
  ],
  "types": [
    {
      "name": "addressBool",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "addr",
            "type": "pubkey"
          },
          {
            "name": "value",
            "type": "bool"
          }
        ]
      }
    },
    {
      "name": "lending",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "fTokenMint",
            "type": "pubkey"
          },
          {
            "name": "lendingId",
            "type": "u16"
          },
          {
            "name": "decimals",
            "docs": [
              "@dev number of decimals for the fToken, same as ASSET"
            ],
            "type": "u8"
          },
          {
            "name": "rewardsRateModel",
            "docs": [
              "@dev To read PDA of rewards rate model to get_rate instruction"
            ],
            "type": "pubkey"
          },
          {
            "name": "liquidityExchangePrice",
            "docs": [
              "@dev exchange price for the underlying asset in the liquidity protocol (without rewards)"
            ],
            "type": "u64"
          },
          {
            "name": "tokenExchangePrice",
            "docs": [
              "@dev exchange price between fToken and the underlying asset (with rewards)"
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdateTimestamp",
            "docs": [
              "@dev timestamp when exchange prices were updated the last time"
            ],
            "type": "u64"
          },
          {
            "name": "tokenReservesLiquidity",
            "type": "pubkey"
          },
          {
            "name": "supplyPositionOnLiquidity",
            "type": "pubkey"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "lendingAdmin",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "liquidityProgram",
            "type": "pubkey"
          },
          {
            "name": "rebalancer",
            "type": "pubkey"
          },
          {
            "name": "nextLendingId",
            "type": "u16"
          },
          {
            "name": "auths",
            "type": {
              "vec": "pubkey"
            }
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "lendingRewardsRateModel",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "mint",
            "docs": [
              "@dev mint address"
            ],
            "type": "pubkey"
          },
          {
            "name": "startTvl",
            "docs": [
              "@dev tvl below which rewards rate is 0. If current TVL is below this value, triggering `update_rate()` on the fToken",
              "might bring the total TVL above this cut-off."
            ],
            "type": "u64"
          },
          {
            "name": "duration",
            "docs": [
              "@dev for how long current rewards should run"
            ],
            "type": "u64"
          },
          {
            "name": "startTime",
            "docs": [
              "@dev when current rewards got started"
            ],
            "type": "u64"
          },
          {
            "name": "yearlyReward",
            "docs": [
              "@dev current annualized reward based on input params (duration, rewardAmount)"
            ],
            "type": "u64"
          },
          {
            "name": "nextDuration",
            "docs": [
              "@dev Duration for the next rewards phase"
            ],
            "type": "u64"
          },
          {
            "name": "nextRewardAmount",
            "docs": [
              "@dev Amount of rewards for the next phase"
            ],
            "type": "u64"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "logDeposit",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "sender",
            "type": "pubkey"
          },
          {
            "name": "receiver",
            "type": "pubkey"
          },
          {
            "name": "assets",
            "type": "u64"
          },
          {
            "name": "sharesMinted",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "logRebalance",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assets",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "logUpdateAuthority",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "newAuthority",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "logUpdateAuths",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authStatus",
            "type": {
              "vec": {
                "defined": {
                  "name": "addressBool"
                }
              }
            }
          }
        ]
      }
    },
    {
      "name": "logUpdateRates",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "tokenExchangePrice",
            "type": "u64"
          },
          {
            "name": "liquidityExchangePrice",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "logUpdateRebalancer",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "newRebalancer",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "logUpdateRewards",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "rewardsRateModel",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "logWithdraw",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "sender",
            "type": "pubkey"
          },
          {
            "name": "receiver",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "assets",
            "type": "u64"
          },
          {
            "name": "sharesBurned",
            "type": "u64"
          }
        ]
      }
    }
  ]
};
