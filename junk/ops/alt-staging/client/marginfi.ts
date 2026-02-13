/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/marginfi.json`.
 */
export type Marginfi = {
  "address": "5UDghkpgW1HfYSrmEj2iAApHShqU44H6PKTAar9LL9bY",
  "metadata": {
    "name": "marginfi",
    "version": "0.1.6",
    "spec": "0.1.0",
    "description": "Borrow Lending Prime Broker"
  },
  "instructions": [
    {
      "name": "configGroupFee",
      "docs": [
        "(global fee admin only) Enable or disable program fees for any group. Does not require the",
        "group admin to sign: the global fee state admin can turn program fees on or off for any",
        "group"
      ],
      "discriminator": [
        231,
        205,
        66,
        242,
        220,
        87,
        145,
        38
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true
        },
        {
          "name": "globalFeeAdmin",
          "docs": [
            "`global_fee_admin` of the FeeState"
          ],
          "signer": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "enableProgramFee",
          "type": "bool"
        }
      ]
    },
    {
      "name": "configureDeleverageWithdrawalLimit",
      "docs": [
        "(group admin only) Set the daily withdrawal limit for deleverages per group."
      ],
      "discriminator": [
        28,
        132,
        205,
        158,
        67,
        77,
        177,
        63
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        }
      ],
      "args": [
        {
          "name": "limit",
          "type": "u32"
        }
      ]
    },
    {
      "name": "driftDeposit",
      "docs": [
        "(user) Deposit into a Drift spot market through a marginfi account",
        "* amount - in the underlying token (e.g., USDC), in native decimals"
      ],
      "discriminator": [
        252,
        63,
        250,
        201,
        98,
        55,
        130,
        12
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "driftOracle",
          "docs": [
            "The oracle account for the asset (not needed if using oracle type QuoteAsset)"
          ],
          "optional": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority, which owns the Drift user account"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Used as an intermediary to deposit tokens into Drift"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "Owned by authority, the source account for the token deposit"
          ],
          "writable": true
        },
        {
          "name": "driftState",
          "docs": [
            "The Drift state account"
          ]
        },
        {
          "name": "driftUser",
          "docs": [
            "The Drift user account owned by liquidity_vault_authority"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftUserStats",
          "docs": [
            "The Drift user stats account owned by liquidity_vault_authority"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftSpotMarket",
          "docs": [
            "The Drift spot market for this asset"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftSpotMarketVault",
          "docs": [
            "The Drift spot market vault that will receive tokens"
          ],
          "writable": true
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint"
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftProgram",
          "address": "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"
        },
        {
          "name": "tokenProgram"
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
      ]
    },
    {
      "name": "driftHarvestReward",
      "docs": [
        "(fee admin only) Harvest rewards from admin deposits in Drift spot markets",
        "The harvest spot market must be different from the bank's main drift spot market"
      ],
      "discriminator": [
        167,
        161,
        240,
        194,
        138,
        54,
        87,
        189
      ],
      "accounts": [
        {
          "name": "bank"
        },
        {
          "name": "feeState",
          "docs": [
            "Global fee state that contains the global_fee_wallet"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "intermediaryTokenAccount",
          "docs": [
            "To create this manually just send some of the reward token",
            "to the liquidity vault authority address before claiming"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "rewardMint"
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
          "name": "destinationTokenAccount",
          "docs": [
            "Destination token account must be owned by the global fee wallet"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "feeState"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "rewardMint"
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
          "name": "driftState",
          "docs": [
            "Drift accounts"
          ]
        },
        {
          "name": "driftUser",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftUserStats",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "harvestDriftSpotMarket",
          "docs": [
            "The harvest spot market - MUST be different from bank's drift_spot_market",
            "This is the market that contains admin deposits to harvest"
          ],
          "writable": true
        },
        {
          "name": "harvestDriftSpotMarketVault",
          "docs": [
            "The harvest spot market vault - derived from harvest_drift_spot_market"
          ],
          "writable": true
        },
        {
          "name": "driftSigner",
          "docs": [
            "The Drift signer PDA"
          ]
        },
        {
          "name": "rewardMint"
        },
        {
          "name": "driftProgram",
          "address": "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "driftInitUser",
      "docs": [
        "(permissionless) Initialize a Drift user and user stats for a marginfi bank",
        "Creates user with sub_account_id = 0 and empty name",
        "Requires a minimum deposit to ensure the account remains active",
        "* amount - minimum deposit amount (at least 10 units) in native decimals"
      ],
      "discriminator": [
        29,
        18,
        236,
        190,
        29,
        254,
        114,
        169
      ],
      "accounts": [
        {
          "name": "feePayer",
          "docs": [
            "Pays to init the drift user and user stats accounts and provides initial deposit"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "The fee payer must provide a nominal amount of bank tokens so the account is not empty.",
            "This amount is irrecoverable and will prevent the account from being closed."
          ],
          "writable": true
        },
        {
          "name": "bank"
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The liquidity vault authority (PDA that will own the Drift user)"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Used as an intermediary to deposit a nominal amount of token into Drift."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint (e.g., USDC)"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftUserStats",
          "docs": [
            "The user stats account to be created"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  95,
                  115,
                  116,
                  97,
                  116,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                9,
                84,
                219,
                190,
                158,
                201,
                96,
                201,
                138,
                122,
                41,
                63,
                226,
                19,
                54,
                150,
                111,
                225,
                128,
                209,
                81,
                174,
                75,
                129,
                121,
                86,
                31,
                137,
                133,
                74,
                83,
                246
              ]
            }
          },
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftUser",
          "docs": [
            "The user account to be created (sub_account_id = 0)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              },
              {
                "kind": "const",
                "value": [
                  0,
                  0
                ]
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                9,
                84,
                219,
                190,
                158,
                201,
                96,
                201,
                138,
                122,
                41,
                63,
                226,
                19,
                54,
                150,
                111,
                225,
                128,
                209,
                81,
                174,
                75,
                129,
                121,
                86,
                31,
                137,
                133,
                74,
                83,
                246
              ]
            }
          },
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftState",
          "writable": true
        },
        {
          "name": "driftSpotMarket",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftSpotMarketVault",
          "docs": [
            "The Drift spot market vault where tokens will be deposited"
          ],
          "writable": true
        },
        {
          "name": "driftOracle",
          "docs": [
            "Oracle for the asset (can be null for USDC/market 0)"
          ],
          "optional": true
        },
        {
          "name": "driftProgram",
          "address": "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "rent",
          "address": "SysvarRent111111111111111111111111111111111"
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
      ]
    },
    {
      "name": "driftWithdraw",
      "docs": [
        "(user) Withdraw from a Drift spot market through a marginfi account",
        "* amount - in the underlying token (e.g., USDC), in native decimals",
        "* withdraw_all - if true, withdraws entire position"
      ],
      "discriminator": [
        86,
        59,
        186,
        123,
        183,
        181,
        234,
        137
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true,
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "driftOracle",
          "docs": [
            "The oracle account for the asset (not needed if using oracle type QuoteAsset)"
          ],
          "optional": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority, which owns the Drift user account"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Receives tokens from Drift withdrawal"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "destinationTokenAccount",
          "docs": [
            "Token account that will receive the withdrawn tokens"
          ],
          "writable": true
        },
        {
          "name": "driftState",
          "docs": [
            "The Drift state account"
          ]
        },
        {
          "name": "driftUser",
          "docs": [
            "The Drift user account owned by liquidity_vault_authority"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftUserStats",
          "docs": [
            "The Drift user stats account owned by liquidity_vault_authority"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftSpotMarket",
          "docs": [
            "The Drift spot market for this asset"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftSpotMarketVault",
          "docs": [
            "The Drift spot market vault that holds tokens"
          ],
          "writable": true
        },
        {
          "name": "driftRewardOracle",
          "docs": [
            "Optional: Oracle for first reward asset (only needed if rewards exist)"
          ],
          "optional": true
        },
        {
          "name": "driftRewardSpotMarket",
          "docs": [
            "Optional: Spot market for first reward asset (only needed if rewards exist)"
          ],
          "optional": true
        },
        {
          "name": "driftRewardMint",
          "docs": [
            "Optional: Mint for first reward asset (only needed if rewards exist)"
          ],
          "optional": true
        },
        {
          "name": "driftRewardOracle2",
          "docs": [
            "Optional: Oracle for second reward asset (backup in case multiple rewards)"
          ],
          "optional": true
        },
        {
          "name": "driftRewardSpotMarket2",
          "docs": [
            "Optional: Spot market for second reward asset (backup in case multiple rewards)"
          ],
          "optional": true
        },
        {
          "name": "driftRewardMint2",
          "docs": [
            "Optional: Mint for second reward asset (backup in case multiple rewards)"
          ],
          "optional": true
        },
        {
          "name": "driftSigner",
          "docs": [
            "The Drift signer PDA"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint"
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "driftProgram",
          "address": "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"
        },
        {
          "name": "tokenProgram"
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
          "name": "withdrawAll",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "editGlobalFeeState",
      "docs": [
        "(global fee admin only) Adjust fees, admin, or the destination wallet"
      ],
      "discriminator": [
        52,
        62,
        35,
        129,
        93,
        69,
        165,
        202
      ],
      "accounts": [
        {
          "name": "globalFeeAdmin",
          "docs": [
            "Admin of the global FeeState"
          ],
          "signer": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "feeState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "admin",
          "type": "pubkey"
        },
        {
          "name": "feeWallet",
          "type": "pubkey"
        },
        {
          "name": "bankInitFlatSolFee",
          "type": "u32"
        },
        {
          "name": "liquidationFlatSolFee",
          "type": "u32"
        },
        {
          "name": "programFeeFixed",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        },
        {
          "name": "programFeeRate",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        },
        {
          "name": "liquidationMaxFee",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        }
      ]
    },
    {
      "name": "editStakedSettings",
      "discriminator": [
        11,
        108,
        215,
        87,
        240,
        9,
        66,
        241
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "relations": [
            "stakedSettings"
          ]
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        },
        {
          "name": "stakedSettings",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "settings",
          "type": {
            "defined": {
              "name": "stakedSettingsEditConfig"
            }
          }
        }
      ]
    },
    {
      "name": "endDeleverage",
      "discriminator": [
        114,
        14,
        250,
        143,
        252,
        104,
        214,
        209
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "liquidationRecord",
          "writable": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "group",
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "riskAdmin",
          "signer": true,
          "relations": [
            "group"
          ]
        }
      ],
      "args": []
    },
    {
      "name": "endLiquidation",
      "discriminator": [
        110,
        11,
        244,
        54,
        229,
        181,
        22,
        184
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "docs": [
            "Account under liquidation"
          ],
          "writable": true
        },
        {
          "name": "liquidationRecord",
          "docs": [
            "The associated liquidation record PDA for the given `marginfi_account`"
          ],
          "writable": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "liquidationReceiver",
          "writable": true,
          "signer": true,
          "relations": [
            "liquidationRecord"
          ]
        },
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalFeeWallet",
          "writable": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initBankMetadata",
      "docs": [
        "(permissionless) pay the rent to open a bank's metadata."
      ],
      "discriminator": [
        94,
        239,
        50,
        136,
        137,
        204,
        254,
        213
      ],
      "accounts": [
        {
          "name": "bank"
        },
        {
          "name": "feePayer",
          "docs": [
            "Pays the init fee"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "metadata",
          "docs": [
            "Note: unique per-bank."
          ],
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
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initGlobalFeeState",
      "docs": [
        "(Runs once per program) Configures the fee state account, where the global admin sets fees",
        "that are assessed to the protocol"
      ],
      "discriminator": [
        82,
        48,
        247,
        59,
        220,
        109,
        231,
        44
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "Pays the init fee"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "feeState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
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
          "name": "admin",
          "type": "pubkey"
        },
        {
          "name": "feeWallet",
          "type": "pubkey"
        },
        {
          "name": "bankInitFlatSolFee",
          "type": "u32"
        },
        {
          "name": "liquidationFlatSolFee",
          "type": "u32"
        },
        {
          "name": "programFeeFixed",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        },
        {
          "name": "programFeeRate",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        },
        {
          "name": "liquidationMaxFee",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        }
      ]
    },
    {
      "name": "initStakedSettings",
      "docs": [
        "(group admin only) Init the Staked Settings account, which is used to create staked",
        "collateral banks, and must run before any staked collateral bank can be created with",
        "`add_pool_permissionless`. Running this ix effectively opts the group into the staked",
        "collateral feature."
      ],
      "discriminator": [
        52,
        35,
        149,
        44,
        69,
        86,
        69,
        80
      ],
      "accounts": [
        {
          "name": "marginfiGroup"
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        },
        {
          "name": "feePayer",
          "docs": [
            "Pays the init fee"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "stakedSettings",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101,
                  100,
                  95,
                  115,
                  101,
                  116,
                  116,
                  105,
                  110,
                  103,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "marginfiGroup"
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
          "name": "settings",
          "type": {
            "defined": {
              "name": "stakedSettingsConfig"
            }
          }
        }
      ]
    },
    {
      "name": "juplendDeposit",
      "docs": [
        "(user) Deposit into a JupLend lending pool through a marginfi account.",
        "* amount - in the underlying token (e.g., USDC), in native decimals"
      ],
      "discriminator": [
        114,
        11,
        218,
        81,
        183,
        165,
        143,
        255
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "Owned by authority, the source account for the token deposit."
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).",
            "NOTE: JupLend marks the signer as writable in their deposit instruction."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Bank liquidity vault (holds underlying mint and is used as depositor_token_account)."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Underlying mint."
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "juplendLending",
          "docs": [
            "JupLend lending state account."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "fTokenMint",
          "docs": [
            "JupLend fToken mint."
          ],
          "writable": true,
          "relations": [
            "juplendLending"
          ]
        },
        {
          "name": "juplendFTokenVault",
          "docs": [
            "Bank's fToken vault (validated via has_one on bank)."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingAdmin"
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
          "docs": [
            "NOTE: JupLend marks this as writable in their Lending program (unusual for a program account)"
          ],
          "writable": true
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "juplendProgram",
          "address": "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9"
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
      ]
    },
    {
      "name": "juplendInitPosition",
      "docs": [
        "(permissionless) Initialize the bank-level JupLend position.",
        "",
        "This creates the bank's fToken ATA (owned by the bank liquidity vault authority) and",
        "performs a nominal seed deposit into JupLend, then flips the bank from `Paused` to",
        "`Operational`."
      ],
      "discriminator": [
        176,
        255,
        151,
        106,
        5,
        207,
        74,
        215
      ],
      "accounts": [
        {
          "name": "feePayer",
          "docs": [
            "Pays for the ATA creation (if needed) and provides a nominal deposit amount."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "Token account owned by the fee payer holding the underlying mint."
          ],
          "writable": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs)."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Bank liquidity vault (holds underlying mint and is used as depositor_token_account)."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Underlying mint (must match bank mint and JupLend lending state mint)."
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "juplendLending",
          "docs": [
            "JupLend lending state account."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "fTokenMint",
          "docs": [
            "JupLend fToken mint."
          ],
          "writable": true,
          "relations": [
            "juplendLending"
          ]
        },
        {
          "name": "juplendFTokenVault",
          "docs": [
            "Bank's fToken vault (validated via has_one on bank)."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingAdmin"
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
          "docs": [
            "NOTE: JupLend marks this as writable in their Lending program (unusual for a program account)"
          ],
          "writable": true
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "juplendProgram",
          "address": "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9"
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
      ]
    },
    {
      "name": "juplendWithdraw",
      "docs": [
        "(user) Withdraw from a JupLend lending pool through a marginfi account.",
        "* amount - in the underlying token (e.g., USDC), in native decimals"
      ],
      "discriminator": [
        245,
        164,
        253,
        202,
        53,
        77,
        251,
        221
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true,
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "destinationTokenAccount",
          "docs": [
            "Token account that will receive the underlying withdrawal.",
            "WARN: Completely unchecked!"
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).",
            "NOTE: JupLend marks the signer as writable in their withdraw instruction."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Bank liquidity vault (receives underlying from JupLend withdraw)."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Underlying mint."
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "juplendLending",
          "docs": [
            "JupLend lending state account."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "fTokenMint",
          "docs": [
            "JupLend fToken mint."
          ],
          "writable": true,
          "relations": [
            "juplendLending"
          ]
        },
        {
          "name": "juplendFTokenVault",
          "docs": [
            "Bank's fToken vault (validated via has_one on bank)."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingAdmin"
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
          "docs": [
            "JupLend claim account for liquidity_vault_authority.",
            "TEMPORARY: Mainnet currently requires this account (passing None causes ConstraintMut errors),",
            "but an upcoming upgrade is expected to make it truly optional. The account is never actually",
            "validated or used - you can pass any mutable account. We create the canonical PDA for consistency.",
            "Seeds: [\"user_claim\", liquidity_vault_authority, mint] on Liquidity program."
          ],
          "writable": true
        },
        {
          "name": "liquidity",
          "writable": true
        },
        {
          "name": "liquidityProgram",
          "writable": true
        },
        {
          "name": "rewardsRateModel"
        },
        {
          "name": "juplendProgram",
          "address": "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9"
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
          "name": "withdrawAll",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "kaminoDeposit",
      "docs": [
        "(user) Deposit into a Kamino pool through a marginfi account",
        "* amount - in the liquidity token (e.g. if there is a Kamino USDC bank, pass the amount of",
        "USDC desired), in native decimals."
      ],
      "discriminator": [
        237,
        8,
        188,
        187,
        115,
        99,
        49,
        85
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "Owned by authority, the source account for the token deposit."
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority, which owns the Kamino obligation. Note: Kamino needs",
            "this to be mut because `deposit` might return the rent here"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Used as an intermediary to deposit token into Kamino"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "kaminoObligation",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "kaminoReserve",
          "docs": [
            "The Kamino reserve that holds liquidity"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint (e.g., USDC). Kamino calls this the `reserve_liquidity_mint`"
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The reserve's mint for tokenized representations of Kamino deposits."
          ],
          "writable": true
        },
        {
          "name": "reserveDestinationDepositCollateral",
          "docs": [
            "The reserve's destination for tokenized representations of deposits. Note: the",
            "`reserve_collateral_mint` will mint tokens directly to this account."
          ],
          "writable": true
        },
        {
          "name": "obligationFarmUserState",
          "docs": [
            "Required if the Kamino reserve has an active farm."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "reserveFarmState",
          "docs": [
            "Required if the Kamino reserve has an active farm."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "kaminoProgram",
          "address": "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
        },
        {
          "name": "farmsProgram",
          "docs": [
            "Farms program for Kamino staking functionality"
          ],
          "address": "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
        },
        {
          "name": "collateralTokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "liquidityTokenProgram"
        },
        {
          "name": "instructionSysvarAccount",
          "address": "Sysvar1nstructions1111111111111111111111111"
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
      "name": "kaminoHarvestReward",
      "docs": [
        "(fee admin only) Harvest the specified reward index from the Kamino Farm attached to this bank.",
        "",
        "* `reward_index` — index of the reward token in the Kamino Farm's reward list"
      ],
      "discriminator": [
        163,
        202,
        248,
        141,
        106,
        20,
        116,
        5
      ],
      "accounts": [
        {
          "name": "bank"
        },
        {
          "name": "feeState",
          "docs": [
            "Global fee state that contains the global_fee_admin"
          ]
        },
        {
          "name": "destinationTokenAccount",
          "docs": [
            "Destination token account must be owned by the global fee admin"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "feeState"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "rewardMint"
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
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority, which owns the Kamino obligation."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "userState",
          "writable": true
        },
        {
          "name": "farmState",
          "writable": true
        },
        {
          "name": "globalConfig"
        },
        {
          "name": "rewardMint"
        },
        {
          "name": "userRewardAta",
          "docs": [
            "An initialized ATA of type reward mint owned by liquidity vault"
          ],
          "writable": true
        },
        {
          "name": "rewardsVault",
          "writable": true
        },
        {
          "name": "rewardsTreasuryVault",
          "writable": true
        },
        {
          "name": "farmVaultsAuthority"
        },
        {
          "name": "scopePrices",
          "optional": true
        },
        {
          "name": "farmsProgram",
          "address": "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "rewardIndex",
          "type": "u64"
        }
      ]
    },
    {
      "name": "kaminoInitObligation",
      "docs": [
        "(permissionless) Initialize a Kamino obligation for a marginfi bank",
        "* amount - In token, in native decimals. Must be >10 (i.e. 10 lamports, not 10 tokens). Lost",
        "forever. Generally, try to make this the equivalent of around $1, in case Kamino ever",
        "rounds small balances down to zero."
      ],
      "discriminator": [
        253,
        177,
        160,
        225,
        70,
        156,
        217,
        109
      ],
      "accounts": [
        {
          "name": "feePayer",
          "docs": [
            "Pays to init the obligation and pays a nominal amount to ensure the obligation has a",
            "non-zero balance."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "bank"
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "The fee payer must provide a nominal amount of bank tokens so the obligation is not empty.",
            "This amount is irrecoverable and and will prevent the obligation from ever being closed,",
            "even if the bank is otherwise empty (Kamino normally closes empty obligations automatically)"
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The liquidity vault authority (PDA that will own the Kamino obligation). Note: Kamino needs",
            "this to be mut because `deposit` might return the rent here"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Used as an intermediary to deposit a nominal amount of token into the obligation."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "kaminoObligation",
          "docs": [
            "The obligation account to be created. Note that the key was already derived when",
            "initializing the bank, and this must match the obligation recorded at that time."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "userMetadata",
          "writable": true
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority"
        },
        {
          "name": "kaminoReserve",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint (e.g., USDC). Kamino calls this the `reserve_liquidity_mint`"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The reserve's mint for tokenized representations of Kamino deposits."
          ],
          "writable": true
        },
        {
          "name": "reserveDestinationDepositCollateral",
          "docs": [
            "The reserve's destination for tokenized representations of deposits. Note: the",
            "`reserve_collateral_mint` will mint tokens directly to this account."
          ],
          "writable": true
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
        },
        {
          "name": "obligationFarmUserState",
          "docs": [
            "Required if the Kamino reserve has an active farm."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "reserveFarmState",
          "docs": [
            "Required if the Kamino reserve has an active farm."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "kaminoProgram",
          "address": "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
        },
        {
          "name": "farmsProgram",
          "docs": [
            "Farms program for Kamino staking functionality"
          ],
          "address": "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
        },
        {
          "name": "collateralTokenProgram",
          "docs": [
            "Note: the collateral token always uses Token classic, never Token22."
          ],
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "liquidityTokenProgram",
          "docs": [
            "Note: Kamino does not have full Token22 support, certain Token22 features are disallowed.",
            "Expect this to update over time. Check with the Kamino source."
          ]
        },
        {
          "name": "instructionSysvarAccount",
          "address": "Sysvar1nstructions1111111111111111111111111"
        },
        {
          "name": "rent",
          "address": "SysvarRent111111111111111111111111111111111"
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
      ]
    },
    {
      "name": "kaminoWithdraw",
      "docs": [
        "(user) Withdraw from a Kamino pool through a marginfi account",
        "* amount - in the collateral token (NOT liquidity token), in native decimals. Must convert",
        "from collateral to liquidity token amounts using the current exchange rate.",
        "* withdraw_all - if true, withdraw the entire mrgn balance (Note: due to rounding down, a",
        "deposit and withdraw back to back may result in several lamports less)"
      ],
      "discriminator": [
        199,
        101,
        41,
        45,
        213,
        98,
        224,
        200
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true,
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "destinationTokenAccount",
          "docs": [
            "Token account that will get tokens back",
            "WARN: Completely unchecked!"
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          },
          "relations": [
            "bank"
          ]
        },
        {
          "name": "kaminoObligation",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingMarket",
          "docs": [
            "The Kamino lending market"
          ]
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "The Kamino lending market authority"
          ]
        },
        {
          "name": "kaminoReserve",
          "docs": [
            "The Kamino reserve that holds liquidity"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "reserveLiquidityMint",
          "docs": [
            "The liquidity token mint (e.g., USDC)",
            "Needs serde to get the mint decimals for transfer checked"
          ],
          "writable": true
        },
        {
          "name": "reserveLiquiditySupply",
          "docs": [
            "The reserve's liquidity supply account"
          ],
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The reserve's collateral mint"
          ],
          "writable": true
        },
        {
          "name": "reserveSourceCollateral",
          "docs": [
            "The reserve's source for collateral tokens"
          ],
          "writable": true
        },
        {
          "name": "obligationFarmUserState",
          "docs": [
            "Optional farms accounts for Kamino staking functionality"
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "reserveFarmState",
          "writable": true,
          "optional": true
        },
        {
          "name": "kaminoProgram",
          "address": "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
        },
        {
          "name": "farmsProgram",
          "docs": [
            "Farms program for Kamino staking functionality"
          ],
          "address": "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
        },
        {
          "name": "collateralTokenProgram",
          "docs": [
            "The token program for the collateral token"
          ],
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "liquidityTokenProgram",
          "docs": [
            "The token program for the liquidity token"
          ]
        },
        {
          "name": "instructionSysvarAccount",
          "docs": [
            "Used by kamino validate CPI calls"
          ],
          "address": "Sysvar1nstructions1111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "withdrawAll",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "lendingAccountBorrow",
      "discriminator": [
        4,
        126,
        116,
        53,
        48,
        5,
        212,
        31
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "destinationTokenAccount",
          "writable": true
        },
        {
          "name": "bankLiquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "relations": [
            "bank"
          ]
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
      "name": "lendingAccountCloseBalance",
      "discriminator": [
        245,
        54,
        41,
        4,
        243,
        202,
        31,
        17
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "lendingAccountDeposit",
      "discriminator": [
        171,
        94,
        235,
        103,
        82,
        64,
        212,
        140
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "signerTokenAccount",
          "writable": true
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "depositUpToLimit",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "lendingAccountEndFlashloan",
      "discriminator": [
        105,
        124,
        201,
        106,
        153,
        2,
        8,
        156
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        }
      ],
      "args": []
    },
    {
      "name": "lendingAccountLiquidate",
      "docs": [
        "Liquidate a lending account balance of an unhealthy marginfi account"
      ],
      "discriminator": [
        214,
        169,
        151,
        213,
        251,
        167,
        86,
        219
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "assetBank",
            "liabBank",
            "liquidatorMarginfiAccount",
            "liquidateeMarginfiAccount"
          ]
        },
        {
          "name": "assetBank",
          "writable": true
        },
        {
          "name": "liabBank",
          "writable": true
        },
        {
          "name": "liquidatorMarginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "liquidatorMarginfiAccount"
          ]
        },
        {
          "name": "liquidateeMarginfiAccount",
          "writable": true
        },
        {
          "name": "bankLiquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "liabBank"
              }
            ]
          }
        },
        {
          "name": "bankLiquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "liabBank"
              }
            ]
          }
        },
        {
          "name": "bankInsuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "liabBank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "assetAmount",
          "type": "u64"
        },
        {
          "name": "liquidateeAccounts",
          "type": "u8"
        },
        {
          "name": "liquidatorAccounts",
          "type": "u8"
        }
      ]
    },
    {
      "name": "lendingAccountPulseHealth",
      "docs": [
        "(Permissionless) Refresh the internal risk engine health cache. Useful for liquidators and",
        "other consumers that want to see the internal risk state of a user account. This cache is",
        "read-only and serves no purpose except being populated by this ix.",
        "* remaining accounts expected in the same order as borrow, etc. I.e., for each balance the",
        "user has, pass bank and oracle: <bank1, oracle1, bank2, oracle2>"
      ],
      "discriminator": [
        186,
        52,
        117,
        97,
        34,
        74,
        39,
        253
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "lendingAccountRepay",
      "discriminator": [
        79,
        209,
        172,
        177,
        222,
        51,
        173,
        151
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "docs": [
            "Must be marginfi_account's authority, unless in liquidation/deleverage receivership",
            "",
            "Note: during receivership, there are no signer checks whatsoever: any key can repay as",
            "long as the invariants checked at the end of receivership are met."
          ],
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "signerTokenAccount",
          "writable": true
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "repayAll",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "lendingAccountSettleEmissions",
      "discriminator": [
        161,
        58,
        136,
        174,
        242,
        223,
        156,
        176
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "lendingAccountStartFlashloan",
      "discriminator": [
        14,
        131,
        33,
        220,
        81,
        186,
        180,
        107
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "ixsSysvar",
          "address": "Sysvar1nstructions1111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "endIndex",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingAccountWithdraw",
      "discriminator": [
        36,
        72,
        74,
        19,
        210,
        210,
        192,
        192
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true,
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "docs": [
            "Must be marginfi_account's authority, unless in liquidation/deleverage receivership",
            "",
            "Note: during receivership, there are no signer checks whatsoever: any key can repay as",
            "long as the invariants checked at the end of receivership are met."
          ],
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "destinationTokenAccount",
          "writable": true
        },
        {
          "name": "bankLiquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "withdrawAll",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "lendingAccountWithdrawEmissions",
      "discriminator": [
        234,
        22,
        84,
        214,
        118,
        176,
        140,
        170
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "emissionsMint",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "emissionsAuth",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  97,
                  117,
                  116,
                  104,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "emissionsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  116,
                  111,
                  107,
                  101,
                  110,
                  95,
                  97,
                  99,
                  99,
                  111,
                  117,
                  110,
                  116,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "destinationAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "lendingAccountWithdrawEmissionsPermissionless",
      "discriminator": [
        4,
        174,
        124,
        203,
        44,
        49,
        145,
        150
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "emissionsMint",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "emissionsAuth",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  97,
                  117,
                  116,
                  104,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "emissionsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  116,
                  111,
                  107,
                  101,
                  110,
                  95,
                  97,
                  99,
                  99,
                  111,
                  117,
                  110,
                  116,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "destinationAccount",
          "docs": [
            "registered on `marginfi_account`"
          ],
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "lendingPoolAccrueBankInterest",
      "discriminator": [
        108,
        201,
        30,
        87,
        47,
        65,
        97,
        188
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "lendingPoolAddBank",
      "discriminator": [
        215,
        68,
        72,
        78,
        208,
        218,
        103,
        182
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        },
        {
          "name": "feePayer",
          "docs": [
            "Pays to init accounts and pays `fee_state.bank_init_flat_sol_fee` lamports to the protocol"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalFeeWallet",
          "writable": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "bankMint"
        },
        {
          "name": "bank",
          "writable": true,
          "signer": true
        },
        {
          "name": "liquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankConfig",
          "type": {
            "defined": {
              "name": "bankConfigCompact"
            }
          }
        }
      ]
    },
    {
      "name": "lendingPoolAddBankDrift",
      "docs": [
        "(group admin only) Add a Drift bank to the group."
      ],
      "discriminator": [
        62,
        63,
        49,
        48,
        76,
        55,
        108,
        155
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "bankMint",
          "docs": [
            "Must match the mint used by `drift_spot_market`"
          ]
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "group"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "driftSpotMarket",
          "docs": [
            "Drift spot market account that must match the bank mint"
          ]
        },
        {
          "name": "driftUser",
          "docs": [
            "Drift user account for the marginfi program (derived from liquidity_vault_authority)"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              },
              {
                "kind": "const",
                "value": [
                  0,
                  0
                ]
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                9,
                84,
                219,
                190,
                158,
                201,
                96,
                201,
                138,
                122,
                41,
                63,
                226,
                19,
                54,
                150,
                111,
                225,
                128,
                209,
                81,
                174,
                75,
                129,
                121,
                86,
                31,
                137,
                133,
                74,
                83,
                246
              ]
            }
          }
        },
        {
          "name": "driftUserStats",
          "docs": [
            "Drift user stats account for the marginfi program (derived from liquidity_vault_authority)"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  95,
                  115,
                  116,
                  97,
                  116,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                9,
                84,
                219,
                190,
                158,
                201,
                96,
                201,
                138,
                122,
                41,
                63,
                226,
                19,
                54,
                150,
                111,
                225,
                128,
                209,
                81,
                174,
                75,
                129,
                121,
                86,
                31,
                137,
                133,
                74,
                83,
                246
              ]
            }
          }
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "For Drift banks, the `liquidity_vault` never holds assets, but is instead used as an",
            "intermediary when depositing/withdrawing, e.g., withdrawn funds move from Drift -> here ->",
            "the user's token account."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankConfig",
          "type": {
            "defined": {
              "name": "driftConfigCompact"
            }
          }
        },
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolAddBankJuplend",
      "docs": [
        "(admin) Add a JupLend bank to the marginfi group.",
        "",
        "Remaining accounts (for oracle validation):",
        "0. underlying oracle feed (pyth push or switchboard pull)",
        "1. JupLend `Lending` state"
      ],
      "discriminator": [
        18,
        208,
        117,
        90,
        53,
        111,
        195,
        41
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "bankMint",
          "docs": [
            "Must match the mint used by the JupLend lending state."
          ]
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "group"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "juplendLending",
          "docs": [
            "JupLend lending state account that must match the bank mint."
          ]
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "For JupLend banks, the `liquidity_vault` is used as an intermediary when depositing/",
            "withdrawing, e.g., withdrawn funds move from JupLend -> here -> the user's token account.",
            "",
            "TEMPORARY: Uses ATA derivation instead of LIQUIDITY_VAULT_SEED PDA. Mainnet Fluid",
            "currently requires ATA for depositor_token_account, but this is expected to be removed",
            "in an upcoming upgrade. Once that happens, revert to PDA seeds like other integrations."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "bankMint"
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
          "name": "insuranceVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "fTokenMint",
          "relations": [
            "juplendLending"
          ]
        },
        {
          "name": "juplendFTokenVault",
          "docs": [
            "The bank's fToken vault holds the fTokens received when depositing into JupLend.",
            "",
            "TEMPORARY: Uses ATA derivation instead of a seed-based PDA. Mainnet Fluid currently",
            "requires ATA for depositor_token_account, but this constraint is expected to be removed",
            "in an upcoming upgrade. Once that happens, revert to a PDA with seeds like other vaults.",
            "",
            "NOTE: JupLend creates fToken mints using the same token program as the underlying mint,",
            "so for Token-2022 underlying mints, fToken mints are also Token-2022."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "liquidityVaultAuthority"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "fTokenMint"
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
          "name": "tokenProgram",
          "docs": [
            "Token program for both underlying mint and fToken mint (SPL Token or Token-2022).",
            "JupLend creates fToken mints using the same token program as the underlying."
          ]
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
          "name": "bankConfig",
          "type": {
            "defined": {
              "name": "juplendConfigCompact"
            }
          }
        },
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolAddBankKamino",
      "docs": [
        "(group admin only) Add a Kamino bank to the group. Pass the oracle and reserve in remaining",
        "accounts 0 and 1 respectively."
      ],
      "discriminator": [
        118,
        53,
        16,
        243,
        255,
        245,
        149,
        241
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "bankMint",
          "docs": [
            "Must match the mint used by `kamino_reserve`, Kamino calls this the `reserve_liquidity_mint`",
            "aka `liquidity.mint_pubkey`"
          ]
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "group"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "kaminoReserve"
        },
        {
          "name": "kaminoObligation",
          "docs": [
            "Note: not yet initialized in this instruction, run `init_obligation` after."
          ]
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "Will be authority of the bank's `kamino_obligation`. Note: When depositing/withdrawing",
            "Kamino assets, the source/destination must also be owned by the obligation authority. This",
            "account owns the `liquidity_vault`, and thus acts as intermediary for deposits/withdraws"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "For Kamino banks, the `liquidity_vault` never holds assets, but is instead used as an",
            "intermediary when depositing/withdrawing, e.g., withdrawn funds move from Kamino -> here ->",
            "the user's token account."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankConfig",
          "type": {
            "defined": {
              "name": "kaminoConfigCompact"
            }
          }
        },
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolAddBankPermissionless",
      "discriminator": [
        127,
        187,
        121,
        34,
        187,
        167,
        238,
        102
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true,
          "relations": [
            "stakedSettings"
          ]
        },
        {
          "name": "stakedSettings"
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "bankMint",
          "docs": [
            "Mint of the spl-single-pool LST (a PDA derived from `stake_pool`)",
            "",
            "because the sol_pool and stake_pool will not derive to a valid PDA which is also owned by",
            "the staking program and spl-single-pool program."
          ]
        },
        {
          "name": "solPool"
        },
        {
          "name": "stakePool",
          "docs": [
            "this key.",
            "",
            "If derives the same `bank_mint`, then this must be the correct stake pool for that mint, and",
            "we can subsequently use it to validate the `sol_pool`"
          ]
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "marginfiGroup"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "liquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolAddBankSolend",
      "docs": [
        "(admin) Add a Solend bank to the marginfi group"
      ],
      "discriminator": [
        81,
        233,
        203,
        199,
        47,
        226,
        0,
        68
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "bankMint",
          "docs": [
            "Must match the mint used by `solend_reserve`, Solend calls this the `liquidity.mint_pubkey`"
          ]
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "group"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "solendReserve",
          "docs": [
            "Solend reserve account that must match the bank mint"
          ]
        },
        {
          "name": "solendObligation",
          "docs": [
            "Obligation PDA for this bank in Solend",
            "Will be initialized and transferred to Solend in init_obligation instruction"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  101,
                  110,
                  100,
                  95,
                  111,
                  98,
                  108,
                  105,
                  103,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "For Solend banks, the `liquidity_vault` never holds assets, but is instead used as an",
            "intermediary when depositing/withdrawing, e.g., withdrawn funds move from Solend -> here ->",
            "the user's token account."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "docs": [
            "Note: Currently does nothing."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "docs": [
            "Note: Currently does nothing."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankConfig",
          "type": {
            "defined": {
              "name": "solendConfigCompact"
            }
          }
        },
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolAddBankWithSeed",
      "docs": [
        "A copy of lending_pool_add_bank with an additional bank seed.",
        "This seed is used to create a PDA for the bank's signature.",
        "lending_pool_add_bank is preserved for backwards compatibility."
      ],
      "discriminator": [
        76,
        211,
        213,
        171,
        117,
        78,
        158,
        76
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        },
        {
          "name": "feePayer",
          "docs": [
            "Pays to init accounts and pays `fee_state.bank_init_flat_sol_fee` lamports to the protocol"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalFeeWallet",
          "writable": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "bankMint"
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "marginfiGroup"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "liquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankConfig",
          "type": {
            "defined": {
              "name": "bankConfigCompact"
            }
          }
        },
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolCloneBank",
      "docs": [
        "Staging or localnet only, panics on mainnet"
      ],
      "discriminator": [
        214,
        93,
        17,
        236,
        177,
        228,
        78,
        17
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "bankMint"
        },
        {
          "name": "sourceBank",
          "docs": [
            "Source bank to clone from mainnet program",
            ""
          ]
        },
        {
          "name": "bank",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "marginfiGroup"
              },
              {
                "kind": "account",
                "path": "bankMint"
              },
              {
                "kind": "arg",
                "path": "bankSeed"
              }
            ]
          }
        },
        {
          "name": "liquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "bankSeed",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolCloseBank",
      "discriminator": [
        22,
        115,
        7,
        130,
        227,
        85,
        0,
        47
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true,
          "relations": [
            "group"
          ]
        }
      ],
      "args": []
    },
    {
      "name": "lendingPoolCollectBankFees",
      "discriminator": [
        201,
        5,
        215,
        116,
        230,
        92,
        75,
        150
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "feeAta",
          "docs": [
            "(validated in handler). Must already exist, may require initializing the ATA if it does not",
            "already exist prior to this ix."
          ],
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "lendingPoolConfigureBank",
      "docs": [
        "(admin only)"
      ],
      "discriminator": [
        121,
        173,
        156,
        40,
        93,
        148,
        56,
        237
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "bankConfigOpt",
          "type": {
            "defined": {
              "name": "bankConfigOpt"
            }
          }
        }
      ]
    },
    {
      "name": "lendingPoolConfigureBankEmode",
      "docs": [
        "(emode_admin only)"
      ],
      "discriminator": [
        17,
        175,
        91,
        57,
        239,
        86,
        49,
        71
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "emodeAdmin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "emodeTag",
          "type": "u16"
        },
        {
          "name": "entries",
          "type": {
            "array": [
              {
                "defined": {
                  "name": "emodeEntry"
                }
              },
              10
            ]
          }
        }
      ]
    },
    {
      "name": "lendingPoolConfigureBankInterestOnly",
      "docs": [
        "(delegate_curve_admin only)"
      ],
      "discriminator": [
        245,
        107,
        83,
        38,
        103,
        219,
        163,
        241
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "delegateCurveAdmin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "interestRateConfig",
          "type": {
            "defined": {
              "name": "interestRateConfigOpt"
            }
          }
        }
      ]
    },
    {
      "name": "lendingPoolConfigureBankLimitsOnly",
      "docs": [
        "(delegate_limits_admin only)"
      ],
      "discriminator": [
        157,
        196,
        221,
        200,
        202,
        62,
        84,
        21
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "delegateLimitAdmin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "depositLimit",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "borrowLimit",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "totalAssetValueInitLimit",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "lendingPoolConfigureBankOracle",
      "docs": [
        "(admin only)"
      ],
      "discriminator": [
        209,
        82,
        255,
        171,
        124,
        21,
        71,
        81
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "setup",
          "type": "u8"
        },
        {
          "name": "oracle",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "lendingPoolHandleBankruptcy",
      "docs": [
        "Handle bad debt of a bankrupt marginfi account for a given bank."
      ],
      "discriminator": [
        162,
        11,
        56,
        139,
        90,
        128,
        70,
        173
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank",
            "marginfiAccount"
          ]
        },
        {
          "name": "signer",
          "docs": [
            "PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG is not set"
          ],
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    },
    {
      "name": "lendingPoolSetFixedOraclePrice",
      "docs": [
        "(admin only)"
      ],
      "discriminator": [
        28,
        126,
        127,
        127,
        60,
        37,
        211,
        125
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "price",
          "type": {
            "defined": {
              "name": "wrappedI80f48"
            }
          }
        }
      ]
    },
    {
      "name": "lendingPoolSetupEmissions",
      "docs": [
        "(delegate_emissions_admin only)"
      ],
      "discriminator": [
        206,
        97,
        120,
        172,
        113,
        204,
        169,
        70
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "delegateEmissionsAdmin",
          "writable": true,
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "emissionsMint"
        },
        {
          "name": "emissionsAuth",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  97,
                  117,
                  116,
                  104,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "emissionsTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  116,
                  111,
                  107,
                  101,
                  110,
                  95,
                  97,
                  99,
                  99,
                  111,
                  117,
                  110,
                  116,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "emissionsFundingAccount",
          "docs": [
            "NOTE: This is a TokenAccount, spl transfer will validate it.",
            ""
          ],
          "writable": true
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "flags",
          "type": "u64"
        },
        {
          "name": "rate",
          "type": "u64"
        },
        {
          "name": "totalEmissions",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lendingPoolUpdateEmissionsParameters",
      "docs": [
        "(delegate_emissions_admin only)"
      ],
      "discriminator": [
        55,
        213,
        224,
        168,
        153,
        53,
        197,
        40
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "delegateEmissionsAdmin",
          "writable": true,
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "emissionsMint"
        },
        {
          "name": "emissionsTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  109,
                  105,
                  115,
                  115,
                  105,
                  111,
                  110,
                  115,
                  95,
                  116,
                  111,
                  107,
                  101,
                  110,
                  95,
                  97,
                  99,
                  99,
                  111,
                  117,
                  110,
                  116,
                  95,
                  115,
                  101,
                  101,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              },
              {
                "kind": "account",
                "path": "emissionsMint"
              }
            ]
          }
        },
        {
          "name": "emissionsFundingAccount",
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "emissionsFlags",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "emissionsRate",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "additionalEmissions",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "lendingPoolUpdateFeesDestinationAccount",
      "discriminator": [
        102,
        4,
        121,
        243,
        237,
        110,
        95,
        13
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "destinationAccount",
          "docs": [
            "Bank fees will be sent to this account which must be an ATA of the bank's mint."
          ]
        }
      ],
      "args": []
    },
    {
      "name": "lendingPoolWithdrawFees",
      "discriminator": [
        92,
        140,
        215,
        254,
        170,
        0,
        83,
        174
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank"
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "dstTokenAccount",
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
      "name": "lendingPoolWithdrawFeesPermissionless",
      "discriminator": [
        57,
        245,
        1,
        208,
        130,
        18,
        145,
        113
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank"
        },
        {
          "name": "feeVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feeVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "feesDestinationAccount",
          "writable": true,
          "relations": [
            "bank"
          ]
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
      "name": "lendingPoolWithdrawInsurance",
      "discriminator": [
        108,
        60,
        60,
        246,
        104,
        79,
        159,
        243
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank"
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "insuranceVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "insuranceVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  115,
                  117,
                  114,
                  97,
                  110,
                  99,
                  101,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "dstTokenAccount",
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
      "name": "marginfiAccountClose",
      "discriminator": [
        186,
        221,
        93,
        34,
        50,
        97,
        194,
        241
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "marginfiAccountInitLiqRecord",
      "discriminator": [
        236,
        213,
        238,
        126,
        147,
        251,
        164,
        8
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "liquidationRecord",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  95,
                  114,
                  101,
                  99,
                  111,
                  114,
                  100
                ]
              },
              {
                "kind": "account",
                "path": "marginfiAccount"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "marginfiAccountInitialize",
      "docs": [
        "Initialize a marginfi account for a given group. The account is a fresh keypair, and must",
        "sign. If you are a CPI caller, consider using `marginfi_account_initialize_pda` instead, or",
        "create the account manually and use `transfer_to_new_account` to gift it to the owner you",
        "wish."
      ],
      "discriminator": [
        43,
        78,
        61,
        255,
        148,
        52,
        249,
        154
      ],
      "accounts": [
        {
          "name": "marginfiGroup"
        },
        {
          "name": "marginfiAccount",
          "writable": true,
          "signer": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "marginfiAccountInitializePda",
      "docs": [
        "The same as `marginfi_account_initialize`, except the created marginfi account uses a PDA",
        "(Program Derived Address)",
        "",
        "seeds:",
        "- marginfi_group",
        "- authority: The account authority (owner)",
        "- account_index: A u16 value to allow multiple accounts per authority",
        "- third_party_id: Optional u16 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be",
        "used freely. For a dedicated seed used by just your program (via CPI), contact us."
      ],
      "discriminator": [
        87,
        177,
        91,
        80,
        218,
        119,
        245,
        31
      ],
      "accounts": [
        {
          "name": "marginfiGroup"
        },
        {
          "name": "marginfiAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110,
                  102,
                  105,
                  95,
                  97,
                  99,
                  99,
                  111,
                  117,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "marginfiGroup"
              },
              {
                "kind": "account",
                "path": "authority"
              },
              {
                "kind": "arg",
                "path": "accountIndex"
              },
              {
                "kind": "arg",
                "path": "third_party_id.unwrap_or(0)"
              }
            ]
          }
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "instructionsSysvar",
          "docs": [
            "Instructions sysvar for CPI validation",
            ""
          ],
          "address": "Sysvar1nstructions1111111111111111111111111"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "accountIndex",
          "type": "u16"
        },
        {
          "name": "thirdPartyId",
          "type": {
            "option": "u16"
          }
        }
      ]
    },
    {
      "name": "marginfiAccountUpdateEmissionsDestinationAccount",
      "discriminator": [
        73,
        185,
        162,
        201,
        111,
        24,
        116,
        185
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "destinationAccount",
          "docs": [
            "User's earned emissions will be sent to the canonical ATA of this wallet.",
            ""
          ]
        }
      ],
      "args": []
    },
    {
      "name": "marginfiGroupConfigure",
      "discriminator": [
        62,
        199,
        81,
        78,
        33,
        13,
        236,
        61
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true
        },
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marginfiGroup"
          ]
        }
      ],
      "args": [
        {
          "name": "newAdmin",
          "type": "pubkey"
        },
        {
          "name": "newEmodeAdmin",
          "type": "pubkey"
        },
        {
          "name": "newCurveAdmin",
          "type": "pubkey"
        },
        {
          "name": "newLimitAdmin",
          "type": "pubkey"
        },
        {
          "name": "newEmissionsAdmin",
          "type": "pubkey"
        },
        {
          "name": "newMetadataAdmin",
          "type": "pubkey"
        },
        {
          "name": "newRiskAdmin",
          "type": "pubkey"
        },
        {
          "name": "isArenaGroup",
          "type": "bool"
        }
      ]
    },
    {
      "name": "marginfiGroupInitialize",
      "discriminator": [
        255,
        67,
        67,
        26,
        94,
        31,
        34,
        20
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "writable": true,
          "signer": true
        },
        {
          "name": "admin",
          "writable": true,
          "signer": true
        },
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
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
          "name": "isArenaGroup",
          "type": "bool"
        }
      ]
    },
    {
      "name": "migrateCurve",
      "docs": [
        "(Permissionless) Convert a bank from the legacy curve setup to the new setup, with no effect",
        "on how interest accrues."
      ],
      "discriminator": [
        151,
        254,
        50,
        13,
        112,
        235,
        152,
        72
      ],
      "accounts": [
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "panicPause",
      "discriminator": [
        76,
        164,
        123,
        25,
        4,
        43,
        79,
        165
      ],
      "accounts": [
        {
          "name": "globalFeeAdmin",
          "docs": [
            "Admin of the global FeeState (can trigger panic pause)"
          ],
          "signer": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "feeState",
          "docs": [
            "Global fee state account containing the panic state"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        }
      ],
      "args": []
    },
    {
      "name": "panicUnpause",
      "discriminator": [
        236,
        107,
        194,
        242,
        99,
        51,
        121,
        128
      ],
      "accounts": [
        {
          "name": "globalFeeAdmin",
          "docs": [
            "Admin of the global FeeState (can manually unpause)"
          ],
          "writable": true,
          "signer": true,
          "relations": [
            "feeState"
          ]
        },
        {
          "name": "feeState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        }
      ],
      "args": []
    },
    {
      "name": "panicUnpausePermissionless",
      "docs": [
        "(permissionless) Unpause the protocol when pause time has expired"
      ],
      "discriminator": [
        245,
        139,
        50,
        159,
        213,
        62,
        91,
        248
      ],
      "accounts": [
        {
          "name": "feeState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        }
      ],
      "args": []
    },
    {
      "name": "propagateFeeState",
      "docs": [
        "(Permissionless) Force any group to adopt the current FeeState settings"
      ],
      "discriminator": [
        64,
        3,
        166,
        194,
        129,
        21,
        101,
        155
      ],
      "accounts": [
        {
          "name": "feeState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  101,
                  101,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "marginfiGroup",
          "docs": [
            "Any group, this ix is permisionless and can propagate the fee to any group"
          ],
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "propagateStakedSettings",
      "discriminator": [
        210,
        30,
        152,
        69,
        130,
        99,
        222,
        170
      ],
      "accounts": [
        {
          "name": "marginfiGroup",
          "relations": [
            "stakedSettings"
          ]
        },
        {
          "name": "stakedSettings"
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "purgeDeleverageBalance",
      "docs": [
        "(risk admin only) Purge a user's lending balance without withdrawing anything. Only usable",
        "after all the debt has been settled on a bank in deleveraging mode, e.g. when",
        "`TOKENLESS_REPAYMENTS_ALLOWED` and `TOKENLESS_REPAYMENTS_COMPLETE`. used to purge remaining",
        "lending assets in a now-worthless bank before it is fully sunset."
      ],
      "discriminator": [
        132,
        187,
        25,
        149,
        181,
        59,
        253,
        136
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "riskAdmin",
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "bank",
          "writable": true
        }
      ],
      "args": []
    },
    {
      "name": "solendDeposit",
      "docs": [
        "(user) Deposit into a Solend reserve through a marginfi account",
        "* amount - in the underlying token (e.g., USDC), in native decimals"
      ],
      "discriminator": [
        56,
        127,
        176,
        148,
        12,
        25,
        3,
        24
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "Owned by authority, the source account for the token deposit."
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The bank's liquidity vault authority, which owns the Solend obligation"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Used as an intermediary to deposit tokens into Solend"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "solendObligation",
          "docs": [
            "The Solend obligation account"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "Derived from the lending market"
          ]
        },
        {
          "name": "solendReserve",
          "docs": [
            "The Solend reserve that holds liquidity"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint (e.g., USDC)"
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "reserveLiquiditySupply",
          "docs": [
            "Reserve's liquidity supply SPL Token account"
          ],
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The reserve's mint for cTokens"
          ],
          "writable": true
        },
        {
          "name": "reserveCollateralSupply",
          "docs": [
            "The reserve's collateral supply account (where cTokens are stored)"
          ],
          "writable": true
        },
        {
          "name": "userCollateral",
          "docs": [
            "The user's destination for cTokens (collateral). This is a temporary account owned by",
            "liquidity_vault_authority that will hold cTokens between deposit and obligation update."
          ],
          "writable": true
        },
        {
          "name": "pythPrice",
          "docs": [
            "Oracle accounts - required by Solend even if not actively used"
          ]
        },
        {
          "name": "switchboardFeed"
        },
        {
          "name": "solendProgram",
          "address": "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"
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
      "name": "solendInitObligation",
      "docs": [
        "(permissionless) Initialize a Solend obligation for a marginfi bank",
        "Requires a minimum deposit to ensure the obligation remains active",
        "* amount - minimum deposit amount (at least 10 units) in native decimals"
      ],
      "discriminator": [
        81,
        96,
        123,
        149,
        218,
        116,
        235,
        196
      ],
      "accounts": [
        {
          "name": "feePayer",
          "docs": [
            "Pays to init the obligation and pays a nominal amount to ensure the obligation has a",
            "non-zero balance."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "bank"
        },
        {
          "name": "signerTokenAccount",
          "docs": [
            "The fee payer must provide a nominal amount of bank tokens so the obligation is not empty.",
            "This amount is irrecoverable and will prevent the obligation from ever being closed."
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "docs": [
            "The liquidity vault authority (PDA that will own the Solend obligation)"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "docs": [
            "Used as an intermediary to deposit a nominal amount of token into the obligation."
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "solendObligation",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  101,
                  110,
                  100,
                  95,
                  111,
                  98,
                  108,
                  105,
                  103,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          },
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingMarket"
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "Derived from the lending market"
          ]
        },
        {
          "name": "solendReserve",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint (e.g., USDC)"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "reserveLiquiditySupply",
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The reserve's mint for cTokens"
          ],
          "writable": true
        },
        {
          "name": "reserveCollateralSupply",
          "docs": [
            "The reserve's collateral supply account (where cTokens are stored)"
          ],
          "writable": true
        },
        {
          "name": "userCollateral",
          "docs": [
            "The user's destination for cTokens (collateral). This is a temporary account owned by",
            "liquidity_vault_authority that will hold cTokens between deposit and obligation update."
          ],
          "writable": true
        },
        {
          "name": "pythPrice",
          "docs": [
            "Oracle accounts - required by Solend even if not actively used"
          ]
        },
        {
          "name": "switchboardFeed"
        },
        {
          "name": "solendProgram",
          "address": "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "rent",
          "address": "SysvarRent111111111111111111111111111111111"
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
      ]
    },
    {
      "name": "solendWithdraw",
      "docs": [
        "(user) Withdraw from a Solend reserve through a marginfi account",
        "* amount - in collateral tokens (cTokens), in native decimals",
        "* withdraw_all - withdraw entire position if true"
      ],
      "discriminator": [
        238,
        144,
        170,
        199,
        21,
        72,
        155,
        36
      ],
      "accounts": [
        {
          "name": "group",
          "writable": true,
          "relations": [
            "marginfiAccount",
            "bank"
          ]
        },
        {
          "name": "marginfiAccount",
          "writable": true
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "bank",
          "writable": true
        },
        {
          "name": "destinationTokenAccount",
          "docs": [
            "Token account that will get tokens back",
            "WARN: Completely unchecked!"
          ],
          "writable": true
        },
        {
          "name": "liquidityVaultAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  113,
                  117,
                  105,
                  100,
                  105,
                  116,
                  121,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104
                ]
              },
              {
                "kind": "account",
                "path": "bank"
              }
            ]
          }
        },
        {
          "name": "liquidityVault",
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "solendObligation",
          "docs": [
            "The Solend obligation account"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "lendingMarket",
          "writable": true
        },
        {
          "name": "lendingMarketAuthority",
          "docs": [
            "Derived from the lending market"
          ]
        },
        {
          "name": "solendReserve",
          "docs": [
            "The Solend reserve that holds liquidity"
          ],
          "writable": true,
          "relations": [
            "bank"
          ]
        },
        {
          "name": "mint",
          "docs": [
            "Bank's liquidity token mint (e.g., USDC)"
          ],
          "relations": [
            "bank"
          ]
        },
        {
          "name": "reserveLiquiditySupply",
          "docs": [
            "Reserve's liquidity supply SPL Token account"
          ],
          "writable": true
        },
        {
          "name": "reserveCollateralMint",
          "docs": [
            "The reserve's mint for cTokens"
          ],
          "writable": true
        },
        {
          "name": "reserveCollateralSupply",
          "docs": [
            "The reserve's collateral supply account (where cTokens are stored)"
          ],
          "writable": true
        },
        {
          "name": "userCollateral",
          "docs": [
            "The user's destination for cTokens (collateral). This is a temporary account owned by",
            "liquidity_vault_authority that holds cTokens."
          ],
          "writable": true
        },
        {
          "name": "solendProgram",
          "address": "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "withdrawAll",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "startDeleverage",
      "discriminator": [
        10,
        138,
        10,
        57,
        40,
        232,
        182,
        193
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "docs": [
            "Account to deleverage"
          ],
          "writable": true
        },
        {
          "name": "liquidationRecord",
          "docs": [
            "The associated liquidation record PDA for the given `marginfi_account`"
          ],
          "writable": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "group",
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "riskAdmin",
          "docs": [
            "The risk admin will have the authority to withdraw/repay as if they are the user authority",
            "until the end of the tx."
          ],
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "instructionSysvar",
          "address": "Sysvar1nstructions1111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "startLiquidation",
      "discriminator": [
        244,
        93,
        90,
        214,
        192,
        166,
        191,
        21
      ],
      "accounts": [
        {
          "name": "marginfiAccount",
          "docs": [
            "Account under liquidation"
          ],
          "writable": true
        },
        {
          "name": "liquidationRecord",
          "docs": [
            "The associated liquidation record PDA for the given `marginfi_account`"
          ],
          "writable": true,
          "relations": [
            "marginfiAccount"
          ]
        },
        {
          "name": "liquidationReceiver",
          "docs": [
            "This account will have the authority to withdraw/repay as if they are the user authority",
            "until the end of the tx.",
            ""
          ]
        },
        {
          "name": "instructionSysvar",
          "address": "Sysvar1nstructions1111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "transferToNewAccount",
      "discriminator": [
        28,
        79,
        129,
        231,
        169,
        69,
        69,
        65
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "oldMarginfiAccount"
          ]
        },
        {
          "name": "oldMarginfiAccount",
          "writable": true
        },
        {
          "name": "newMarginfiAccount",
          "writable": true,
          "signer": true
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "oldMarginfiAccount"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "newAuthority"
        },
        {
          "name": "globalFeeWallet",
          "writable": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "transferToNewAccountPda",
      "docs": [
        "Same as `transfer_to_new_account` except the resulting account is a PDA",
        "",
        "seeds:",
        "- marginfi_group",
        "- authority: The account authority (owner)",
        "- account_index: A u32 value to allow multiple accounts per authority",
        "- third_party_id: Optional u32 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be",
        "used freely. For a dedicated seed used by just your program (via CPI), contact us."
      ],
      "discriminator": [
        172,
        210,
        224,
        220,
        146,
        212,
        253,
        49
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "oldMarginfiAccount"
          ]
        },
        {
          "name": "oldMarginfiAccount",
          "writable": true
        },
        {
          "name": "newMarginfiAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  103,
                  105,
                  110,
                  102,
                  105,
                  95,
                  97,
                  99,
                  99,
                  111,
                  117,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "group"
              },
              {
                "kind": "account",
                "path": "newAuthority"
              },
              {
                "kind": "arg",
                "path": "accountIndex"
              },
              {
                "kind": "arg",
                "path": "third_party_id.unwrap_or(0)"
              }
            ]
          }
        },
        {
          "name": "authority",
          "signer": true,
          "relations": [
            "oldMarginfiAccount"
          ]
        },
        {
          "name": "feePayer",
          "writable": true,
          "signer": true
        },
        {
          "name": "newAuthority"
        },
        {
          "name": "globalFeeWallet",
          "writable": true
        },
        {
          "name": "instructionsSysvar",
          "docs": [
            "Instructions sysvar for CPI validation"
          ],
          "address": "Sysvar1nstructions1111111111111111111111111"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "accountIndex",
          "type": "u16"
        },
        {
          "name": "thirdPartyId",
          "type": {
            "option": "u16"
          }
        }
      ]
    },
    {
      "name": "writeBankMetadata",
      "docs": [
        "(metadata admin only) Write ticker/descrption information for a bank on-chain. Optional, not",
        "all Banks are guranteed to have metadata."
      ],
      "discriminator": [
        147,
        78,
        81,
        133,
        129,
        138,
        233,
        59
      ],
      "accounts": [
        {
          "name": "group",
          "relations": [
            "bank"
          ]
        },
        {
          "name": "bank",
          "relations": [
            "metadata"
          ]
        },
        {
          "name": "metadataAdmin",
          "writable": true,
          "signer": true,
          "relations": [
            "group"
          ]
        },
        {
          "name": "metadata",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "ticker",
          "type": {
            "option": "bytes"
          }
        },
        {
          "name": "description",
          "type": {
            "option": "bytes"
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "bank",
      "discriminator": [
        142,
        49,
        166,
        242,
        50,
        66,
        97,
        188
      ]
    },
    {
      "name": "bankMetadata",
      "discriminator": [
        49,
        207,
        31,
        34,
        67,
        225,
        169,
        186
      ]
    },
    {
      "name": "feeState",
      "discriminator": [
        63,
        224,
        16,
        85,
        193,
        36,
        235,
        220
      ]
    },
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
      "name": "liquidationRecord",
      "discriminator": [
        95,
        116,
        23,
        132,
        89,
        210,
        245,
        162
      ]
    },
    {
      "name": "marginfiAccount",
      "discriminator": [
        67,
        178,
        130,
        109,
        126,
        114,
        28,
        42
      ]
    },
    {
      "name": "marginfiGroup",
      "discriminator": [
        182,
        23,
        173,
        240,
        151,
        206,
        182,
        67
      ]
    },
    {
      "name": "minimalObligation",
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
      "name": "minimalReserve",
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
      "name": "minimalSpotMarket",
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
      "name": "minimalUser",
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
      "name": "solendMinimalReserve",
      "discriminator": [
        1
      ]
    },
    {
      "name": "stakedSettings",
      "discriminator": [
        157,
        140,
        6,
        77,
        89,
        173,
        173,
        125
      ]
    }
  ],
  "events": [
    {
      "name": "deleverageEvent",
      "discriminator": [
        161,
        8,
        108,
        204,
        209,
        198,
        12,
        30
      ]
    },
    {
      "name": "editStakedSettingsEvent",
      "discriminator": [
        29,
        58,
        155,
        191,
        75,
        220,
        145,
        206
      ]
    },
    {
      "name": "healthPulseEvent",
      "discriminator": [
        183,
        159,
        218,
        110,
        61,
        220,
        65,
        1
      ]
    },
    {
      "name": "lendingAccountBorrowEvent",
      "discriminator": [
        223,
        96,
        81,
        10,
        156,
        99,
        26,
        59
      ]
    },
    {
      "name": "lendingAccountDepositEvent",
      "discriminator": [
        161,
        54,
        237,
        217,
        105,
        248,
        122,
        151
      ]
    },
    {
      "name": "lendingAccountLiquidateEvent",
      "discriminator": [
        166,
        160,
        249,
        154,
        183,
        39,
        23,
        242
      ]
    },
    {
      "name": "lendingAccountRepayEvent",
      "discriminator": [
        16,
        220,
        55,
        111,
        7,
        80,
        16,
        25
      ]
    },
    {
      "name": "lendingAccountWithdrawEvent",
      "discriminator": [
        3,
        220,
        148,
        243,
        33,
        249,
        54,
        88
      ]
    },
    {
      "name": "lendingPoolBankAccrueInterestEvent",
      "discriminator": [
        104,
        117,
        187,
        156,
        111,
        154,
        106,
        186
      ]
    },
    {
      "name": "lendingPoolBankCollectFeesEvent",
      "discriminator": [
        101,
        119,
        97,
        250,
        169,
        175,
        156,
        253
      ]
    },
    {
      "name": "lendingPoolBankConfigureEvent",
      "discriminator": [
        246,
        35,
        233,
        110,
        93,
        152,
        235,
        40
      ]
    },
    {
      "name": "lendingPoolBankConfigureFrozenEvent",
      "discriminator": [
        24,
        10,
        55,
        18,
        49,
        150,
        157,
        179
      ]
    },
    {
      "name": "lendingPoolBankConfigureOracleEvent",
      "discriminator": [
        119,
        140,
        110,
        253,
        150,
        64,
        210,
        62
      ]
    },
    {
      "name": "lendingPoolBankCreateEvent",
      "discriminator": [
        236,
        220,
        201,
        63,
        239,
        126,
        136,
        249
      ]
    },
    {
      "name": "lendingPoolBankHandleBankruptcyEvent",
      "discriminator": [
        166,
        77,
        41,
        140,
        36,
        94,
        10,
        57
      ]
    },
    {
      "name": "lendingPoolBankSetFixedOraclePriceEvent",
      "discriminator": [
        65,
        72,
        8,
        85,
        229,
        20,
        90,
        26
      ]
    },
    {
      "name": "liquidationReceiverEvent",
      "discriminator": [
        40,
        131,
        224,
        220,
        151,
        83,
        24,
        230
      ]
    },
    {
      "name": "marginfiAccountCreateEvent",
      "discriminator": [
        183,
        5,
        117,
        104,
        122,
        199,
        68,
        51
      ]
    },
    {
      "name": "marginfiAccountTransferToNewAccount",
      "discriminator": [
        59,
        105,
        171,
        110,
        223,
        136,
        80,
        89
      ]
    },
    {
      "name": "marginfiGroupConfigureEvent",
      "discriminator": [
        241,
        104,
        172,
        167,
        41,
        195,
        199,
        170
      ]
    },
    {
      "name": "marginfiGroupCreateEvent",
      "discriminator": [
        233,
        125,
        61,
        14,
        98,
        240,
        136,
        253
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "internalLogicError",
      "msg": "Internal Marginfi logic error"
    },
    {
      "code": 6001,
      "name": "bankNotFound",
      "msg": "Invalid bank index"
    },
    {
      "code": 6002,
      "name": "lendingAccountBalanceNotFound",
      "msg": "Lending account balance not found"
    },
    {
      "code": 6003,
      "name": "bankAssetCapacityExceeded",
      "msg": "Bank deposit capacity exceeded"
    },
    {
      "code": 6004,
      "name": "invalidTransfer",
      "msg": "Invalid transfer"
    },
    {
      "code": 6005,
      "name": "missingPythOrBankAccount",
      "msg": "Missing Oracle, Bank, LST mint, or Sol Pool"
    },
    {
      "code": 6006,
      "name": "missingPythAccount",
      "msg": "Missing Pyth account"
    },
    {
      "code": 6007,
      "name": "missingBankAccount",
      "msg": "Missing Bank account"
    },
    {
      "code": 6008,
      "name": "invalidBankAccount",
      "msg": "Invalid Bank account"
    },
    {
      "code": 6009,
      "name": "riskEngineInitRejected",
      "msg": "RiskEngine rejected due to either bad health or stale oracles"
    },
    {
      "code": 6010,
      "name": "lendingAccountBalanceSlotsFull",
      "msg": "Lending account balance slots are full"
    },
    {
      "code": 6011,
      "name": "bankAlreadyExists",
      "msg": "Bank already exists"
    },
    {
      "code": 6012,
      "name": "zeroLiquidationAmount",
      "msg": "Amount to liquidate must be positive"
    },
    {
      "code": 6013,
      "name": "accountNotBankrupt",
      "msg": "Account is not bankrupt"
    },
    {
      "code": 6014,
      "name": "balanceNotBadDebt",
      "msg": "Account balance is not bad debt"
    },
    {
      "code": 6015,
      "name": "invalidConfig",
      "msg": "Invalid group config"
    },
    {
      "code": 6016,
      "name": "bankPaused",
      "msg": "Bank paused"
    },
    {
      "code": 6017,
      "name": "bankReduceOnly",
      "msg": "Bank is ReduceOnly mode"
    },
    {
      "code": 6018,
      "name": "bankAccountNotFound",
      "msg": "Bank is missing"
    },
    {
      "code": 6019,
      "name": "operationDepositOnly",
      "msg": "Operation is deposit-only"
    },
    {
      "code": 6020,
      "name": "operationWithdrawOnly",
      "msg": "Operation is withdraw-only"
    },
    {
      "code": 6021,
      "name": "operationBorrowOnly",
      "msg": "Operation is borrow-only"
    },
    {
      "code": 6022,
      "name": "operationRepayOnly",
      "msg": "Operation is repay-only"
    },
    {
      "code": 6023,
      "name": "noAssetFound",
      "msg": "No asset found"
    },
    {
      "code": 6024,
      "name": "noLiabilityFound",
      "msg": "No liability found"
    },
    {
      "code": 6025,
      "name": "invalidOracleSetup",
      "msg": "Invalid oracle setup"
    },
    {
      "code": 6026,
      "name": "illegalUtilizationRatio",
      "msg": "Invalid bank utilization ratio"
    },
    {
      "code": 6027,
      "name": "bankLiabilityCapacityExceeded",
      "msg": "Bank borrow cap exceeded"
    },
    {
      "code": 6028,
      "name": "invalidPrice",
      "msg": "Invalid Price"
    },
    {
      "code": 6029,
      "name": "isolatedAccountIllegalState",
      "msg": "Account can have only one liability when account is under isolated risk"
    },
    {
      "code": 6030,
      "name": "emissionsAlreadySetup",
      "msg": "Emissions already setup"
    },
    {
      "code": 6031,
      "name": "oracleNotSetup",
      "msg": "Oracle is not set"
    },
    {
      "code": 6032,
      "name": "invalidSwitchboardDecimalConversion",
      "msg": "Invalid switchboard decimal conversion"
    },
    {
      "code": 6033,
      "name": "cannotCloseOutstandingEmissions",
      "msg": "Cannot close balance because of outstanding emissions"
    },
    {
      "code": 6034,
      "name": "emissionsUpdateError",
      "msg": "Update emissions error"
    },
    {
      "code": 6035,
      "name": "accountDisabled",
      "msg": "Account disabled"
    },
    {
      "code": 6036,
      "name": "accountTempActiveBalanceLimitExceeded",
      "msg": "Account can't temporarily open 3 balances, please close a balance first"
    },
    {
      "code": 6037,
      "name": "accountInFlashloan",
      "msg": "Illegal action during flashloan"
    },
    {
      "code": 6038,
      "name": "illegalFlashloan",
      "msg": "Illegal flashloan"
    },
    {
      "code": 6039,
      "name": "illegalFlag",
      "msg": "Illegal flag"
    },
    {
      "code": 6040,
      "name": "illegalBalanceState",
      "msg": "Illegal balance state"
    },
    {
      "code": 6041,
      "name": "illegalAccountAuthorityTransfer",
      "msg": "Illegal account authority transfer"
    },
    {
      "code": 6042,
      "name": "unauthorized",
      "msg": "unauthorized"
    },
    {
      "code": 6043,
      "name": "illegalAction",
      "msg": "Invalid account authority"
    },
    {
      "code": 6044,
      "name": "t22MintRequired",
      "msg": "Token22 Banks require mint account as first remaining account"
    },
    {
      "code": 6045,
      "name": "invalidFeeAta",
      "msg": "Invalid ATA for global fee account"
    },
    {
      "code": 6046,
      "name": "addedStakedPoolManually",
      "msg": "Use add pool permissionless instead"
    },
    {
      "code": 6047,
      "name": "assetTagMismatch",
      "msg": "Staked SOL accounts can only deposit staked assets and borrow SOL"
    },
    {
      "code": 6048,
      "name": "stakePoolValidationFailed",
      "msg": "Stake pool validation failed: check the stake pool, mint, or sol pool"
    },
    {
      "code": 6049,
      "name": "switchboardStalePrice",
      "msg": "Switchboard oracle: stale price"
    },
    {
      "code": 6050,
      "name": "pythPushStalePrice",
      "msg": "Pyth Push oracle: stale price"
    },
    {
      "code": 6051,
      "name": "wrongNumberOfOracleAccounts",
      "msg": "Oracle error: wrong number of accounts"
    },
    {
      "code": 6052,
      "name": "wrongOracleAccountKeys",
      "msg": "Oracle error: wrong account keys"
    },
    {
      "code": 6053,
      "name": "pythPushWrongAccountOwner",
      "msg": "Pyth Push oracle: wrong account owner"
    },
    {
      "code": 6054,
      "name": "stakedPythPushWrongAccountOwner",
      "msg": "Staked Pyth Push oracle: wrong account owner"
    },
    {
      "code": 6055,
      "name": "oracleMaxConfidenceExceeded",
      "msg": "Oracle max confidence exceeded: try again later"
    },
    {
      "code": 6056,
      "name": "pythPushInsufficientVerificationLevel",
      "msg": "Pyth Push oracle: insufficient verification level"
    },
    {
      "code": 6057,
      "name": "zeroAssetPrice",
      "msg": "Zero asset price"
    },
    {
      "code": 6058,
      "name": "zeroLiabilityPrice",
      "msg": "Zero liability price"
    },
    {
      "code": 6059,
      "name": "switchboardWrongAccountOwner",
      "msg": "Switchboard oracle: wrong account owner"
    },
    {
      "code": 6060,
      "name": "pythPushInvalidAccount",
      "msg": "Pyth Push oracle: invalid account"
    },
    {
      "code": 6061,
      "name": "switchboardInvalidAccount",
      "msg": "Switchboard oracle: invalid account"
    },
    {
      "code": 6062,
      "name": "mathError",
      "msg": "Math error"
    },
    {
      "code": 6063,
      "name": "invalidEmissionsDestinationAccount",
      "msg": "Invalid emissions destination account"
    },
    {
      "code": 6064,
      "name": "sameAssetAndLiabilityBanks",
      "msg": "Asset and liability bank cannot be the same"
    },
    {
      "code": 6065,
      "name": "overliquidationAttempt",
      "msg": "Trying to withdraw more assets than available"
    },
    {
      "code": 6066,
      "name": "noLiabilitiesInLiabilityBank",
      "msg": "Liability bank has no liabilities"
    },
    {
      "code": 6067,
      "name": "assetsInLiabilityBank",
      "msg": "Liability bank has assets"
    },
    {
      "code": 6068,
      "name": "healthyAccount",
      "msg": "Account is healthy and cannot be liquidated"
    },
    {
      "code": 6069,
      "name": "exhaustedLiability",
      "msg": "Liability payoff too severe, exhausted liability"
    },
    {
      "code": 6070,
      "name": "tooSeverePayoff",
      "msg": "Liability payoff too severe, liability balance has assets"
    },
    {
      "code": 6071,
      "name": "tooSevereLiquidation",
      "msg": "Liquidation too severe, account above maintenance requirement"
    },
    {
      "code": 6072,
      "name": "worseHealthPostLiquidation",
      "msg": "Liquidation would worsen account health"
    },
    {
      "code": 6073,
      "name": "arenaBankLimit",
      "msg": "Arena groups can only support two banks"
    },
    {
      "code": 6074,
      "name": "arenaSettingCannotChange",
      "msg": "Arena groups cannot return to non-arena status"
    },
    {
      "code": 6075,
      "name": "badEmodeConfig",
      "msg": "The Emode config was invalid"
    },
    {
      "code": 6076,
      "name": "pythPushInvalidWindowSize",
      "msg": "TWAP window size does not match expected duration"
    },
    {
      "code": 6077,
      "name": "invalidFeesDestinationAccount",
      "msg": "Invalid fees destination account"
    },
    {
      "code": 6078,
      "name": "bankCannotClose",
      "msg": "Banks cannot close when they have open positions or emissions outstanding"
    },
    {
      "code": 6079,
      "name": "accountAlreadyMigrated",
      "msg": "Account already migrated"
    },
    {
      "code": 6080,
      "name": "protocolPaused",
      "msg": "Protocol is paused"
    },
    {
      "code": 6081,
      "name": "metadataTooLong",
      "msg": "Metadata is too long"
    },
    {
      "code": 6082,
      "name": "pauseLimitExceeded",
      "msg": "Pause limit exceeded"
    },
    {
      "code": 6083,
      "name": "protocolNotPaused",
      "msg": "Protocol is not paused"
    },
    {
      "code": 6084,
      "name": "bankKilledByBankruptcy",
      "msg": "Bank killed by bankruptcy: bank shutdown and value of all holdings is zero"
    },
    {
      "code": 6085,
      "name": "unexpectedLiquidationState",
      "msg": "Liquidation state issue. Check start before end, end last, and both unique"
    },
    {
      "code": 6086,
      "name": "startNotFirst",
      "msg": "Liquidation start must be first instruction (other than compute program ixes)"
    },
    {
      "code": 6087,
      "name": "startRepeats",
      "msg": "Only one liquidation event allowed per tx"
    },
    {
      "code": 6088,
      "name": "endNotLast",
      "msg": "The end instruction must be the last ix in the tx"
    },
    {
      "code": 6089,
      "name": "forbiddenIx",
      "msg": "Tried to call an instruction that is forbidden during liquidation"
    },
    {
      "code": 6090,
      "name": "liquidationPremiumTooHigh",
      "msg": "Seized too much of the asset relative to liability repaid"
    },
    {
      "code": 6091,
      "name": "notAllowedInCpi",
      "msg": "Start and end liquidation and flashloan must be top-level instructions"
    },
    {
      "code": 6092,
      "name": "zeroSupplyInStakePool",
      "msg": "Stake pool supply is zero: cannot compute price"
    },
    {
      "code": 6093,
      "name": "invalidGroup",
      "msg": "Invalid group: account constraint violated"
    },
    {
      "code": 6094,
      "name": "invalidLiquidityVault",
      "msg": "Invalid liquidity vault: account constraint violated"
    },
    {
      "code": 6095,
      "name": "invalidLiquidationRecord",
      "msg": "Invalid liquidation record: account constraint violated"
    },
    {
      "code": 6096,
      "name": "invalidLiquidationReceiver",
      "msg": "Invalid liquidation receiver: account constraint violated"
    },
    {
      "code": 6097,
      "name": "invalidEmissionsMint",
      "msg": "Invalid emissions mint: account constraint violated"
    },
    {
      "code": 6098,
      "name": "invalidMint",
      "msg": "Invalid mint: account constraint violated"
    },
    {
      "code": 6099,
      "name": "invalidFeeWallet",
      "msg": "Invalid fee wallet: account constraint violated"
    },
    {
      "code": 6100,
      "name": "fixedOraclePriceNegative",
      "msg": "Fixed oracle price must be zero or greater"
    },
    {
      "code": 6101,
      "name": "dailyWithdrawalLimitExceeded",
      "msg": "Daily withdrawal limit exceeded: try again later"
    },
    {
      "code": 6102,
      "name": "zeroWithdrawalLimit",
      "msg": "Cannot set daily withdrawal limit to zero"
    },
    {
      "code": 6200,
      "name": "wrongAssetTagForStandardInstructions",
      "msg": "Wrong asset tag for standard instructions, expected DEFAULT, SOL, or STAKED asset tag"
    },
    {
      "code": 6201,
      "name": "wrongAssetTagForKaminoInstructions",
      "msg": "Wrong asset tag for Kamino instructions, expected KAMINO asset tag"
    },
    {
      "code": 6202,
      "name": "cantAddPool",
      "msg": "Cannot create a kamino bank with this instruction, use add_bank_kamino"
    },
    {
      "code": 6203,
      "name": "kaminoReserveMintAddressMismatch",
      "msg": "Kamino reserve mint address doesn't match the bank mint address"
    },
    {
      "code": 6204,
      "name": "kaminoDepositFailed",
      "msg": "Deposit failed: obligation deposit amount increase did not match the expected increase, left - actual, right - expected"
    },
    {
      "code": 6205,
      "name": "kaminoWithdrawFailed",
      "msg": "Withdraw failed: token vault increase did not match the expected increase, left - actual, right - expected"
    },
    {
      "code": 6206,
      "name": "reserveStale",
      "msg": "Kamino Reserve data is stale - run refresh_reserve on kamino program first"
    },
    {
      "code": 6207,
      "name": "invalidObligationDepositCount",
      "msg": "Kamino obligation must have exactly one active deposit, at index 0"
    },
    {
      "code": 6208,
      "name": "obligationDepositReserveMismatch",
      "msg": "Kamino obligation deposit doesn't match the expected reserve"
    },
    {
      "code": 6209,
      "name": "obligationInitDepositInsufficient",
      "msg": "Failed to meet minimum deposit amount requirement for init obligation"
    },
    {
      "code": 6210,
      "name": "kaminoReserveValidationFailed",
      "msg": "Kamino reserve validation failed"
    },
    {
      "code": 6211,
      "name": "kaminoInvalidOracleSetup",
      "msg": "Invalid oracle setup: only KaminoPythPush and KaminoSwitchboardPull are supported"
    },
    {
      "code": 6212,
      "name": "integrationPositionLimitExceeded",
      "msg": "Maximum integration positions limit exceeded (max 8 Kamino/Drift/Solend/JupLend positions per account)"
    },
    {
      "code": 6213,
      "name": "invalidKaminoReserve",
      "msg": "Invalid Kamino reserve: account constraint violated"
    },
    {
      "code": 6214,
      "name": "invalidKaminoObligation",
      "msg": "Invalid Kamino obligation: account constraint violated"
    },
    {
      "code": 6300,
      "name": "driftInvalidOracleSetup",
      "msg": "Invalid oracle setup: only DriftPythPull and DriftSwitchboardPull are supported"
    },
    {
      "code": 6301,
      "name": "driftSpotMarketMintMismatch",
      "msg": "Drift spot market mint does not match bank mint"
    },
    {
      "code": 6302,
      "name": "wrongBankAssetTagForDriftOperation",
      "msg": "Wrong bank asset tag for Drift operation"
    },
    {
      "code": 6303,
      "name": "cantUseStandardOperationsOnDriftAssets",
      "msg": "Cannot use standard operations on Drift assets"
    },
    {
      "code": 6304,
      "name": "driftSpotMarketValidationFailed",
      "msg": "Drift spot market validation failed"
    },
    {
      "code": 6305,
      "name": "driftInvalidSpotPositions",
      "msg": "Drift user has invalid spot positions (only first position can have balance)"
    },
    {
      "code": 6306,
      "name": "driftSpotPositionMarketMismatch",
      "msg": "Drift spot position market does not match bank's configured market"
    },
    {
      "code": 6307,
      "name": "driftInvalidPositionType",
      "msg": "Drift position has invalid balance type (must be deposit)"
    },
    {
      "code": 6308,
      "name": "driftScaledBalanceMismatch",
      "msg": "Drift scaled balance change does not match expected amount"
    },
    {
      "code": 6309,
      "name": "driftWithdrawFailed",
      "msg": "Drift withdrawal failed - token amount mismatch"
    },
    {
      "code": 6310,
      "name": "driftUserInitDepositInsufficient",
      "msg": "Drift user initial deposit insufficient (minimum 10 units required)"
    },
    {
      "code": 6311,
      "name": "invalidDriftAccount",
      "msg": "Invalid drift account"
    },
    {
      "code": 6312,
      "name": "driftAuthorityMismatch",
      "msg": "Drift authority mismatch"
    },
    {
      "code": 6313,
      "name": "driftInvalidHarvestPositionIndex",
      "msg": "Invalid harvest position index - must be between 2 and 7"
    },
    {
      "code": 6314,
      "name": "driftPositionEmpty",
      "msg": "Drift position is empty"
    },
    {
      "code": 6315,
      "name": "driftInvalidBalanceType",
      "msg": "Drift position has invalid balance type"
    },
    {
      "code": 6316,
      "name": "driftNoAdminDeposit",
      "msg": "No admin deposits found in Drift positions 2-7 for this market"
    },
    {
      "code": 6317,
      "name": "driftHarvestSameMarket",
      "msg": "Cannot harvest from the same market as the bank's main drift spot market"
    },
    {
      "code": 6318,
      "name": "driftBrickedAccount",
      "msg": "Drift account bricked: too many active deposits from admin operations"
    },
    {
      "code": 6319,
      "name": "driftMissingRewardOracle",
      "msg": "Drift reward oracle required when 2+ active deposits exist"
    },
    {
      "code": 6320,
      "name": "driftMissingRewardSpotMarket",
      "msg": "Drift reward spot market required when 2+ active deposits exist"
    },
    {
      "code": 6321,
      "name": "driftMissingRewardAccounts",
      "msg": "Drift account has admin deposits that require reward accounts to be provided"
    },
    {
      "code": 6322,
      "name": "driftSpotMarketStale",
      "msg": "Drift spot market is stale, interest needs to be updated"
    },
    {
      "code": 6323,
      "name": "invalidDriftSpotMarket",
      "msg": "Invalid Drift spot market: account constraint violated"
    },
    {
      "code": 6324,
      "name": "invalidDriftUser",
      "msg": "Invalid Drift user: account constraint violated"
    },
    {
      "code": 6325,
      "name": "invalidDriftUserStats",
      "msg": "Invalid Drift user stats: account constraint violated"
    },
    {
      "code": 6400,
      "name": "solendInvalidOracleSetup",
      "msg": "Invalid oracle setup: only SolendPythPull and SolendSwitchboardPull are supported"
    },
    {
      "code": 6401,
      "name": "solendReserveValidationFailed",
      "msg": "Solend reserve validation failed"
    },
    {
      "code": 6402,
      "name": "solendObligationOwnerMismatch",
      "msg": "Solend obligation owner mismatch"
    },
    {
      "code": 6403,
      "name": "wrongBankAssetTagForSolendOperation",
      "msg": "Wrong bank asset tag for Solend operation"
    },
    {
      "code": 6404,
      "name": "cantUseStandardOperationsOnSolendAssets",
      "msg": "Cannot use standard operations on Solend assets"
    },
    {
      "code": 6405,
      "name": "solendReserveMismatch",
      "msg": "Solend reserve mismatch"
    },
    {
      "code": 6406,
      "name": "solendReserveMintMismatch",
      "msg": "Solend reserve mint mismatch"
    },
    {
      "code": 6407,
      "name": "solendInvalidDepositPositions",
      "msg": "Solend obligation has invalid deposits (only first position can have balance)"
    },
    {
      "code": 6408,
      "name": "solendDepositPositionReserveMismatch",
      "msg": "Solend deposit position reserve does not match bank's configured reserve"
    },
    {
      "code": 6409,
      "name": "solendCTokenBalanceMismatch",
      "msg": "Solend cToken balance change does not match expected amount"
    },
    {
      "code": 6410,
      "name": "solendWithdrawFailed",
      "msg": "Solend withdrawal failed - token amount mismatch"
    },
    {
      "code": 6411,
      "name": "solendReserveStale",
      "msg": "Solend reserve is stale"
    },
    {
      "code": 6412,
      "name": "solendDepositFailed",
      "msg": "Solend deposit failed - collateral amount mismatch"
    },
    {
      "code": 6413,
      "name": "invalidSolendAccount",
      "msg": "Invalid Solend account owner"
    },
    {
      "code": 6414,
      "name": "invalidSolendAccountVersion",
      "msg": "Invalid Solend account version"
    },
    {
      "code": 6415,
      "name": "invalidSolendReserve",
      "msg": "Invalid Solend reserve: account constraint violated"
    },
    {
      "code": 6416,
      "name": "invalidSolendObligation",
      "msg": "Invalid Solend obligation: account constraint violated"
    },
    {
      "code": 6500,
      "name": "juplendInvalidOracleSetup",
      "msg": "Invalid oracle setup: only JuplendPythPull and JuplendSwitchboardPull are supported"
    },
    {
      "code": 6501,
      "name": "juplendLendingValidationFailed",
      "msg": "Juplend lending state validation failed"
    },
    {
      "code": 6502,
      "name": "wrongBankAssetTagForJuplendOperation",
      "msg": "Wrong bank asset tag for Juplend operation"
    },
    {
      "code": 6503,
      "name": "cantUseStandardOperationsOnJuplendAssets",
      "msg": "Cannot use standard operations on Juplend assets"
    },
    {
      "code": 6504,
      "name": "juplendLendingStale",
      "msg": "Juplend lending state is stale"
    },
    {
      "code": 6505,
      "name": "invalidJuplendLending",
      "msg": "Invalid Juplend lending: account constraint violated"
    },
    {
      "code": 6506,
      "name": "juplendLendingMintMismatch",
      "msg": "Juplend lending mint mismatch"
    },
    {
      "code": 6507,
      "name": "juplendBankAlreadyActivated",
      "msg": "Juplend bank is already activated"
    },
    {
      "code": 6508,
      "name": "invalidJuplendFTokenVault",
      "msg": "Invalid Juplend fToken vault"
    },
    {
      "code": 6509,
      "name": "juplendDepositFailed",
      "msg": "Juplend deposit failed"
    },
    {
      "code": 6510,
      "name": "juplendWithdrawFailed",
      "msg": "Juplend withdraw failed"
    },
    {
      "code": 6511,
      "name": "juplendInitPositionDepositInsufficient",
      "msg": "Juplend init position deposit insufficient"
    }
  ],
  "types": [
    {
      "name": "accountEventHeader",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "signer",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "marginfiAccount",
            "type": "pubkey"
          },
          {
            "name": "marginfiAccountAuthority",
            "type": "pubkey"
          },
          {
            "name": "marginfiGroup",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "balance",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "active",
            "type": "u8"
          },
          {
            "name": "bankPk",
            "type": "pubkey"
          },
          {
            "name": "bankAssetTag",
            "docs": [
              "Inherited from the bank when the position is first created and CANNOT BE CHANGED after that.",
              "Note that all balances created before the addition of this feature use `ASSET_TAG_DEFAULT`"
            ],
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "assetShares",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityShares",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "emissionsOutstanding",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "lastUpdate",
            "type": "u64"
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                1
              ]
            }
          }
        ]
      }
    },
    {
      "name": "bank",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "mintDecimals",
            "type": "u8"
          },
          {
            "name": "group",
            "type": "pubkey"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                7
              ]
            }
          },
          {
            "name": "assetShareValue",
            "docs": [
              "Monotonically increases as interest rate accumulates. For typical banks, a user's asset",
              "value in token = (number of shares the user has * asset_share_value).",
              "* A float (arbitrary decimals)",
              "* Initially 1"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityShareValue",
            "docs": [
              "Monotonically increases as interest rate accumulates. For typical banks, a user's liabilty",
              "value in token = (number of shares the user has * liability_share_value)",
              "* A float (arbitrary decimals)",
              "* Initially 1"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liquidityVault",
            "type": "pubkey"
          },
          {
            "name": "liquidityVaultBump",
            "type": "u8"
          },
          {
            "name": "liquidityVaultAuthorityBump",
            "type": "u8"
          },
          {
            "name": "insuranceVault",
            "type": "pubkey"
          },
          {
            "name": "insuranceVaultBump",
            "type": "u8"
          },
          {
            "name": "insuranceVaultAuthorityBump",
            "type": "u8"
          },
          {
            "name": "pad1",
            "type": {
              "array": [
                "u8",
                4
              ]
            }
          },
          {
            "name": "collectedInsuranceFeesOutstanding",
            "docs": [
              "Fees collected and pending withdraw for the `insurance_vault`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "feeVault",
            "type": "pubkey"
          },
          {
            "name": "feeVaultBump",
            "type": "u8"
          },
          {
            "name": "feeVaultAuthorityBump",
            "type": "u8"
          },
          {
            "name": "pad2",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "collectedGroupFeesOutstanding",
            "docs": [
              "Fees collected and pending withdraw for the `fee_vault`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "totalLiabilityShares",
            "docs": [
              "Sum of all liability shares held by all borrowers in this bank.",
              "* Uses `mint_decimals`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "totalAssetShares",
            "docs": [
              "Sum of all asset shares held by all depositors in this bank.",
              "* Uses `mint_decimals`",
              "* For Kamino banks, this is the quantity of collateral tokens (NOT liquidity tokens) in the",
              "bank, and also uses `mint_decimals`, though the mint itself will always show (6) decimals",
              "exactly (i.e Kamino ignores this and treats it as if it was using `mint_decimals`)"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "lastUpdate",
            "type": "i64"
          },
          {
            "name": "config",
            "type": {
              "defined": {
                "name": "bankConfig"
              }
            }
          },
          {
            "name": "flags",
            "docs": [
              "Bank Config Flags",
              "",
              "- EMISSIONS_FLAG_BORROW_ACTIVE: 1",
              "- EMISSIONS_FLAG_LENDING_ACTIVE: 2",
              "- PERMISSIONLESS_BAD_DEBT_SETTLEMENT: 4",
              "- FREEZE_SETTINGS: 8",
              "- CLOSE_ENABLED_FLAG: 16",
              "- TOKENLESS_REPAYMENTS_ACTIVE: 32",
              ""
            ],
            "type": "u64"
          },
          {
            "name": "emissionsRate",
            "docs": [
              "Emissions APR. Number of emitted tokens (emissions_mint) per 1e(bank.mint_decimal) tokens",
              "(bank mint) (native amount) per 1 YEAR."
            ],
            "type": "u64"
          },
          {
            "name": "emissionsRemaining",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "emissionsMint",
            "type": "pubkey"
          },
          {
            "name": "collectedProgramFeesOutstanding",
            "docs": [
              "Fees collected and pending withdraw for the `FeeState.global_fee_wallet`'s canonical ATA for `mint`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "emode",
            "docs": [
              "Controls this bank's emode configuration, which enables some banks to treat the assets of",
              "certain other banks more preferentially as collateral."
            ],
            "type": {
              "defined": {
                "name": "emodeSettings"
              }
            }
          },
          {
            "name": "feesDestinationAccount",
            "docs": [
              "Set with `update_fees_destination_account`. Fees can be withdrawn to the canonical ATA of",
              "this wallet without the admin's input (withdraw_fees_permissionless). If pubkey default, the",
              "bank doesn't support this feature, and the fees must be collected manually (withdraw_fees)."
            ],
            "type": "pubkey"
          },
          {
            "name": "cache",
            "type": {
              "defined": {
                "name": "bankCache"
              }
            }
          },
          {
            "name": "lendingPositionCount",
            "docs": [
              "Number of user lending positions currently open in this bank",
              "* For banks created prior to 0.1.4, this is the number of positions opened/closed after",
              "0.1.4 goes live, and may be negative.",
              "* For banks created in 0.1.4 or later, this is the number of positions open in total, and",
              "the bank may safely be closed if this is zero. Will never go negative."
            ],
            "type": "i32"
          },
          {
            "name": "borrowingPositionCount",
            "docs": [
              "Number of user borrowing positions currently open in this bank",
              "* For banks created prior to 0.1.4, this is the number of positions opened/closed after",
              "0.1.4 goes live, and may be negative.",
              "* For banks created in 0.1.4 or later, this is the number of positions open in total, and",
              "the bank may safely be closed if this is zero. Will never go negative."
            ],
            "type": "i32"
          },
          {
            "name": "padding0",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "kaminoReserve",
            "docs": [
              "Kamino banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "kaminoObligation",
            "docs": [
              "Kamino banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "driftSpotMarket",
            "docs": [
              "Drift banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "driftUser",
            "docs": [
              "Drift banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "driftUserStats",
            "docs": [
              "Drift banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "solendReserve",
            "docs": [
              "Solend banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "solendObligation",
            "docs": [
              "Solend banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "juplendLending",
            "docs": [
              "JupLend banks only, otherwise Pubkey default"
            ],
            "type": "pubkey"
          },
          {
            "name": "juplendFTokenVault",
            "docs": [
              "JupLend banks only: the fToken vault that holds minted fTokens"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding1",
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
      "name": "bankCache",
      "docs": [
        "A read-only cache of the bank's key metrics, e.g. spot interest/fee rates."
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "baseRate",
            "docs": [
              "Actual (spot) interest/fee rates of the bank, based on utilization",
              "* APR (annual percentage rate) values",
              "* From 0-1000%, as u32, e.g. u32::MAX = 1000%, u:32::MAX/2 = 500%, etc"
            ],
            "type": "u32"
          },
          {
            "name": "lendingRate",
            "docs": [
              "Equivalent to `base_rate` * utilization",
              "* From 0-1000%, as u32, e.g. u32::MAX = 1000%, u:32::MAX/2 = 500%, etc"
            ],
            "type": "u32"
          },
          {
            "name": "borrowingRate",
            "docs": [
              "Equivalent to `base_rate` * (1 + ir_fees) + fixed_fees",
              "* From 0-1000%, as u32, e.g. u32::MAX = 1000%, u:32::MAX/2 = 500%, etc"
            ],
            "type": "u32"
          },
          {
            "name": "interestAccumulatedFor",
            "docs": [
              "* in seconds"
            ],
            "type": "u32"
          },
          {
            "name": "accumulatedSinceLastUpdate",
            "docs": [
              "equivalent to (share value increase in the last `interest_accumulated_for` seconds *",
              "shares), i.e. the delta in `asset_share_value`, in token.",
              "* Note: if the tx that triggered this cache update increased or decreased the net shares,",
              "this value still reports using the PRE-CHANGE share amount, since interest is always",
              "earned on that amount.",
              "* in token, in native decimals, as I80F48"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "reserved0",
            "type": {
              "array": [
                "u8",
                128
              ]
            }
          }
        ]
      }
    },
    {
      "name": "bankConfig",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetWeightInit",
            "docs": [
              "TODO: Convert weights to (u64, u64) to avoid precision loss (maybe?)"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "interestRateConfig",
            "type": {
              "defined": {
                "name": "interestRateConfig"
              }
            }
          },
          {
            "name": "operationalState",
            "type": {
              "defined": {
                "name": "bankOperationalState"
              }
            }
          },
          {
            "name": "oracleSetup",
            "type": {
              "defined": {
                "name": "oracleSetup"
              }
            }
          },
          {
            "name": "oracleKeys",
            "type": {
              "array": [
                "pubkey",
                5
              ]
            }
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "borrowLimit",
            "type": "u64"
          },
          {
            "name": "riskTier",
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "assetTag",
            "docs": [
              "Determines what kinds of assets users of this bank can interact with.",
              "Options:",
              "* ASSET_TAG_DEFAULT (0) - A regular asset that can be comingled with any other regular asset",
              "or with `ASSET_TAG_SOL`",
              "* ASSET_TAG_SOL (1) - Accounts with a SOL position can comingle with **either**",
              "`ASSET_TAG_DEFAULT` or `ASSET_TAG_STAKED` positions, but not both",
              "* ASSET_TAG_STAKED (2) - Staked SOL assets. Accounts with a STAKED position can only deposit",
              "other STAKED assets or SOL (`ASSET_TAG_SOL`) and can only borrow SOL"
            ],
            "type": "u8"
          },
          {
            "name": "configFlags",
            "docs": [
              "Flags for various config options",
              "* 1 - Always set if bank created in 0.1.4 or later, or if migrated to the new pyth",
              "oracle setup from a prior version. Not set in 0.1.3 or earlier banks using pyth that have",
              "not yet migrated. Does nothing for banks that use switchboard.",
              "* 2, 4, 8, 16, etc - reserved for future use."
            ],
            "type": "u8"
          },
          {
            "name": "pad1",
            "type": {
              "array": [
                "u8",
                5
              ]
            }
          },
          {
            "name": "totalAssetValueInitLimit",
            "docs": [
              "USD denominated limit for calculating asset value for initialization margin requirements.",
              "Example, if total SOL deposits are equal to $1M and the limit it set to $500K,",
              "then SOL assets will be discounted by 50%.",
              "",
              "In other words the max value of liabilities that can be backed by the asset is $500K.",
              "This is useful for limiting the damage of orcale attacks.",
              "",
              "Value is UI USD value, for example value 100 -> $100"
            ],
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "docs": [
              "Time window in seconds for the oracle price feed to be considered live."
            ],
            "type": "u16"
          },
          {
            "name": "padding0",
            "type": {
              "array": [
                "u8",
                2
              ]
            }
          },
          {
            "name": "oracleMaxConfidence",
            "docs": [
              "From 0-100%, if the confidence exceeds this value, the oracle is considered invalid. Note:",
              "the confidence adjustment is capped at 5% regardless of this value.",
              "* 0 falls back to using the default 10% instead, i.e., U32_MAX_DIV_10",
              "* A %, as u32, e.g. 100% = u32::MAX, 50% = u32::MAX/2, etc."
            ],
            "type": "u32"
          },
          {
            "name": "fixedPrice",
            "docs": [
              "Stored oracle price for `OracleSetup::Fixed`, otherwise does nothing"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          }
        ]
      }
    },
    {
      "name": "bankConfigCompact",
      "docs": [
        "TODO: Convert weights to (u64, u64) to avoid precision loss (maybe?)"
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "interestRateConfig",
            "type": {
              "defined": {
                "name": "interestRateConfigCompact"
              }
            }
          },
          {
            "name": "operationalState",
            "type": {
              "defined": {
                "name": "bankOperationalState"
              }
            }
          },
          {
            "name": "borrowLimit",
            "type": "u64"
          },
          {
            "name": "riskTier",
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "assetTag",
            "docs": [
              "Determines what kinds of assets users of this bank can interact with.",
              "Options:",
              "* ASSET_TAG_DEFAULT (0) - A regular asset that can be comingled with any other regular asset",
              "or with `ASSET_TAG_SOL`",
              "* ASSET_TAG_SOL (1) - Accounts with a SOL position can comingle with **either**",
              "`ASSET_TAG_DEFAULT` or `ASSET_TAG_STAKED` positions, but not both",
              "* ASSET_TAG_STAKED (2) - Staked SOL assets. Accounts with a STAKED position can only deposit",
              "other STAKED assets or SOL (`ASSET_TAG_SOL`) and can only borrow SOL"
            ],
            "type": "u8"
          },
          {
            "name": "configFlags",
            "docs": [
              "Flags for various config options",
              "* 1 - Always set if bank created in 0.1.4 or later, or if migrated to the new oracle",
              "setup from a prior version. Not set in 0.1.3 or earlier banks that have not yet migrated.",
              "* 2, 4, 8, 16, etc - reserved for future use."
            ],
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                5
              ]
            }
          },
          {
            "name": "totalAssetValueInitLimit",
            "docs": [
              "USD denominated limit for calculating asset value for initialization margin requirements.",
              "Example, if total SOL deposits are equal to $1M and the limit it set to $500K,",
              "then SOL assets will be discounted by 50%.",
              "",
              "In other words the max value of liabilities that can be backed by the asset is $500K.",
              "This is useful for limiting the damage of orcale attacks.",
              "",
              "Value is UI USD value, for example value 100 -> $100"
            ],
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "docs": [
              "Time window in seconds for the oracle price feed to be considered live."
            ],
            "type": "u16"
          },
          {
            "name": "oracleMaxConfidence",
            "docs": [
              "From 0-100%, if the confidence exceeds this value, the oracle is considered invalid. Note:",
              "the confidence adjustment is capped at 5% regardless of this value.",
              "* 0% = use the default (10%)",
              "* A %, as u32, e.g. 100% = u32::MAX, 50% = u32::MAX/2, etc."
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "bankConfigOpt",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetWeightInit",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "liabilityWeightInit",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "liabilityWeightMaint",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "depositLimit",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "borrowLimit",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "operationalState",
            "type": {
              "option": {
                "defined": {
                  "name": "bankOperationalState"
                }
              }
            }
          },
          {
            "name": "interestRateConfig",
            "type": {
              "option": {
                "defined": {
                  "name": "interestRateConfigOpt"
                }
              }
            }
          },
          {
            "name": "riskTier",
            "type": {
              "option": {
                "defined": {
                  "name": "riskTier"
                }
              }
            }
          },
          {
            "name": "assetTag",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "oracleMaxConfidence",
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "oracleMaxAge",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "permissionlessBadDebtSettlement",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "freezeSettings",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "tokenlessRepaymentsAllowed",
            "type": {
              "option": "bool"
            }
          }
        ]
      }
    },
    {
      "name": "bankMetadata",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bank",
            "docs": [
              "Bank this metadata corresponds to"
            ],
            "type": "pubkey"
          },
          {
            "name": "placeholder",
            "type": "u64"
          },
          {
            "name": "ticker",
            "docs": [
              "The token's ticker name, e.g. USDC",
              "* utf-8"
            ],
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          },
          {
            "name": "description",
            "docs": [
              "The token's plain english descripion, e.g US Dollar Coin",
              "* utf-8"
            ],
            "type": {
              "array": [
                "u8",
                128
              ]
            }
          },
          {
            "name": "dataBlob",
            "docs": [
              "Reserved for future use. Room for a very small icon or something else cool"
            ],
            "type": {
              "array": [
                "u8",
                256
              ]
            }
          },
          {
            "name": "endDescriptionByte",
            "docs": [
              "The last data byte in description (padding follows)"
            ],
            "type": "u16"
          },
          {
            "name": "endDataBlob",
            "docs": [
              "The last data byte in data_blob (padding follows)"
            ],
            "type": "u16"
          },
          {
            "name": "endTickerByte",
            "docs": [
              "The last data byte in ticker (padding follows)"
            ],
            "type": "u8"
          },
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                2
              ]
            }
          }
        ]
      }
    },
    {
      "name": "bankOperationalState",
      "repr": {
        "kind": "rust"
      },
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "paused"
          },
          {
            "name": "operational"
          },
          {
            "name": "reduceOnly"
          },
          {
            "name": "killedByBankruptcy"
          }
        ]
      }
    },
    {
      "name": "deleverageEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marginfiAccount",
            "type": "pubkey"
          },
          {
            "name": "riskAdmin",
            "type": "pubkey"
          },
          {
            "name": "deleverageeAssetsSeized",
            "type": "f64"
          },
          {
            "name": "deleverageeLiabilityRepaid",
            "type": "f64"
          }
        ]
      }
    },
    {
      "name": "driftConfigCompact",
      "docs": [
        "Used to configure Drift banks. A simplified version of `BankConfigCompact` which omits most",
        "values related to interest since Drift banks cannot earn interest or be borrowed against."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "type": "pubkey"
          },
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "oracleSetup",
            "docs": [
              "Either `DriftPythPull` or `DriftSwitchboardPull`"
            ],
            "type": {
              "defined": {
                "name": "oracleSetup"
              }
            }
          },
          {
            "name": "operationalState",
            "docs": [
              "Bank operational state - allows starting banks in paused state"
            ],
            "type": {
              "defined": {
                "name": "bankOperationalState"
              }
            }
          },
          {
            "name": "riskTier",
            "docs": [
              "Risk tier - determines if assets can be borrowed in isolation"
            ],
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "configFlags",
            "docs": [
              "Config flags for future-proofing"
            ],
            "type": "u8"
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "type": "u16"
          },
          {
            "name": "oracleMaxConfidence",
            "docs": [
              "Oracle confidence threshold (0 = use default 10%)"
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "editStakedSettingsEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "group",
            "type": "pubkey"
          },
          {
            "name": "settings",
            "type": {
              "defined": {
                "name": "stakedSettingsEditConfig"
              }
            }
          }
        ]
      }
    },
    {
      "name": "emodeConfig",
      "docs": [
        "An emode configuration. Each bank has one such configuration, but this may also be the",
        "intersection of many configurations (see `reconcile_emode_configs`). For example, the risk",
        "engine creates such an intersection from all the emode config of all banks the user is borrowing",
        "from."
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "entries",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "emodeEntry"
                  }
                },
                10
              ]
            }
          }
        ]
      }
    },
    {
      "name": "emodeEntry",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "collateralBankEmodeTag",
            "docs": [
              "emode_tag of the bank(s) whose collateral you wish to treat preferentially."
            ],
            "type": "u16"
          },
          {
            "name": "flags",
            "docs": [
              "* APPLIES_TO_ISOLATED (1) - (NOT YET IMPLEMENTED) if set, isolated banks with this tag",
              "also benefit. If not set, isolated banks continue to offer zero collateral, even if they",
              "use this tag.",
              "* 2, 4, 8, 16, 32, etc - reserved for future use"
            ],
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                5
              ]
            }
          },
          {
            "name": "assetWeightInit",
            "docs": [
              "Note: If set below the collateral bank's weight, does nothing."
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "docs": [
              "Note: If set below the collateral bank's weight, does nothing."
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          }
        ]
      }
    },
    {
      "name": "emodeSettings",
      "docs": [
        "Controls the bank's e-mode configuration, allowing certain collateral sources to be treated more",
        "favorably as collateral when used to borrow from this bank."
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "emodeTag",
            "docs": [
              "This bank's NON-unique id that other banks will use to determine what emode rate to use when",
              "this bank is offered as collateral.",
              "",
              "For example, all stablecoin banks might share the same emode_tag, and in their entries, each",
              "such stablecoin bank will recognize that collateral sources with this \"stable\" tag get",
              "preferential weights. When a new stablecoin is added that is considered riskier, it may get",
              "a new, less favorable emode tag, and eventually get upgraded to the same one as the other",
              "stables",
              "",
              "* 0 is in an invalid tag and will do nothing."
            ],
            "type": "u16"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "timestamp",
            "docs": [
              "Unix timestamp from the system clock when emode state was last updated"
            ],
            "type": "i64"
          },
          {
            "name": "flags",
            "docs": [
              "EMODE_ON (1) - If set, at least one entry is configured",
              "2, 4, 8, etc, Reserved for future use"
            ],
            "type": "u64"
          },
          {
            "name": "emodeConfig",
            "type": {
              "defined": {
                "name": "emodeConfig"
              }
            }
          }
        ]
      }
    },
    {
      "name": "feeState",
      "docs": [
        "Unique per-program. The Program Owner uses this account to administrate fees collected by the protocol"
      ],
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "key",
            "docs": [
              "The fee state's own key. A PDA derived from just `b\"feestate\"`"
            ],
            "type": "pubkey"
          },
          {
            "name": "globalFeeAdmin",
            "docs": [
              "Can modify fees"
            ],
            "type": "pubkey"
          },
          {
            "name": "globalFeeWallet",
            "docs": [
              "The base wallet for all protocol fees. All SOL fees go to this wallet. All non-SOL fees go",
              "to the cannonical ATA of this wallet for that asset."
            ],
            "type": "pubkey"
          },
          {
            "name": "placeholder0",
            "type": "u64"
          },
          {
            "name": "bankInitFlatSolFee",
            "docs": [
              "Flat fee assessed when a new bank is initialized, in lamports.",
              "* In SOL, in native decimals."
            ],
            "type": "u32"
          },
          {
            "name": "bumpSeed",
            "type": "u8"
          },
          {
            "name": "padding0",
            "type": {
              "array": [
                "u8",
                3
              ]
            }
          },
          {
            "name": "liquidationMaxFee",
            "docs": [
              "Liquidators can claim at this premium, when liquidating an asset in receivership",
              "liquidation, e.g. (1 + this) * amount repaid <= asset seized",
              "* A percentage"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "programFeeFixed",
            "docs": [
              "Fee collected by the program owner from all groups",
              "* A percentage"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "programFeeRate",
            "docs": [
              "Fee collected by the program owner from all groups",
              "* A percentage"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "panicState",
            "docs": [
              "When the global admin pauses the protocol in the event of an emergency, information about",
              "the pause duration will be stored here and propagated to groups."
            ],
            "type": {
              "defined": {
                "name": "panicState"
              }
            }
          },
          {
            "name": "placeholder1",
            "type": "u64"
          },
          {
            "name": "liquidationFlatSolFee",
            "docs": [
              "Flat fee assessed for insurance/program use when a liquidation is executed",
              "* In SOL, in native decimals."
            ],
            "type": "u32"
          },
          {
            "name": "reserved0",
            "type": {
              "array": [
                "u8",
                20
              ]
            }
          },
          {
            "name": "reserved1",
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
      "name": "feeStateCache",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "globalFeeWallet",
            "type": "pubkey"
          },
          {
            "name": "programFeeFixed",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "programFeeRate",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "lastUpdate",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "groupEventHeader",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "signer",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "marginfiGroup",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "healthCache",
      "docs": [
        "A read-only cache of the internal risk engine's information. Only valid in borrow/withdraw if",
        "the tx does not fail. To see the state in any context, e.g. to figure out if the risk engine is",
        "failing due to some bad price information, use `pulse_health`."
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetValue",
            "docs": [
              "Internal risk engine asset value, using initial weight (e.g. what is used for borrowing",
              "purposes), with all confidence adjustments, and other discounts on price.",
              "* Uses EMA price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityValue",
            "docs": [
              "Internal risk engine liability value, using initial weight (e.g. what is used for borrowing",
              "purposes), with all confidence adjustments, and other discounts on price.",
              "* Uses EMA price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetValueMaint",
            "docs": [
              "Internal risk engine asset value, using maintenance weight (e.g. what is used for",
              "liquidation purposes), with all confidence adjustments.",
              "* Zero if the risk engine failed to load",
              "* Uses SPOT price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityValueMaint",
            "docs": [
              "Internal risk engine liability value, using maintenance weight (e.g. what is used for",
              "liquidation purposes), with all confidence adjustments.",
              "* Zero if the risk engine failed to load",
              "* Uses SPOT price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetValueEquity",
            "docs": [
              "The \"true\" value of assets without any confidence or weight adjustments. Internally, used",
              "only for bankruptcies.",
              "* Zero if the risk engine failed to load",
              "* Uses EMA price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityValueEquity",
            "docs": [
              "The \"true\" value of liabilities without any confidence or weight adjustments.",
              "Internally, used only for bankruptcies.",
              "* Zero if the risk engine failed to load",
              "* Uses EMA price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "timestamp",
            "docs": [
              "Unix timestamp from the system clock when this cache was last updated"
            ],
            "type": "i64"
          },
          {
            "name": "flags",
            "docs": [
              "The flags that indicate the state of the health cache. This is a u64 bitfield, where each",
              "bit represents a flag.",
              "",
              "* HEALTHY = 1 - If set, the account cannot be liquidated. If 0, the account is unhealthy and",
              "can be liquidated.",
              "* ENGINE STATUS = 2 - If set, the engine did not error during the last health pulse. If 0,",
              "the engine would have errored and this cache is likely invalid. `RiskEngineInitRejected`",
              "is ignored and will allow the flag to be set anyways.",
              "* ORACLE OK = 4 - If set, the engine did not error due to an oracle issue. If 0, engine was",
              "passed a bad bank or oracle account, or an oracle was stale. Check the order in which",
              "accounts were passed and ensure each balance has the correct banks/oracles, and that",
              "oracle cranks ran recently enough. Check `internal_err` and `err_index` for more details",
              "in some circumstances. Invalid if generated after borrow/withdraw (these instructions will",
              "ignore oracle issues if health is still satisfactory with some balance zeroed out).",
              "* 8, 16, 32, 64, 128, etc - reserved for future use"
            ],
            "type": "u32"
          },
          {
            "name": "mrgnErr",
            "docs": [
              "If the engine errored, look here for the error code. If the engine returns ok, you may also",
              "check here to see if the risk engine rejected this tx (3009)."
            ],
            "type": "u32"
          },
          {
            "name": "prices",
            "docs": [
              "Each price corresponds to that index of Balances in the LendingAccount. Useful for debugging",
              "or liquidator consumption, to determine how a user's position is priced internally.",
              "* An f64 stored as bytes"
            ],
            "type": {
              "array": [
                {
                  "array": [
                    "u8",
                    8
                  ]
                },
                16
              ]
            }
          },
          {
            "name": "internalErr",
            "docs": [
              "Errors in asset oracles are ignored (with prices treated as zero). If you see a zero price",
              "and the `ORACLE_OK` flag is not set, check here to see what error was ignored internally."
            ],
            "type": "u32"
          },
          {
            "name": "errIndex",
            "docs": [
              "Index in `balances` where `internal_err` appeared"
            ],
            "type": "u8"
          },
          {
            "name": "programVersion",
            "docs": [
              "Since 0.1.3, the version will be encoded here. See PROGRAM_VERSION."
            ],
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                2
              ]
            }
          },
          {
            "name": "internalLiqErr",
            "type": "u32"
          },
          {
            "name": "internalBankruptcyErr",
            "type": "u32"
          },
          {
            "name": "reserved0",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "reserved1",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          }
        ]
      }
    },
    {
      "name": "healthPulseEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "account",
            "type": "pubkey"
          },
          {
            "name": "healthCache",
            "type": {
              "defined": {
                "name": "healthCache"
              }
            }
          }
        ]
      }
    },
    {
      "name": "interestRateConfig",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "optimalUtilizationRate",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "plateauInterestRate",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "maxInterestRate",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "insuranceFeeFixedApr",
            "docs": [
              "Goes to insurance, funds `collected_insurance_fees_outstanding`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "insuranceIrFee",
            "docs": [
              "Goes to insurance, funds `collected_insurance_fees_outstanding`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "protocolFixedFeeApr",
            "docs": [
              "Earned by the group, goes to `collected_group_fees_outstanding`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "protocolIrFee",
            "docs": [
              "Earned by the group, goes to `collected_group_fees_outstanding`"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "protocolOriginationFee",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "zeroUtilRate",
            "docs": [
              "The base rate at utilization = 0",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": "u32"
          },
          {
            "name": "hundredUtilRate",
            "docs": [
              "The base rate at utilization = 100",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": "u32"
          },
          {
            "name": "points",
            "docs": [
              "The base rate at various points between 0 and 100%, exclusive. Essentially a piece-wise",
              "linear curve.",
              "* always in ascending order, e.g. points[0] = first kink point, points[1] = second kink",
              "point, and so forth.",
              "* points where util = 0 are unused"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "ratePoint"
                  }
                },
                5
              ]
            }
          },
          {
            "name": "curveType",
            "docs": [
              "Determines which interest rate curve implementation is active. 0 (INTEREST_CURVE_LEGACY) =",
              "legacy three point curve, 1 (INTEREST_CURVE_SEVEN_POINT) = multi-point curve."
            ],
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                7
              ]
            }
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "padding3",
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
      "name": "interestRateConfigCompact",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "insuranceFeeFixedApr",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "insuranceIrFee",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "protocolFixedFeeApr",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "protocolIrFee",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "protocolOriginationFee",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "zeroUtilRate",
            "docs": [
              "The base rate at utilization = 0",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": "u32"
          },
          {
            "name": "hundredUtilRate",
            "docs": [
              "The base rate at utilization = 100",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": "u32"
          },
          {
            "name": "points",
            "docs": [
              "The base rate at various points between 0 and 100%, exclusive. Essentially a piece-wise",
              "linear curve.",
              "* always in ascending order, e.g. points[0] = first kink point, points[1] = second kink",
              "point, and so forth.",
              "* points where util = 0 are unused"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "ratePoint"
                  }
                },
                5
              ]
            }
          }
        ]
      }
    },
    {
      "name": "interestRateConfigOpt",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "insuranceFeeFixedApr",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "insuranceIrFee",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "protocolFixedFeeApr",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "protocolIrFee",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "protocolOriginationFee",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "zeroUtilRate",
            "docs": [
              "The base rate at utilization = 0",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "hundredUtilRate",
            "docs": [
              "The base rate at utilization = 100",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "points",
            "docs": [
              "The base rate at various points between 0 and 100%, exclusive. Essentially a piece-wise",
              "linear curve.",
              "* always in ascending order, e.g. points[0] = first kink point, points[1] = second kink",
              "point, and so forth.",
              "* points where util = 0 are unused"
            ],
            "type": {
              "option": {
                "array": [
                  {
                    "defined": {
                      "name": "ratePoint"
                    }
                  },
                  5
                ]
              }
            }
          }
        ]
      }
    },
    {
      "name": "juplendConfigCompact",
      "docs": [
        "Used to configure JupLend banks. A simplified version of `BankConfigCompact` which omits most",
        "values related to interest since JupLend banks cannot earn interest or be borrowed against.",
        "",
        "Note: JupLend banks always start in `Paused` state and can only be set to `Operational`",
        "via `juplend_init_position` (seed deposit + fToken ATA)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "type": "pubkey"
          },
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "oracleSetup",
            "docs": [
              "Either `JuplendPythPull` or `JuplendSwitchboardPull`"
            ],
            "type": {
              "defined": {
                "name": "oracleSetup"
              }
            }
          },
          {
            "name": "riskTier",
            "docs": [
              "Risk tier - determines if assets can be borrowed in isolation"
            ],
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "configFlags",
            "docs": [
              "Config flags for future-proofing"
            ],
            "type": "u8"
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "type": "u16"
          },
          {
            "name": "oracleMaxConfidence",
            "docs": [
              "Oracle confidence threshold (0 = use default 10%)"
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "kaminoConfigCompact",
      "docs": [
        "Used to configure Kamino banks. A simplified version of `BankConfigCompact` which omits most",
        "values related to interest since Kamino banks cannot earn interest or be borrowed against."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "type": "pubkey"
          },
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "oracleSetup",
            "docs": [
              "Either `KaminoPythPush` or `KaminoSwitchboardPull`"
            ],
            "type": {
              "defined": {
                "name": "oracleSetup"
              }
            }
          },
          {
            "name": "operationalState",
            "docs": [
              "Bank operational state - allows starting banks in paused state"
            ],
            "type": {
              "defined": {
                "name": "bankOperationalState"
              }
            }
          },
          {
            "name": "riskTier",
            "docs": [
              "Risk tier - determines if assets can be borrowed in isolation"
            ],
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "configFlags",
            "docs": [
              "Config flags for future-proofing"
            ],
            "type": "u8"
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "docs": [
              "Currently unused: Kamino's oracle age applies to kamino banks."
            ],
            "type": "u16"
          },
          {
            "name": "oracleMaxConfidence",
            "docs": [
              "Oracle confidence threshold (0 = use default 10%)"
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "lending",
      "docs": [
        "Minimal representation of the on-chain JupLend `Lending` account.",
        "",
        "Notes:",
        "- We intentionally use a **zero-copy** layout here to match how other integrations load large",
        "external accounts (and to avoid paying Borsh (de)serialization cost on every access).",
        "- `repr(C, packed)` keeps the byte layout identical to a field-by-field serialization",
        "(i.e. no implicit padding). This is important because `Pubkey` has alignment=1 while `u64`",
        "has alignment=8; using plain `repr(C)` would insert padding before the first `u64`."
      ],
      "serialization": "bytemuckunsafe",
      "repr": {
        "kind": "c",
        "packed": true
      },
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
              "number of decimals for the fToken, same as underlying mint"
            ],
            "type": "u8"
          },
          {
            "name": "rewardsRateModel",
            "docs": [
              "PDA of rewards rate model (LRRM)"
            ],
            "type": "pubkey"
          },
          {
            "name": "liquidityExchangePrice",
            "docs": [
              "exchange price in the liquidity layer (no rewards)"
            ],
            "type": "u64"
          },
          {
            "name": "tokenExchangePrice",
            "docs": [
              "exchange price between fToken and underlying (with rewards)"
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdateTimestamp",
            "docs": [
              "unix timestamp when exchange prices were updated last"
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
      "name": "lendingAccount",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "balances",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "balance"
                  }
                },
                16
              ]
            }
          },
          {
            "name": "padding",
            "type": {
              "array": [
                "u64",
                8
              ]
            }
          }
        ]
      }
    },
    {
      "name": "lendingAccountBorrowEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "lendingAccountDepositEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "lendingAccountLiquidateEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "liquidateeMarginfiAccount",
            "type": "pubkey"
          },
          {
            "name": "liquidateeMarginfiAccountAuthority",
            "type": "pubkey"
          },
          {
            "name": "assetBank",
            "type": "pubkey"
          },
          {
            "name": "assetMint",
            "type": "pubkey"
          },
          {
            "name": "liabilityBank",
            "type": "pubkey"
          },
          {
            "name": "liabilityMint",
            "type": "pubkey"
          },
          {
            "name": "liquidateePreHealth",
            "type": "f64"
          },
          {
            "name": "liquidateePostHealth",
            "type": "f64"
          },
          {
            "name": "preBalances",
            "type": {
              "defined": {
                "name": "liquidationBalances"
              }
            }
          },
          {
            "name": "postBalances",
            "type": {
              "defined": {
                "name": "liquidationBalances"
              }
            }
          }
        ]
      }
    },
    {
      "name": "lendingAccountRepayEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "closeBalance",
            "type": "bool"
          }
        ]
      }
    },
    {
      "name": "lendingAccountWithdrawEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "closeBalance",
            "type": "bool"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankAccrueInterestEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "delta",
            "type": "u64"
          },
          {
            "name": "feesCollected",
            "type": "f64"
          },
          {
            "name": "insuranceCollected",
            "type": "f64"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankCollectFeesEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "groupFeesCollected",
            "type": "f64"
          },
          {
            "name": "groupFeesOutstanding",
            "type": "f64"
          },
          {
            "name": "insuranceFeesCollected",
            "type": "f64"
          },
          {
            "name": "insuranceFeesOutstanding",
            "type": "f64"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankConfigureEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "config",
            "type": {
              "defined": {
                "name": "bankConfigOpt"
              }
            }
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankConfigureFrozenEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "borrowLimit",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankConfigureOracleEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "oracleSetup",
            "type": "u8"
          },
          {
            "name": "oracle",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankCreateEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankHandleBankruptcyEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "badDebt",
            "type": "f64"
          },
          {
            "name": "coveredAmount",
            "type": "f64"
          },
          {
            "name": "socializedAmount",
            "type": "f64"
          }
        ]
      }
    },
    {
      "name": "lendingPoolBankSetFixedOraclePriceEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "bank",
            "type": "pubkey"
          },
          {
            "name": "price",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          }
        ]
      }
    },
    {
      "name": "liquidationBalances",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "liquidateeAssetBalance",
            "type": "f64"
          },
          {
            "name": "liquidateeLiabilityBalance",
            "type": "f64"
          },
          {
            "name": "liquidatorAssetBalance",
            "type": "f64"
          },
          {
            "name": "liquidatorLiabilityBalance",
            "type": "f64"
          }
        ]
      }
    },
    {
      "name": "liquidationCache",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetValueMaint",
            "docs": [
              "Internal risk engine asset value snapshot taken when liquidation begins, using maintenance",
              "weight with all confidence adjustments.",
              "* Uses SPOT price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityValueMaint",
            "docs": [
              "Internal risk engine liability value snapshot taken when liquidation begins, using",
              "maintenance weight with all confidence adjustments.",
              "* Uses SPOT price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetValueEquity",
            "docs": [
              "Actual cash value of assets pre-liquidation (inclusive of price adjustment for oracle",
              "confidence, but without any weights)",
              "* Liquidator is allowed to seize up to `liability_value_equity` - this amount",
              "* Uses EMA price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "liabilityValueEquity",
            "docs": [
              "Actual cash value of liabilities pre-liquidation (inclusive of price adjustment for oracle",
              "confidence, but without any weights)",
              "* Liquidator is allowed to seize up to this amount - `asset_value_equity`",
              "* Uses EMA price",
              "* In dollars"
            ],
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "placeholder",
            "type": "u64"
          },
          {
            "name": "reserved0",
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
      "name": "liquidationEntry",
      "docs": [
        "Used to record key details of the last few liquidation events on the account"
      ],
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "assetAmountSeized",
            "docs": [
              "Dollar amount seized",
              "* An f64 stored as bytes"
            ],
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "liabAmountRepaid",
            "docs": [
              "Dollar amount repaid",
              "* An f64 stored as bytes"
            ],
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "placeholder0",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          },
          {
            "name": "reserved0",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          }
        ]
      }
    },
    {
      "name": "liquidationReceiverEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marginfiAccount",
            "type": "pubkey"
          },
          {
            "name": "liquidationReceiver",
            "type": "pubkey"
          },
          {
            "name": "liquidateeAssetsSeized",
            "type": "f64"
          },
          {
            "name": "liquidateeLiabilityRepaid",
            "type": "f64"
          },
          {
            "name": "lampsFeePaid",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "liquidationRecord",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "key",
            "docs": [
              "This account's own key. A PDA derived from `marginfi_account`"
            ],
            "type": "pubkey"
          },
          {
            "name": "marginfiAccount",
            "docs": [
              "Account this record tracks"
            ],
            "type": "pubkey"
          },
          {
            "name": "recordPayer",
            "docs": [
              "The key that paid to create this account. At some point, we may allow this wallet to reclaim",
              "the rent paid to open a record."
            ],
            "type": "pubkey"
          },
          {
            "name": "liquidationReceiver",
            "docs": [
              "The liquidator taking receivership of the `marginfi_account` to complete a liquidation. Pays",
              "the liquidation fee.",
              "* Always pubkey default unless actively within a liquidation event."
            ],
            "type": "pubkey"
          },
          {
            "name": "entries",
            "docs": [
              "Basic historical data for the last few liquidation events on this account"
            ],
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "liquidationEntry"
                  }
                },
                4
              ]
            }
          },
          {
            "name": "cache",
            "type": {
              "defined": {
                "name": "liquidationCache"
              }
            }
          },
          {
            "name": "reserved0",
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          },
          {
            "name": "reserved2",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "reserved3",
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
      "name": "marginfiAccount",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "group",
            "type": "pubkey"
          },
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "lendingAccount",
            "type": {
              "defined": {
                "name": "lendingAccount"
              }
            }
          },
          {
            "name": "accountFlags",
            "docs": [
              "The flags that indicate the state of the account. This is u64 bitfield, where each bit",
              "represents a flag.",
              "",
              "Flags:MarginfiAccount",
              "- 1: `ACCOUNT_DISABLED` - Indicates that the account is disabled and no further actions can",
              "be taken on it.",
              "- 2: `ACCOUNT_IN_FLASHLOAN` - Only set when an account is within a flash loan, e.g. when",
              "start_flashloan is called, then unset when the flashloan ends.",
              "- 4: `ACCOUNT_FLAG_DEPRECATED` - Deprecated, available for future use",
              "- 8: `ACCOUNT_TRANSFER_AUTHORITY_DEPRECATED` - the admin has flagged with account to be",
              "moved, original owner can now call `set_account_transfer_authority`",
              "- 16: `ACCOUNT_IN_RECEIVERSHIP` - the account is eligible to be liquidated and has entered",
              "receivership, a liquidator is able to control borrows and withdraws until the end of the",
              "tx. This flag will only appear within a tx."
            ],
            "type": "u64"
          },
          {
            "name": "emissionsDestinationAccount",
            "docs": [
              "Set with `update_emissions_destination_account`. Emissions rewards can be withdrawn to the",
              "cannonical ATA of this wallet without the user's input (withdraw_emissions_permissionless).",
              "If pubkey default, the user has not opted into this feature, and must claim emissions",
              "manually (withdraw_emissions)."
            ],
            "type": "pubkey"
          },
          {
            "name": "healthCache",
            "type": {
              "defined": {
                "name": "healthCache"
              }
            }
          },
          {
            "name": "migratedFrom",
            "docs": [
              "If this account was migrated from another one, store the original account key"
            ],
            "type": "pubkey"
          },
          {
            "name": "migratedTo",
            "docs": [
              "If this account has been migrated to another one, store the destination account key"
            ],
            "type": "pubkey"
          },
          {
            "name": "lastUpdate",
            "type": "u64"
          },
          {
            "name": "accountIndex",
            "docs": [
              "If a PDA-based account, the account index, a seed used to derive the PDA that can be chosen",
              "arbitrarily (0.1.5 or later). Otherwise, does nothing."
            ],
            "type": "u16"
          },
          {
            "name": "thirdPartyIndex",
            "docs": [
              "If a PDA-based account (0.1.5 or later), a \"vendor specific\" id. Values < PDA_FREE_THRESHOLD",
              "can be used by anyone with no restrictions. Values >= PDA_FREE_THRESHOLD can only be used by",
              "a particular program via CPI. These values require being added to a list, contact us for",
              "more details. For legacy non-pda accounts, does nothing.",
              "",
              "Note: use a unique seed to tag accounts related to some particular program or campaign so",
              "you can easily fetch them all later."
            ],
            "type": "u16"
          },
          {
            "name": "bump",
            "docs": [
              "This account's bump, if a PDA-based account (0.1.5 or later). Otherwise, does nothing."
            ],
            "type": "u8"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                3
              ]
            }
          },
          {
            "name": "liquidationRecord",
            "docs": [
              "Stores information related to liquidations made against this account. A pda of this",
              "account's key, and \"liq_record\"",
              "* Typically pubkey default if this account has never been liquidated or close to liquidation",
              "* Opening this account is permissionless. Typically the liquidator pays, but e.g. we may",
              "also charge the user if they are opening a risky position on the front end."
            ],
            "type": "pubkey"
          },
          {
            "name": "padding0",
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
      "name": "marginfiAccountCreateEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          }
        ]
      }
    },
    {
      "name": "marginfiAccountTransferToNewAccount",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "accountEventHeader"
              }
            }
          },
          {
            "name": "oldAccount",
            "type": "pubkey"
          },
          {
            "name": "oldAccountAuthority",
            "type": "pubkey"
          },
          {
            "name": "newAccountAuthority",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "marginfiGroup",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "admin",
            "docs": [
              "Broadly able to modify anything, and can set/remove other admins at will."
            ],
            "type": "pubkey"
          },
          {
            "name": "groupFlags",
            "docs": [
              "Bitmask for group settings flags.",
              "* 0: `PROGRAM_FEES_ENABLED` If set, program-level fees are enabled.",
              "* 1: `ARENA_GROUP` If set, this is an arena group, which can only have two banks",
              "* Bits 2-63: Reserved for future use."
            ],
            "type": "u64"
          },
          {
            "name": "feeStateCache",
            "docs": [
              "Caches information from the global `FeeState` so the FeeState can be omitted on certain ixes"
            ],
            "type": {
              "defined": {
                "name": "feeStateCache"
              }
            }
          },
          {
            "name": "banks",
            "type": "u16"
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "emodeAdmin",
            "docs": [
              "This admin can configure collateral ratios above (but not below) the collateral ratio of",
              "certain banks , e.g. allow SOL to count as 90% collateral when borrowing an LST instead of",
              "the default rate."
            ],
            "type": "pubkey"
          },
          {
            "name": "delegateCurveAdmin",
            "type": "pubkey"
          },
          {
            "name": "delegateLimitAdmin",
            "docs": [
              "Can modify the `deposit_limit`, `borrow_limit`, `total_asset_value_init_limit` but nothing",
              "else, for every bank under this group"
            ],
            "type": "pubkey"
          },
          {
            "name": "delegateEmissionsAdmin",
            "docs": [
              "Can modify the emissions `flags`, `emissions_rate` and `emissions_mint`, but nothing else,",
              "for every bank under this group"
            ],
            "type": "pubkey"
          },
          {
            "name": "panicStateCache",
            "docs": [
              "When program keeper temporarily puts the program into panic mode, information about the",
              "duration of the lockup will be available here."
            ],
            "type": {
              "defined": {
                "name": "panicStateCache"
              }
            }
          },
          {
            "name": "deleverageWithdrawWindowCache",
            "docs": [
              "Keeps track of the liquidity withdrawn from the group over the day as a result of",
              "deleverages. Used as a protection mechanism against too big (and unwanted) withdrawals (e.g.",
              "when the risk admin is compromised)."
            ],
            "type": {
              "defined": {
                "name": "withdrawWindowCache"
              }
            }
          },
          {
            "name": "riskAdmin",
            "docs": [
              "Can run bankruptcy and forced deleverage ixes to e.g. sunset risky/illiquid assets"
            ],
            "type": "pubkey"
          },
          {
            "name": "metadataAdmin",
            "docs": [
              "Can modify a Bank's metadata, and nothing else."
            ],
            "type": "pubkey"
          },
          {
            "name": "padding0",
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    2
                  ]
                },
                12
              ]
            }
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    2
                  ]
                },
                32
              ]
            }
          }
        ]
      }
    },
    {
      "name": "marginfiGroupConfigureEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          },
          {
            "name": "admin",
            "type": "pubkey"
          },
          {
            "name": "flags",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "marginfiGroupCreateEvent",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "header",
            "type": {
              "defined": {
                "name": "groupEventHeader"
              }
            }
          }
        ]
      }
    },
    {
      "name": "minimalObligation",
      "docs": [
        "A minimal copy of Kamino's Obligation for zero-copy deserialization"
      ],
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "tag",
            "type": "u64"
          },
          {
            "name": "lastUpdateSlot",
            "docs": [
              "Kamino obligations are only good for one slot, e.g. `refresh_obligation` must have run within the",
              "same slot as any ix that needs a non-stale obligation e.g. withdraw."
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdateStale",
            "docs": [
              "True if the obligation is stale, which will cause various ixes like withdraw to fail. Typically",
              "set to true in any tx that modifies obligation balance, and set to false at the end of a",
              "successful `refresh_obligation`",
              "* 0 = false, 1 = true"
            ],
            "type": "u8"
          },
          {
            "name": "lastUpdatePriceStatus",
            "docs": [
              "Each bit represents a passed check in price status.",
              "* 63 = all checks passed",
              "",
              "Otherwise:",
              "* PRICE_LOADED =        0b_0000_0001; // 1",
              "* PRICE_AGE_CHECKED =   0b_0000_0010; // 2",
              "* TWAP_CHECKED =        0b_0000_0100; // 4",
              "* TWAP_AGE_CHECKED =    0b_0000_1000; // 8",
              "* HEURISTIC_CHECKED =   0b_0001_0000; // 16",
              "* PRICE_USAGE_ALLOWED = 0b_0010_0000; // 32"
            ],
            "type": "u8"
          },
          {
            "name": "lastUpdatePlaceholder",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "lendingMarket",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "docs": [
              "For mrgn banks, the bank's Liquidity Vault Authority (a pda which can be derived if the bank",
              "key is known)"
            ],
            "type": "pubkey"
          },
          {
            "name": "deposits",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "minimalObligationCollateral"
                  }
                },
                8
              ]
            }
          },
          {
            "name": "lowestReserveDepositLiquidationLtv",
            "type": "u64"
          },
          {
            "name": "depositedValueSf",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "paddingPart1",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart2",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart3",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart4",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart5a",
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          },
          {
            "name": "paddingPart5c",
            "type": {
              "array": [
                "u8",
                24
              ]
            }
          }
        ]
      }
    },
    {
      "name": "minimalObligationCollateral",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "depositReserve",
            "type": "pubkey"
          },
          {
            "name": "depositedAmount",
            "docs": [
              "In collateral token (NOT liquidity token), use `collateral_to_liquidity` to convert back to",
              "liquidity token!",
              "* Always 6 decimals"
            ],
            "type": "u64"
          },
          {
            "name": "marketValueSf",
            "docs": [
              "* In dollars, based on last oracle price update",
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.",
              "* A float (arbitrary decimals)"
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "borrowedAmountAgainstThisCollateralInElevationGroup",
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
      "name": "minimalReserve",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u64"
          },
          {
            "name": "slot",
            "docs": [
              "Kamino reserves are only good for one slot, e.g. `refresh_reserve` must have run within the",
              "same slot as any ix that needs a non-stale reserve e.g. withdraw."
            ],
            "type": "u64"
          },
          {
            "name": "stale",
            "docs": [
              "True if the reserve is stale, which will cause various ixes like withdraw to fail. Typically",
              "set to true in any tx that modifies reserve balance, and set to false at the end of a",
              "successful `refresh_reserve`",
              "* 0 = false, 1 = true"
            ],
            "type": "u8"
          },
          {
            "name": "priceStatus",
            "docs": [
              "Each bit represents a passed check in price status.",
              "* 63 = all checks passed",
              "",
              "Otherwise:",
              "* PRICE_LOADED =        0b_0000_0001; // 1",
              "* PRICE_AGE_CHECKED =   0b_0000_0010; // 2",
              "* TWAP_CHECKED =        0b_0000_0100; // 4",
              "* TWAP_AGE_CHECKED =    0b_0000_1000; // 8",
              "* HEURISTIC_CHECKED =   0b_0001_0000; // 16",
              "* PRICE_USAGE_ALLOWED = 0b_0010_0000; // 32"
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
          },
          {
            "name": "lendingMarket",
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
            "name": "mintPubkey",
            "type": "pubkey"
          },
          {
            "name": "supplyVault",
            "docs": [
              "* A PDA"
            ],
            "type": "pubkey"
          },
          {
            "name": "feeVault",
            "docs": [
              "* A PDA"
            ],
            "type": "pubkey"
          },
          {
            "name": "availableAmount",
            "docs": [
              "In simple terms: (amount in supply vault - outstanding borrows)",
              "* In token, with `mint_decimals`"
            ],
            "type": "u64"
          },
          {
            "name": "borrowedAmountSf",
            "docs": [
              "* In token, with `mint_decimals`",
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino."
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "marketPriceSf",
            "docs": [
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino."
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "marketPriceLastUpdatedTs",
            "type": "u64"
          },
          {
            "name": "mintDecimals",
            "type": "u64"
          },
          {
            "name": "depositLimitCrossedTimestamp",
            "type": "u64"
          },
          {
            "name": "borrowLimitCrossedTimestamp",
            "type": "u64"
          },
          {
            "name": "cumulativeBorrowRateBsf",
            "type": {
              "array": [
                "u8",
                48
              ]
            }
          },
          {
            "name": "accumulatedProtocolFeesSf",
            "docs": [
              "* In token, with `mint_decimals`",
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino."
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "accumulatedReferrerFeesSf",
            "docs": [
              "* In token, with `mint_decimals`",
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino."
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "pendingReferrerFeesSf",
            "docs": [
              "* In token, with `mint_decimals`",
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino."
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "absoluteReferralRateSf",
            "docs": [
              "* In token, with `mint_decimals`",
              "* Actually an I68F60, stored as a u128 (i.e. BN) in Kamino."
            ],
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "tokenProgram",
            "docs": [
              "Token or Token22. If token22, note that Kamino does not support all Token22 extensions."
            ],
            "type": "pubkey"
          },
          {
            "name": "padding2Part1",
            "type": {
              "array": [
                "u8",
                256
              ]
            }
          },
          {
            "name": "padding2Part2",
            "type": {
              "array": [
                "u8",
                128
              ]
            }
          },
          {
            "name": "padding2Part3",
            "type": {
              "array": [
                "u8",
                24
              ]
            }
          },
          {
            "name": "padding3",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart1",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart2",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "paddingPart3",
            "type": {
              "array": [
                "u8",
                128
              ]
            }
          },
          {
            "name": "paddingPart4",
            "type": {
              "array": [
                "u8",
                48
              ]
            }
          },
          {
            "name": "collateralMintPubkey",
            "docs": [
              "Mints collateral tokens",
              "* A PDA",
              "* technically 6 decimals, but uses `mint_decimals` regardless for all purposes",
              "* authority = lending_market_authority"
            ],
            "type": "pubkey"
          },
          {
            "name": "mintTotalSupply",
            "docs": [
              "Total number of collateral tokens",
              "* uses `mint_decimals`, even though it's technically 6 decimals under the hood"
            ],
            "type": "u64"
          },
          {
            "name": "collateralSupplyVault",
            "docs": [
              "* A PDA"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding1ReserveCollateral",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "padding2ReserveCollateral",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "padding4Part1",
            "type": {
              "array": [
                "u8",
                4096
              ]
            }
          },
          {
            "name": "padding4Part2",
            "type": {
              "array": [
                "u8",
                512
              ]
            }
          },
          {
            "name": "padding4Part3",
            "type": {
              "array": [
                "u8",
                256
              ]
            }
          },
          {
            "name": "padding4Part4",
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          },
          {
            "name": "padding4Part5",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "padding4Part6",
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
      "name": "minimalSpotMarket",
      "docs": [
        "Minimal representation of Drift's SpotMarket account",
        "Only includes the fields we actually need for marginfi integration",
        "https://github.com/drift-labs/protocol-v2/tree/master/programs/drift/src/state/spot_market.rs#L35"
      ],
      "serialization": "bytemuckunsafe",
      "repr": {
        "kind": "c"
      },
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
              "The vault used to store the market's deposits"
            ],
            "type": "pubkey"
          },
          {
            "name": "padding1",
            "type": {
              "array": [
                "u8",
                296
              ]
            }
          },
          {
            "name": "depositBalance",
            "docs": [
              "All the fields we need for testing (stored as raw bytes for simplicity)"
            ],
            "type": "u128"
          },
          {
            "name": "borrowBalance",
            "type": "u128"
          },
          {
            "name": "cumulativeDepositInterest",
            "type": "u128"
          },
          {
            "name": "cumulativeBorrowInterest",
            "type": "u128"
          },
          {
            "name": "padding2",
            "type": {
              "array": [
                "u8",
                72
              ]
            }
          },
          {
            "name": "lastInterestTs",
            "docs": [
              "Last time the cumulative deposit and borrow interest was updated",
              "Offset: 568 bytes from start of struct"
            ],
            "type": "u64"
          },
          {
            "name": "padding2b",
            "type": {
              "array": [
                "u8",
                104
              ]
            }
          },
          {
            "name": "decimals",
            "type": "u32"
          },
          {
            "name": "marketIndex",
            "type": "u16"
          },
          {
            "name": "padding3",
            "type": {
              "array": [
                "u8",
                49
              ]
            }
          },
          {
            "name": "poolId",
            "type": "u8"
          },
          {
            "name": "padding4",
            "docs": [
              "Padding to reach 776 bytes total"
            ],
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
      "name": "minimalUser",
      "docs": [
        "Minimal representation of Drift's User account",
        "Only includes the fields we actually need"
      ],
      "serialization": "bytemuckunsafe",
      "repr": {
        "kind": "c"
      },
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
              "An addresses that can control the account on the authority's behalf"
            ],
            "type": "pubkey"
          },
          {
            "name": "name",
            "docs": [
              "Encoded display name for the account"
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
              "The user's spot positions (8 positions)"
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
            "name": "padding1",
            "docs": [
              "Skip to the fields we need at the end",
              "Total size needed: 4376 - 96 (header) - 352 (positions) = 3928 bytes"
            ],
            "type": {
              "array": [
                "u8",
                3920
              ]
            }
          },
          {
            "name": "subAccountId",
            "docs": [
              "Sub account id for this user account"
            ],
            "type": "u16"
          },
          {
            "name": "status",
            "type": "u8"
          },
          {
            "name": "isBeingLiquidated",
            "docs": [
              "Whether the user is being liquidated"
            ],
            "type": "u8"
          },
          {
            "name": "padding2",
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
      "name": "oracleSetup",
      "repr": {
        "kind": "rust"
      },
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "none"
          },
          {
            "name": "pythLegacy"
          },
          {
            "name": "switchboardV2"
          },
          {
            "name": "pythPushOracle"
          },
          {
            "name": "switchboardPull"
          },
          {
            "name": "stakedWithPythPush"
          },
          {
            "name": "kaminoPythPush"
          },
          {
            "name": "kaminoSwitchboardPull"
          },
          {
            "name": "fixed"
          },
          {
            "name": "driftPythPull"
          },
          {
            "name": "driftSwitchboardPull"
          },
          {
            "name": "solendPythPull"
          },
          {
            "name": "solendSwitchboardPull"
          },
          {
            "name": "juplendPythPull"
          },
          {
            "name": "juplendSwitchboardPull"
          }
        ]
      }
    },
    {
      "name": "panicState",
      "docs": [
        "Panic state for emergency protocol pausing"
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pauseFlags",
            "docs": [
              "Whether the protocol is currently paused (1 = paused, 0 = not paused)"
            ],
            "type": "u8"
          },
          {
            "name": "dailyPauseCount",
            "docs": [
              "Number of times paused today (resets every 24 hours)"
            ],
            "type": "u8"
          },
          {
            "name": "consecutivePauseCount",
            "docs": [
              "Number of consecutive pauses (resets when unpause happens)"
            ],
            "type": "u8"
          },
          {
            "name": "reserved",
            "type": {
              "array": [
                "u8",
                5
              ]
            }
          },
          {
            "name": "pauseStartTimestamp",
            "docs": [
              "Timestamp when the current pause started (0 if not paused)",
              "* When a pause is extended before expiring, this could be in the future."
            ],
            "type": "i64"
          },
          {
            "name": "lastDailyResetTimestamp",
            "docs": [
              "Timestamp of the last daily reset (for tracking daily pause count)"
            ],
            "type": "i64"
          },
          {
            "name": "reservedSpace",
            "docs": [
              "Reserved for future use (making total struct 32 bytes)"
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
      "name": "panicStateCache",
      "docs": [
        "Cached panic state information for fast checking during user operations"
      ],
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "pauseFlags",
            "docs": [
              "Whether the protocol is currently paused (1 = paused, 0 = not paused)"
            ],
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
            "name": "pauseStartTimestamp",
            "docs": [
              "Timestamp when the current pause started (0 if not paused)"
            ],
            "type": "i64"
          },
          {
            "name": "lastCacheUpdate",
            "docs": [
              "Timestamp when this cache was last updated"
            ],
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "ratePoint",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "util",
            "docs": [
              "The utilization rate where `rate` applies",
              "* a %, as u32, out of 100%, e.g. 50% = .5 * u32::MAX"
            ],
            "type": "u32"
          },
          {
            "name": "rate",
            "docs": [
              "The base rate that applies",
              "* a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX"
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "riskTier",
      "repr": {
        "kind": "rust"
      },
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "collateral"
          },
          {
            "name": "isolated"
          }
        ]
      }
    },
    {
      "name": "solendConfigCompact",
      "docs": [
        "Used to configure Solend banks. A simplified version of `BankConfigCompact` which omits most",
        "values related to interest since Solend banks cannot earn interest or be borrowed against."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "type": "pubkey"
          },
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "oracleSetup",
            "docs": [
              "Either `SolendPythPull` or `SolendSwitchboardPull`"
            ],
            "type": {
              "defined": {
                "name": "oracleSetup"
              }
            }
          },
          {
            "name": "operationalState",
            "docs": [
              "Bank operational state - allows starting banks in paused state"
            ],
            "type": {
              "defined": {
                "name": "bankOperationalState"
              }
            }
          },
          {
            "name": "riskTier",
            "docs": [
              "Risk tier - determines if assets can be borrowed in isolation"
            ],
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "configFlags",
            "docs": [
              "Config flags for future-proofing"
            ],
            "type": "u8"
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "type": "u16"
          },
          {
            "name": "oracleMaxConfidence",
            "docs": [
              "Oracle confidence threshold (0 = use default 10%)"
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "solendMinimalReserve",
      "serialization": "bytemuckunsafe",
      "repr": {
        "kind": "c",
        "packed": true
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lastUpdateSlot",
            "docs": [
              "Last slot when supply and rates updated"
            ],
            "type": "u64"
          },
          {
            "name": "lastUpdateStale",
            "docs": [
              "True when marked stale"
            ],
            "type": "u8"
          },
          {
            "name": "lendingMarket",
            "docs": [
              "Lending market address"
            ],
            "type": "pubkey"
          },
          {
            "name": "liquidityMintPubkey",
            "type": "pubkey"
          },
          {
            "name": "liquidityMintDecimals",
            "type": "u8"
          },
          {
            "name": "liquiditySupplyPubkey",
            "type": "pubkey"
          },
          {
            "name": "liquidityPythOraclePubkey",
            "type": "pubkey"
          },
          {
            "name": "liquiditySwitchboardOraclePubkey",
            "type": "pubkey"
          },
          {
            "name": "liquidityAvailableAmount",
            "type": "u64"
          },
          {
            "name": "liquidityBorrowedAmountWads",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "liquidityCumulativeBorrowRateWads",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "liquidityMarketPrice",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "collateralMintPubkey",
            "type": "pubkey"
          },
          {
            "name": "collateralMintTotalSupply",
            "type": "u64"
          },
          {
            "name": "collateralSupplyPubkey",
            "type": "pubkey"
          },
          {
            "name": "configOptimalUtilizationRate",
            "type": "u8"
          },
          {
            "name": "configLoanToValueRatio",
            "type": "u8"
          },
          {
            "name": "configLiquidationBonus",
            "type": "u8"
          },
          {
            "name": "configLiquidationThreshold",
            "type": "u8"
          },
          {
            "name": "paddingToFees64",
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          },
          {
            "name": "paddingToFees6",
            "type": {
              "array": [
                "u8",
                6
              ]
            }
          },
          {
            "name": "liquidityAccumulatedProtocolFeesWads",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          },
          {
            "name": "paddingFinal128",
            "type": {
              "array": [
                "u8",
                128
              ]
            }
          },
          {
            "name": "paddingFinal64",
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          },
          {
            "name": "paddingFinal32",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "paddingFinal6",
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
      "name": "spotBalanceType",
      "repr": {
        "kind": "rust"
      },
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
      "name": "spotPosition",
      "docs": [
        "Minimal representation of a spot position within a User account"
      ],
      "serialization": "bytemuckunsafe",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "scaledBalance",
            "docs": [
              "The scaled balance of the position.",
              "* Precision: SPOT_BALANCE_PRECISION"
            ],
            "type": "u64"
          },
          {
            "name": "openBids",
            "docs": [
              "How many spot bids the user has open",
              "* Precision: token mint precision"
            ],
            "type": "i64"
          },
          {
            "name": "openAsks",
            "docs": [
              "How many spot asks the user has open",
              "* Precision: token mint precision"
            ],
            "type": "i64"
          },
          {
            "name": "cumulativeDeposits",
            "docs": [
              "The cumulative deposits/borrows a user has made",
              "* Precision: token mint precision"
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
            "docs": [
              "Padding"
            ],
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
      "name": "stakedSettings",
      "docs": [
        "Unique per-group. Staked Collateral banks created under a group automatically use these",
        "settings. Groups that have not created this struct cannot create staked collateral banks. When",
        "this struct updates, changes must be permissionlessly propogated to staked collateral banks.",
        "Administrators can also edit the bank manually, i.e. with configure_bank, to temporarily make",
        "changes such as raising the deposit limit for a single bank."
      ],
      "serialization": "bytemuck",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "key",
            "docs": [
              "This account's own key. A PDA derived from `marginfi_group` and `STAKED_SETTINGS_SEED`"
            ],
            "type": "pubkey"
          },
          {
            "name": "marginfiGroup",
            "docs": [
              "Group for which these settings apply"
            ],
            "type": "pubkey"
          },
          {
            "name": "oracle",
            "docs": [
              "Generally, the Pyth push oracle for SOL"
            ],
            "type": "pubkey"
          },
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "type": "u16"
          },
          {
            "name": "riskTier",
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          },
          {
            "name": "pad0",
            "type": {
              "array": [
                "u8",
                5
              ]
            }
          },
          {
            "name": "reserved0",
            "docs": [
              "The following values are irrelevant because staked collateral positions do not support",
              "borrowing."
            ],
            "type": {
              "array": [
                "u8",
                8
              ]
            }
          },
          {
            "name": "reserved1",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "reserved2",
            "type": {
              "array": [
                "u8",
                64
              ]
            }
          }
        ]
      }
    },
    {
      "name": "stakedSettingsConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "type": "pubkey"
          },
          {
            "name": "assetWeightInit",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "defined": {
                "name": "wrappedI80f48"
              }
            }
          },
          {
            "name": "depositLimit",
            "type": "u64"
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": "u64"
          },
          {
            "name": "oracleMaxAge",
            "type": "u16"
          },
          {
            "name": "riskTier",
            "docs": [
              "WARN: You almost certainly want \"Collateral\", using Isolated risk tier makes the asset",
              "worthless as collateral, and is generally useful only when creating a staked collateral pool",
              "for rewards purposes only."
            ],
            "type": {
              "defined": {
                "name": "riskTier"
              }
            }
          }
        ]
      }
    },
    {
      "name": "stakedSettingsEditConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oracle",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "assetWeightInit",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "assetWeightMaint",
            "type": {
              "option": {
                "defined": {
                  "name": "wrappedI80f48"
                }
              }
            }
          },
          {
            "name": "depositLimit",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "totalAssetValueInitLimit",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "oracleMaxAge",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "riskTier",
            "docs": [
              "WARN: You almost certainly want \"Collateral\", using Isolated risk tier makes the asset",
              "worthless as collateral, making all outstanding accounts eligible to be liquidated, and is",
              "generally useful only when creating a staked collateral pool for rewards purposes only."
            ],
            "type": {
              "option": {
                "defined": {
                  "name": "riskTier"
                }
              }
            }
          }
        ]
      }
    },
    {
      "name": "withdrawWindowCache",
      "repr": {
        "kind": "c"
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "dailyLimit",
            "type": "u32"
          },
          {
            "name": "withdrawnToday",
            "type": "u32"
          },
          {
            "name": "lastDailyResetTimestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "wrappedI80f48",
      "serialization": "bytemuck",
      "repr": {
        "kind": "c",
        "align": 8
      },
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "value",
            "type": {
              "array": [
                "u8",
                16
              ]
            }
          }
        ]
      }
    }
  ]
};
