/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/liquidity.json`.
 */
export type Liquidity = {
  address: "jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC";
  metadata: {
    name: "liquidity";
    version: "0.1.0";
    spec: "0.1.0";
    description: "Created with Anchor";
  };
  instructions: [
    {
      name: "changeStatus";
      discriminator: [236, 145, 131, 228, 227, 17, 192, 255];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "liquidity";
          writable: true;
        },
        {
          name: "authList";
        }
      ];
      args: [
        {
          name: "status";
          type: "bool";
        }
      ];
    },
    {
      name: "claim";
      discriminator: [62, 198, 214, 193, 213, 159, 108, 210];
      accounts: [
        {
          name: "user";
          signer: true;
          relations: ["claimAccount"];
        },
        {
          name: "liquidity";
        },
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "mint";
          relations: ["tokenReserve", "claimAccount"];
        },
        {
          name: "recipientTokenAccount";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "arg";
                path: "recipient";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
        },
        {
          name: "vault";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "liquidity";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
          relations: ["tokenReserve"];
        },
        {
          name: "claimAccount";
          writable: true;
        },
        {
          name: "tokenProgram";
        },
        {
          name: "associatedTokenProgram";
          address: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        }
      ];
      args: [
        {
          name: "recipient";
          type: "pubkey";
        }
      ];
    },
    {
      name: "closeClaimAccount";
      discriminator: [241, 146, 203, 216, 58, 222, 91, 118];
      accounts: [
        {
          name: "user";
          writable: true;
          signer: true;
          relations: ["claimAccount"];
        },
        {
          name: "claimAccount";
          writable: true;
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "mint";
          type: "pubkey";
        }
      ];
    },
    {
      name: "collectRevenue";
      discriminator: [87, 96, 211, 36, 240, 43, 246, 87];
      accounts: [
        {
          name: "authority";
          writable: true;
          signer: true;
        },
        {
          name: "liquidity";
        },
        {
          name: "authList";
        },
        {
          name: "mint";
          relations: ["tokenReserve"];
        },
        {
          name: "revenueCollectorAccount";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "revenueCollector";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
        },
        {
          name: "revenueCollector";
        },
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "vault";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "liquidity";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
          relations: ["tokenReserve"];
        },
        {
          name: "tokenProgram";
        },
        {
          name: "associatedTokenProgram";
          address: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [];
    },
    {
      name: "initClaimAccount";
      discriminator: [112, 141, 47, 170, 42, 99, 144, 145];
      accounts: [
        {
          name: "signer";
          writable: true;
          signer: true;
        },
        {
          name: "claimAccount";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [117, 115, 101, 114, 95, 99, 108, 97, 105, 109];
              },
              {
                kind: "arg";
                path: "user";
              },
              {
                kind: "arg";
                path: "mint";
              }
            ];
          };
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "mint";
          type: "pubkey";
        },
        {
          name: "user";
          type: "pubkey";
        }
      ];
    },
    {
      name: "initLiquidity";
      discriminator: [95, 189, 216, 183, 188, 62, 244, 108];
      accounts: [
        {
          name: "signer";
          writable: true;
          signer: true;
        },
        {
          name: "liquidity";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [108, 105, 113, 117, 105, 100, 105, 116, 121];
              }
            ];
          };
        },
        {
          name: "authList";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [97, 117, 116, 104, 95, 108, 105, 115, 116];
              }
            ];
          };
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "authority";
          type: "pubkey";
        },
        {
          name: "revenueCollector";
          type: "pubkey";
        }
      ];
    },
    {
      name: "initNewProtocol";
      discriminator: [193, 147, 5, 32, 138, 135, 213, 158];
      accounts: [
        {
          name: "authority";
          writable: true;
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "userSupplyPosition";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  117,
                  115,
                  101,
                  114,
                  95,
                  115,
                  117,
                  112,
                  112,
                  108,
                  121,
                  95,
                  112,
                  111,
                  115,
                  105,
                  116,
                  105,
                  111,
                  110
                ];
              },
              {
                kind: "arg";
                path: "supplyMint";
              },
              {
                kind: "arg";
                path: "protocol";
              }
            ];
          };
        },
        {
          name: "userBorrowPosition";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  117,
                  115,
                  101,
                  114,
                  95,
                  98,
                  111,
                  114,
                  114,
                  111,
                  119,
                  95,
                  112,
                  111,
                  115,
                  105,
                  116,
                  105,
                  111,
                  110
                ];
              },
              {
                kind: "arg";
                path: "borrowMint";
              },
              {
                kind: "arg";
                path: "protocol";
              }
            ];
          };
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "supplyMint";
          type: "pubkey";
        },
        {
          name: "borrowMint";
          type: "pubkey";
        },
        {
          name: "protocol";
          type: "pubkey";
        }
      ];
    },
    {
      name: "initTokenReserve";
      discriminator: [228, 235, 65, 129, 159, 15, 6, 84];
      accounts: [
        {
          name: "authority";
          writable: true;
          signer: true;
        },
        {
          name: "liquidity";
        },
        {
          name: "authList";
        },
        {
          name: "mint";
        },
        {
          name: "vault";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "liquidity";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
        },
        {
          name: "rateModel";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [114, 97, 116, 101, 95, 109, 111, 100, 101, 108];
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
          };
        },
        {
          name: "tokenReserve";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [114, 101, 115, 101, 114, 118, 101];
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
          };
        },
        {
          name: "tokenProgram";
        },
        {
          name: "associatedTokenProgram";
          address: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        },
        {
          name: "systemProgram";
          address: "11111111111111111111111111111111";
        }
      ];
      args: [];
    },
    {
      name: "operate";
      discriminator: [217, 106, 208, 99, 116, 151, 42, 135];
      accounts: [
        {
          name: "protocol";
          signer: true;
          relations: ["userSupplyPosition", "userBorrowPosition"];
        },
        {
          name: "liquidity";
        },
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "mint";
          relations: [
            "tokenReserve",
            "rateModel",
            "borrowClaimAccount",
            "withdrawClaimAccount"
          ];
        },
        {
          name: "vault";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "account";
                path: "liquidity";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
          relations: ["tokenReserve"];
        },
        {
          name: "userSupplyPosition";
          writable: true;
          optional: true;
        },
        {
          name: "userBorrowPosition";
          writable: true;
          optional: true;
        },
        {
          name: "rateModel";
        },
        {
          name: "withdrawToAccount";
          writable: true;
          optional: true;
          pda: {
            seeds: [
              {
                kind: "arg";
                path: "withdrawTo";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
        },
        {
          name: "borrowToAccount";
          writable: true;
          optional: true;
          pda: {
            seeds: [
              {
                kind: "arg";
                path: "borrowTo";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "account";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
        },
        {
          name: "borrowClaimAccount";
          writable: true;
          optional: true;
        },
        {
          name: "withdrawClaimAccount";
          writable: true;
          optional: true;
        },
        {
          name: "tokenProgram";
        },
        {
          name: "associatedTokenProgram";
          address: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        }
      ];
      args: [
        {
          name: "supplyAmount";
          type: "i128";
        },
        {
          name: "borrowAmount";
          type: "i128";
        },
        {
          name: "withdrawTo";
          type: "pubkey";
        },
        {
          name: "borrowTo";
          type: "pubkey";
        },
        {
          name: "transferType";
          type: {
            defined: {
              name: "transferType";
            };
          };
        }
      ];
    },
    {
      name: "pauseUser";
      discriminator: [18, 63, 43, 94, 239, 53, 101, 14];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "userSupplyPosition";
          writable: true;
        },
        {
          name: "userBorrowPosition";
          writable: true;
        }
      ];
      args: [
        {
          name: "protocol";
          type: "pubkey";
        },
        {
          name: "supplyMint";
          type: "pubkey";
        },
        {
          name: "borrowMint";
          type: "pubkey";
        },
        {
          name: "supplyStatus";
          type: {
            option: "u8";
          };
        },
        {
          name: "borrowStatus";
          type: {
            option: "u8";
          };
        }
      ];
    },
    {
      name: "preOperate";
      discriminator: [129, 205, 158, 155, 198, 155, 72, 133];
      accounts: [
        {
          name: "protocol";
          signer: true;
          relations: ["userSupplyPosition", "userBorrowPosition"];
        },
        {
          name: "liquidity";
        },
        {
          name: "userSupplyPosition";
          optional: true;
        },
        {
          name: "userBorrowPosition";
          optional: true;
        },
        {
          name: "vault";
          pda: {
            seeds: [
              {
                kind: "account";
                path: "liquidity";
              },
              {
                kind: "account";
                path: "tokenProgram";
              },
              {
                kind: "arg";
                path: "mint";
              }
            ];
            program: {
              kind: "const";
              value: [
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
              ];
            };
          };
          relations: ["tokenReserve"];
        },
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "associatedTokenProgram";
          address: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        },
        {
          name: "tokenProgram";
        }
      ];
      args: [
        {
          name: "mint";
          type: "pubkey";
        }
      ];
    },
    {
      name: "unpauseUser";
      discriminator: [71, 115, 128, 252, 182, 126, 234, 62];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "userSupplyPosition";
          writable: true;
        },
        {
          name: "userBorrowPosition";
          writable: true;
        }
      ];
      args: [
        {
          name: "protocol";
          type: "pubkey";
        },
        {
          name: "supplyMint";
          type: "pubkey";
        },
        {
          name: "borrowMint";
          type: "pubkey";
        },
        {
          name: "supplyStatus";
          type: {
            option: "u8";
          };
        },
        {
          name: "borrowStatus";
          type: {
            option: "u8";
          };
        }
      ];
    },
    {
      name: "updateAuthority";
      discriminator: [32, 46, 64, 28, 149, 75, 243, 88];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "liquidity";
          writable: true;
        },
        {
          name: "authList";
          writable: true;
        }
      ];
      args: [
        {
          name: "newAuthority";
          type: "pubkey";
        }
      ];
    },
    {
      name: "updateAuths";
      discriminator: [93, 96, 178, 156, 57, 117, 253, 209];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "liquidity";
        },
        {
          name: "authList";
          writable: true;
        }
      ];
      args: [
        {
          name: "authStatus";
          type: {
            vec: {
              defined: {
                name: "addressBool";
              };
            };
          };
        }
      ];
    },
    {
      name: "updateExchangePrice";
      discriminator: [239, 244, 10, 248, 116, 25, 53, 150];
      accounts: [
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "rateModel";
          writable: true;
        }
      ];
      args: [
        {
          name: "mint";
          type: "pubkey";
        }
      ];
    },
    {
      name: "updateGuardians";
      discriminator: [43, 62, 250, 138, 141, 117, 132, 97];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "liquidity";
        },
        {
          name: "authList";
          writable: true;
        }
      ];
      args: [
        {
          name: "guardianStatus";
          type: {
            vec: {
              defined: {
                name: "addressBool";
              };
            };
          };
        }
      ];
    },
    {
      name: "updateRateDataV1";
      discriminator: [6, 20, 34, 122, 22, 150, 180, 22];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "rateModel";
          writable: true;
        },
        {
          name: "mint";
          relations: ["rateModel", "tokenReserve"];
        },
        {
          name: "tokenReserve";
          writable: true;
        }
      ];
      args: [
        {
          name: "rateData";
          type: {
            defined: {
              name: "rateDataV1Params";
            };
          };
        }
      ];
    },
    {
      name: "updateRateDataV2";
      discriminator: [116, 73, 53, 146, 216, 45, 228, 124];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "rateModel";
          writable: true;
        },
        {
          name: "mint";
          relations: ["rateModel", "tokenReserve"];
        },
        {
          name: "tokenReserve";
          writable: true;
        }
      ];
      args: [
        {
          name: "rateData";
          type: {
            defined: {
              name: "rateDataV2Params";
            };
          };
        }
      ];
    },
    {
      name: "updateRevenueCollector";
      discriminator: [167, 142, 124, 240, 220, 113, 141, 59];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "liquidity";
          writable: true;
        }
      ];
      args: [
        {
          name: "revenueCollector";
          type: "pubkey";
        }
      ];
    },
    {
      name: "updateTokenConfig";
      discriminator: [231, 122, 181, 79, 255, 79, 144, 167];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "rateModel";
          writable: true;
        },
        {
          name: "mint";
          relations: ["rateModel", "tokenReserve"];
        },
        {
          name: "tokenReserve";
          writable: true;
        }
      ];
      args: [
        {
          name: "tokenConfig";
          type: {
            defined: {
              name: "tokenConfig";
            };
          };
        }
      ];
    },
    {
      name: "updateUserBorrowConfig";
      discriminator: [100, 176, 201, 174, 247, 2, 54, 168];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "protocol";
          relations: ["userBorrowPosition"];
        },
        {
          name: "authList";
        },
        {
          name: "rateModel";
        },
        {
          name: "mint";
          relations: ["rateModel", "tokenReserve", "userBorrowPosition"];
        },
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "userBorrowPosition";
          writable: true;
        }
      ];
      args: [
        {
          name: "userBorrowConfig";
          type: {
            defined: {
              name: "userBorrowConfig";
            };
          };
        }
      ];
    },
    {
      name: "updateUserClass";
      discriminator: [12, 206, 68, 135, 63, 212, 48, 119];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
          writable: true;
        }
      ];
      args: [
        {
          name: "userClass";
          type: {
            vec: {
              defined: {
                name: "addressU8";
              };
            };
          };
        }
      ];
    },
    {
      name: "updateUserSupplyConfig";
      discriminator: [217, 239, 225, 218, 33, 49, 234, 183];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "protocol";
          relations: ["userSupplyPosition"];
        },
        {
          name: "authList";
        },
        {
          name: "rateModel";
        },
        {
          name: "mint";
          relations: ["rateModel", "tokenReserve", "userSupplyPosition"];
        },
        {
          name: "tokenReserve";
          writable: true;
        },
        {
          name: "userSupplyPosition";
          writable: true;
        }
      ];
      args: [
        {
          name: "userSupplyConfig";
          type: {
            defined: {
              name: "userSupplyConfig";
            };
          };
        }
      ];
    },
    {
      name: "updateUserWithdrawalLimit";
      discriminator: [162, 9, 186, 9, 213, 30, 173, 78];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "authList";
        },
        {
          name: "userSupplyPosition";
          writable: true;
        }
      ];
      args: [
        {
          name: "newLimit";
          type: "u128";
        },
        {
          name: "protocol";
          type: "pubkey";
        },
        {
          name: "mint";
          type: "pubkey";
        }
      ];
    }
  ];
  accounts: [
    {
      name: "authorizationList";
      discriminator: [19, 157, 117, 43, 236, 167, 251, 69];
    },
    {
      name: "liquidity";
      discriminator: [54, 252, 249, 226, 137, 172, 121, 58];
    },
    {
      name: "rateModel";
      discriminator: [94, 3, 203, 219, 107, 137, 4, 162];
    },
    {
      name: "tokenReserve";
      discriminator: [21, 18, 59, 135, 120, 20, 31, 12];
    },
    {
      name: "userBorrowPosition";
      discriminator: [73, 126, 65, 123, 220, 126, 197, 24];
    },
    {
      name: "userClaim";
      discriminator: [228, 142, 195, 181, 228, 147, 32, 209];
    },
    {
      name: "userSupplyPosition";
      discriminator: [202, 219, 136, 118, 61, 177, 21, 146];
    }
  ];
  events: [
    {
      name: "logBorrowRateCap";
      discriminator: [156, 131, 232, 94, 254, 156, 14, 117];
    },
    {
      name: "logChangeStatus";
      discriminator: [89, 77, 37, 172, 141, 31, 74, 42];
    },
    {
      name: "logClaim";
      discriminator: [238, 50, 157, 85, 151, 58, 231, 45];
    },
    {
      name: "logCollectRevenue";
      discriminator: [64, 198, 22, 194, 123, 87, 166, 82];
    },
    {
      name: "logOperate";
      discriminator: [180, 8, 81, 71, 19, 132, 173, 8];
    },
    {
      name: "logPauseUser";
      discriminator: [100, 17, 114, 224, 180, 30, 52, 170];
    },
    {
      name: "logUnpauseUser";
      discriminator: [170, 91, 132, 96, 179, 77, 168, 26];
    },
    {
      name: "logUpdateAuthority";
      discriminator: [150, 152, 157, 143, 6, 135, 193, 101];
    },
    {
      name: "logUpdateAuths";
      discriminator: [88, 80, 109, 48, 111, 203, 76, 251];
    },
    {
      name: "logUpdateExchangePrices";
      discriminator: [190, 194, 69, 204, 30, 86, 181, 163];
    },
    {
      name: "logUpdateGuardians";
      discriminator: [231, 28, 191, 51, 53, 140, 79, 142];
    },
    {
      name: "logUpdateRateDataV1";
      discriminator: [30, 102, 131, 192, 0, 30, 85, 223];
    },
    {
      name: "logUpdateRateDataV2";
      discriminator: [206, 53, 195, 70, 113, 211, 92, 129];
    },
    {
      name: "logUpdateRevenueCollector";
      discriminator: [44, 143, 80, 250, 211, 147, 180, 159];
    },
    {
      name: "logUpdateTokenConfigs";
      discriminator: [24, 205, 191, 130, 47, 40, 233, 218];
    },
    {
      name: "logUpdateUserBorrowConfigs";
      discriminator: [210, 251, 242, 159, 205, 33, 154, 74];
    },
    {
      name: "logUpdateUserClass";
      discriminator: [185, 193, 106, 248, 11, 53, 0, 136];
    },
    {
      name: "logUpdateUserSupplyConfigs";
      discriminator: [142, 160, 21, 90, 87, 88, 18, 51];
    },
    {
      name: "logUpdateUserWithdrawalLimit";
      discriminator: [114, 131, 152, 189, 120, 253, 88, 105];
    }
  ];
  errors: [
    {
      code: 6000;
      name: "userClassNotPausable";
      msg: "adminModuleUserClassNotPausable";
    },
    {
      code: 6001;
      name: "userClassNotFound";
      msg: "adminModuleUserClassNotFound";
    },
    {
      code: 6002;
      name: "userAlreadyPaused";
      msg: "adminModuleUserAlreadyPaused";
    },
    {
      code: 6003;
      name: "userAlreadyUnpaused";
      msg: "adminModuleUserAlreadyUnpaused";
    },
    {
      code: 6004;
      name: "onlyLiquidityAuthority";
      msg: "adminModuleOnlyLiquidityAuthority";
    },
    {
      code: 6005;
      name: "onlyAuth";
      msg: "adminModuleOnlyAuth";
    },
    {
      code: 6006;
      name: "onlyGuardians";
      msg: "adminModuleOnlyGuardians";
    },
    {
      code: 6007;
      name: "invalidParams";
      msg: "adminModuleInvalidParams";
    },
    {
      code: 6008;
      name: "invalidConfigOrder";
      msg: "adminModuleInvalidConfigOrder";
    },
    {
      code: 6009;
      name: "statusAlreadySet";
      msg: "adminModuleStatusAlreadySet";
    },
    {
      code: 6010;
      name: "limitsCannotBeZero";
      msg: "adminModuleLimitsCanNotBeZero";
    },
    {
      code: 6011;
      name: "maxAuthCountReached";
      msg: "adminModuleMaxAuthCount";
    },
    {
      code: 6012;
      name: "maxUserClassesReached";
      msg: "adminModuleMaxUserClasses";
    },
    {
      code: 6013;
      name: "insufficientBalance";
      msg: "userModuleInsufficientBalance";
    },
    {
      code: 6014;
      name: "userSupplyPositionRequired";
      msg: "userModuleUserSupplyPositionRequired";
    },
    {
      code: 6015;
      name: "userBorrowPositionRequired";
      msg: "userModuleUserBorrowPositionRequired";
    },
    {
      code: 6016;
      name: "claimAccountRequired";
      msg: "userModuleClaimAccountRequired";
    },
    {
      code: 6017;
      name: "withdrawToAccountRequired";
      msg: "userModuleWithdrawToAccountRequired";
    },
    {
      code: 6018;
      name: "borrowToAccountRequired";
      msg: "userModuleBorrowToAccountRequired";
    },
    {
      code: 6019;
      name: "invalidClaimAmount";
      msg: "userModuleInvalidClaimAmount";
    },
    {
      code: 6020;
      name: "noAmountToClaim";
      msg: "userModuleNoAmountToClaim";
    },
    {
      code: 6021;
      name: "amountNotZero";
      msg: "userModuleAmountNotZero";
    },
    {
      code: 6022;
      name: "valueOverflow";
      msg: "userModuleValueOverflow";
    },
    {
      code: 6023;
      name: "invalidTransferType";
      msg: "userModuleInvalidTransferType";
    },
    {
      code: 6024;
      name: "mintMismatch";
      msg: "userModuleMintMismatch";
    },
    {
      code: 6025;
      name: "userNotDefined";
      msg: "userModuleUserNotDefined";
    },
    {
      code: 6026;
      name: "invalidUserClaim";
      msg: "userModuleInvalidUserClaim";
    },
    {
      code: 6027;
      name: "userPaused";
      msg: "userModuleUserPaused";
    },
    {
      code: 6028;
      name: "withdrawalLimitReached";
      msg: "userModuleWithdrawalLimitReached";
    },
    {
      code: 6029;
      name: "borrowLimitReached";
      msg: "userModuleBorrowLimitReached";
    },
    {
      code: 6030;
      name: "operateAmountsNearlyZero";
      msg: "userModuleOperateAmountsZero";
    },
    {
      code: 6031;
      name: "operateAmountTooBig";
      msg: "userModuleOperateAmountsTooBig";
    },
    {
      code: 6032;
      name: "operateAmountsInsufficient";
      msg: "userModuleOperateAmountsInsufficient";
    },
    {
      code: 6033;
      name: "transferAmountOutOfBounds";
      msg: "userModuleTransferAmountOutOfBounds";
    },
    {
      code: 6034;
      name: "forbiddenOperateCall";
      msg: "forbiddenOperateCall";
    },
    {
      code: 6035;
      name: "maxUtilizationReached";
      msg: "userModuleMaxUtilizationReached";
    },
    {
      code: 6036;
      name: "valueOverflowTotalSupply";
      msg: "userModuleValueOverflowTotalSupply";
    },
    {
      code: 6037;
      name: "valueOverflowTotalBorrow";
      msg: "userModuleValueOverflowTotalBorrow";
    },
    {
      code: 6038;
      name: "depositExpected";
      msg: "userModuleDepositExpected";
    },
    {
      code: 6039;
      name: "exchangePriceZero";
      msg: "liquidityCalcsExchangePriceZero";
    },
    {
      code: 6040;
      name: "unsupportedRateVersion";
      msg: "liquidityCalcsUnsupportedRateVersion";
    },
    {
      code: 6041;
      name: "borrowRateNegative";
      msg: "liquidityCalcsBorrowRateNegative";
    },
    {
      code: 6042;
      name: "protocolLockdown";
      msg: "protocolLockdown";
    }
  ];
  types: [
    {
      name: "addressBool";
      type: {
        kind: "struct";
        fields: [
          {
            name: "addr";
            type: "pubkey";
          },
          {
            name: "value";
            type: "bool";
          }
        ];
      };
    },
    {
      name: "addressU8";
      type: {
        kind: "struct";
        fields: [
          {
            name: "addr";
            type: "pubkey";
          },
          {
            name: "value";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "authorizationList";
      type: {
        kind: "struct";
        fields: [
          {
            name: "authUsers";
            type: {
              vec: "pubkey";
            };
          },
          {
            name: "guardians";
            type: {
              vec: "pubkey";
            };
          },
          {
            name: "userClasses";
            type: {
              vec: {
                defined: {
                  name: "userClass";
                };
              };
            };
          }
        ];
      };
    },
    {
      name: "liquidity";
      type: {
        kind: "struct";
        fields: [
          {
            name: "authority";
            type: "pubkey";
          },
          {
            name: "revenueCollector";
            type: "pubkey";
          },
          {
            name: "status";
            type: "bool";
          },
          {
            name: "bump";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "logBorrowRateCap";
      type: {
        kind: "struct";
        fields: [
          {
            name: "token";
            type: "pubkey";
          }
        ];
      };
    },
    {
      name: "logChangeStatus";
      type: {
        kind: "struct";
        fields: [
          {
            name: "newStatus";
            type: "bool";
          }
        ];
      };
    },
    {
      name: "logClaim";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "recipient";
            type: "pubkey";
          },
          {
            name: "amount";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "logCollectRevenue";
      type: {
        kind: "struct";
        fields: [
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "revenueAmount";
            type: "u128";
          }
        ];
      };
    },
    {
      name: "logOperate";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "supplyAmount";
            type: "i128";
          },
          {
            name: "borrowAmount";
            type: "i128";
          },
          {
            name: "withdrawTo";
            type: "pubkey";
          },
          {
            name: "borrowTo";
            type: "pubkey";
          },
          {
            name: "supplyExchangePrice";
            type: "u64";
          },
          {
            name: "borrowExchangePrice";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "logPauseUser";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "mint";
            type: "pubkey";
          },
          {
            name: "status";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "logUnpauseUser";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "mint";
            type: "pubkey";
          },
          {
            name: "status";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "logUpdateAuthority";
      type: {
        kind: "struct";
        fields: [
          {
            name: "newAuthority";
            type: "pubkey";
          }
        ];
      };
    },
    {
      name: "logUpdateAuths";
      type: {
        kind: "struct";
        fields: [
          {
            name: "authStatus";
            type: {
              vec: {
                defined: {
                  name: "addressBool";
                };
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateExchangePrices";
      type: {
        kind: "struct";
        fields: [
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "supplyExchangePrice";
            type: "u128";
          },
          {
            name: "borrowExchangePrice";
            type: "u128";
          },
          {
            name: "borrowRate";
            type: "u16";
          },
          {
            name: "utilization";
            type: "u16";
          }
        ];
      };
    },
    {
      name: "logUpdateGuardians";
      type: {
        kind: "struct";
        fields: [
          {
            name: "guardianStatus";
            type: {
              vec: {
                defined: {
                  name: "addressBool";
                };
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateRateDataV1";
      type: {
        kind: "struct";
        fields: [
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "rateData";
            type: {
              defined: {
                name: "rateDataV1Params";
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateRateDataV2";
      type: {
        kind: "struct";
        fields: [
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "rateData";
            type: {
              defined: {
                name: "rateDataV2Params";
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateRevenueCollector";
      type: {
        kind: "struct";
        fields: [
          {
            name: "revenueCollector";
            type: "pubkey";
          }
        ];
      };
    },
    {
      name: "logUpdateTokenConfigs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "tokenConfig";
            type: {
              defined: {
                name: "tokenConfig";
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateUserBorrowConfigs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "userBorrowConfig";
            type: {
              defined: {
                name: "userBorrowConfig";
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateUserClass";
      type: {
        kind: "struct";
        fields: [
          {
            name: "userClass";
            type: {
              vec: {
                defined: {
                  name: "addressU8";
                };
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateUserSupplyConfigs";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "userSupplyConfig";
            type: {
              defined: {
                name: "userSupplyConfig";
              };
            };
          }
        ];
      };
    },
    {
      name: "logUpdateUserWithdrawalLimit";
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "token";
            type: "pubkey";
          },
          {
            name: "newLimit";
            type: "u128";
          }
        ];
      };
    },
    {
      name: "rateDataV1Params";
      docs: ["@notice struct to set borrow rate data for version 1"];
      type: {
        kind: "struct";
        fields: [
          {
            name: "kink";
            docs: [
              "",
              "@param kink in borrow rate. in 1e2: 100% = 10_000; 1% = 100"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationZero";
            docs: [
              "",
              "@param rateAtUtilizationZero desired borrow rate when utilization is zero. in 1e2: 100% = 10_000; 1% = 100",
              "i.e. constant minimum borrow rate",
              "e.g. at utilization = 0.01% rate could still be at least 4% (rateAtUtilizationZero would be 400 then)"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationKink";
            docs: [
              "",
              "@param rateAtUtilizationKink borrow rate when utilization is at kink. in 1e2: 100% = 10_000; 1% = 100",
              "e.g. when rate should be 7% at kink then rateAtUtilizationKink would be 700"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationMax";
            docs: [
              "",
              "@param rateAtUtilizationMax borrow rate when utilization is maximum at 100%. in 1e2: 100% = 10_000; 1% = 100",
              "e.g. when rate should be 125% at 100% then rateAtUtilizationMax would be 12_500"
            ];
            type: "u128";
          }
        ];
      };
    },
    {
      name: "rateDataV2Params";
      docs: ["@notice struct to set borrow rate data for version 2"];
      type: {
        kind: "struct";
        fields: [
          {
            name: "kink1";
            docs: [
              "",
              "@param kink1 first kink in borrow rate. in 1e2: 100% = 10_000; 1% = 100",
              "utilization below kink 1 usually means slow increase in rate, once utilization is above kink 1 borrow rate increases faster"
            ];
            type: "u128";
          },
          {
            name: "kink2";
            docs: [
              "",
              "@param kink2 second kink in borrow rate. in 1e2: 100% = 10_000; 1% = 100",
              "utilization below kink 2 usually means slow / medium increase in rate, once utilization is above kink 2 borrow rate increases fast"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationZero";
            docs: [
              "",
              "@param rateAtUtilizationZero desired borrow rate when utilization is zero. in 1e2: 100% = 10_000; 1% = 100",
              "i.e. constant minimum borrow rate",
              "e.g. at utilization = 0.01% rate could still be at least 4% (rateAtUtilizationZero would be 400 then)"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationKink1";
            docs: [
              "",
              "@param rateAtUtilizationKink1 desired borrow rate when utilization is at first kink. in 1e2: 100% = 10_000; 1% = 100",
              "e.g. when rate should be 7% at first kink then rateAtUtilizationKink would be 700"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationKink2";
            docs: [
              "",
              "@param rateAtUtilizationKink2 desired borrow rate when utilization is at second kink. in 1e2: 100% = 10_000; 1% = 100",
              "e.g. when rate should be 7% at second kink then rateAtUtilizationKink would be 1_200"
            ];
            type: "u128";
          },
          {
            name: "rateAtUtilizationMax";
            docs: [
              "",
              "@param rateAtUtilizationMax desired borrow rate when utilization is maximum at 100%. in 1e2: 100% = 10_000; 1% = 100",
              "e.g. when rate should be 125% at 100% then rateAtUtilizationMax would be 12_500"
            ];
            type: "u128";
          }
        ];
      };
    },
    {
      name: "rateModel";
      docs: ["Interest rate model data"];
      serialization: "bytemuck";
      repr: {
        kind: "c";
        packed: true;
      };
      type: {
        kind: "struct";
        fields: [
          {
            name: "mint";
            type: "pubkey";
          },
          {
            name: "version";
            type: "u8";
          },
          {
            name: "rateAtZero";
            type: "u16";
          },
          {
            name: "kink1Utilization";
            type: "u16";
          },
          {
            name: "rateAtKink1";
            type: "u16";
          },
          {
            name: "rateAtMax";
            type: "u16";
          },
          {
            name: "kink2Utilization";
            type: "u16";
          },
          {
            name: "rateAtKink2";
            type: "u16";
          }
        ];
      };
    },
    {
      name: "tokenConfig";
      docs: ["@notice struct to set token config"];
      type: {
        kind: "struct";
        fields: [
          {
            name: "token";
            docs: ["", "@param token address"];
            type: "pubkey";
          },
          {
            name: "fee";
            docs: [
              "",
              "@param fee charges on borrower's interest. in 1e2: 100% = 10_000; 1% = 100"
            ];
            type: "u128";
          },
          {
            name: "maxUtilization";
            docs: [
              "",
              "@param maxUtilization maximum allowed utilization. in 1e2: 100% = 10_000; 1% = 100",
              "set to 100% to disable and have default limit of 100% (avoiding SLOAD)."
            ];
            type: "u128";
          }
        ];
      };
    },
    {
      name: "tokenReserve";
      docs: ["Token configuration and exchange prices"];
      serialization: "bytemuck";
      repr: {
        kind: "c";
        packed: true;
      };
      type: {
        kind: "struct";
        fields: [
          {
            name: "mint";
            type: "pubkey";
          },
          {
            name: "vault";
            type: "pubkey";
          },
          {
            name: "borrowRate";
            type: "u16";
          },
          {
            name: "feeOnInterest";
            type: "u16";
          },
          {
            name: "lastUtilization";
            type: "u16";
          },
          {
            name: "lastUpdateTimestamp";
            type: "u64";
          },
          {
            name: "supplyExchangePrice";
            type: "u64";
          },
          {
            name: "borrowExchangePrice";
            type: "u64";
          },
          {
            name: "maxUtilization";
            type: "u16";
          },
          {
            name: "totalSupplyWithInterest";
            type: "u64";
          },
          {
            name: "totalSupplyInterestFree";
            type: "u64";
          },
          {
            name: "totalBorrowWithInterest";
            type: "u64";
          },
          {
            name: "totalBorrowInterestFree";
            type: "u64";
          },
          {
            name: "totalClaimAmount";
            type: "u64";
          },
          {
            name: "interactingProtocol";
            type: "pubkey";
          },
          {
            name: "interactingTimestamp";
            type: "u64";
          },
          {
            name: "interactingBalance";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "transferType";
      type: {
        kind: "enum";
        variants: [
          {
            name: "skip";
          },
          {
            name: "direct";
          },
          {
            name: "claim";
          }
        ];
      };
    },
    {
      name: "userBorrowConfig";
      docs: ["@notice struct to set user borrow & payback config"];
      type: {
        kind: "struct";
        fields: [
          {
            name: "mode";
            docs: ["", "@param mode: 0 = without interest. 1 = with interest"];
            type: "u8";
          },
          {
            name: "expandPercent";
            docs: [
              "",
              "@param expandPercent debt limit expand percent. in 1e2: 100% = 10_000; 1% = 100",
              "Also used to calculate rate at which debt limit should decrease (instant)."
            ];
            type: "u128";
          },
          {
            name: "expandDuration";
            docs: [
              "",
              "@param expandDuration debt limit expand duration in seconds.",
              "used to calculate rate together with expandPercent"
            ];
            type: "u128";
          },
          {
            name: "baseDebtCeiling";
            docs: [
              "",
              "@param baseDebtCeiling base borrow limit. until here, borrow limit remains as baseDebtCeiling",
              "(user can borrow until this point at once without stepped expansion). Above this, automated limit comes in place.",
              "amount in raw (to be multiplied with exchange price) or normal depends on configured mode in user config for the token:",
              "with interest -> raw, without interest -> normal"
            ];
            type: "u128";
          },
          {
            name: "maxDebtCeiling";
            docs: [
              "",
              "@param maxDebtCeiling max borrow ceiling, maximum amount the user can borrow.",
              "amount in raw (to be multiplied with exchange price) or normal depends on configured mode in user config for the token:",
              "with interest -> raw, without interest -> normal"
            ];
            type: "u128";
          }
        ];
      };
    },
    {
      name: "userBorrowPosition";
      docs: ["User borrow position"];
      serialization: "bytemuck";
      repr: {
        kind: "c";
        packed: true;
      };
      type: {
        kind: "struct";
        fields: [
          {
            name: "protocol";
            type: "pubkey";
          },
          {
            name: "mint";
            type: "pubkey";
          },
          {
            name: "withInterest";
            type: "u8";
          },
          {
            name: "amount";
            type: "u64";
          },
          {
            name: "debtCeiling";
            type: "u64";
          },
          {
            name: "lastUpdate";
            type: "u64";
          },
          {
            name: "expandPct";
            type: "u16";
          },
          {
            name: "expandDuration";
            type: "u32";
          },
          {
            name: "baseDebtCeiling";
            type: "u64";
          },
          {
            name: "maxDebtCeiling";
            type: "u64";
          },
          {
            name: "status";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "userClaim";
      serialization: "bytemuck";
      repr: {
        kind: "c";
        packed: true;
      };
      type: {
        kind: "struct";
        fields: [
          {
            name: "user";
            type: "pubkey";
          },
          {
            name: "amount";
            type: "u64";
          },
          {
            name: "mint";
            type: "pubkey";
          }
        ];
      };
    },
    {
      name: "userClass";
      type: {
        kind: "struct";
        fields: [
          {
            name: "addr";
            type: "pubkey";
          },
          {
            name: "class";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "userSupplyConfig";
      docs: ["@notice struct to set user supply & withdrawal config"];
      type: {
        kind: "struct";
        fields: [
          {
            name: "mode";
            docs: ["", "@param mode: 0 = without interest. 1 = with interest"];
            type: "u8";
          },
          {
            name: "expandPercent";
            docs: [
              "",
              "@param expandPercent withdrawal limit expand percent. in 1e2: 100% = 10_000; 1% = 100",
              "Also used to calculate rate at which withdrawal limit should decrease (instant)."
            ];
            type: "u128";
          },
          {
            name: "expandDuration";
            docs: [
              "",
              "@param expandDuration withdrawal limit expand duration in seconds.",
              "used to calculate rate together with expandPercent"
            ];
            type: "u128";
          },
          {
            name: "baseWithdrawalLimit";
            docs: [
              "",
              "@param baseWithdrawalLimit base limit, below this, user can withdraw the entire amount.",
              "amount in raw (to be multiplied with exchange price) or normal depends on configured mode in user config for the token:",
              "with interest -> raw, without interest -> normal"
            ];
            type: "u128";
          }
        ];
      };
    },
    {
      name: "userSupplyPosition";
      docs: ["User supply position"];
      serialization: "bytemuck";
      repr: {
        kind: "c";
        packed: true;
      };
      type: {
        kind: "struct";
        fields: [
          {
            name: "protocol";
            type: "pubkey";
          },
          {
            name: "mint";
            type: "pubkey";
          },
          {
            name: "withInterest";
            type: "u8";
          },
          {
            name: "amount";
            type: "u64";
          },
          {
            name: "withdrawalLimit";
            type: "u128";
          },
          {
            name: "lastUpdate";
            type: "u64";
          },
          {
            name: "expandPct";
            type: "u16";
          },
          {
            name: "expandDuration";
            type: "u64";
          },
          {
            name: "baseWithdrawalLimit";
            type: "u64";
          },
          {
            name: "status";
            type: "u8";
          }
        ];
      };
    }
  ];
};
