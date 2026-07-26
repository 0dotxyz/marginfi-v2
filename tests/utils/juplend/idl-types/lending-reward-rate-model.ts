/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/lending_reward_rate_model.json`.
 */
export type LendingRewardRateModel = {
  address: "jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar";
  metadata: {
    name: "lendingRewardRateModel";
    version: "0.1.0";
    spec: "0.1.0";
    description: "Created with Anchor";
  };
  instructions: [
    {
      name: "cancelQueuedRewards";
      discriminator: [253, 198, 122, 96, 234, 226, 53, 229];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "lendingRewardsAdmin";
        },
        {
          name: "lendingAccount";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "fTokenMint";
        },
        {
          name: "supplyTokenReservesLiquidity";
        },
        {
          name: "lendingRewardsRateModel";
          writable: true;
        },
        {
          name: "lendingProgram";
        }
      ];
      args: [];
    },
    {
      name: "initLendingRewardsAdmin";
      discriminator: [202, 36, 47, 209, 3, 201, 173, 94];
      accounts: [
        {
          name: "signer";
          writable: true;
          signer: true;
        },
        {
          name: "lendingRewardsAdmin";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  108,
                  101,
                  110,
                  100,
                  105,
                  110,
                  103,
                  95,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115,
                  95,
                  97,
                  100,
                  109,
                  105,
                  110
                ];
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
          name: "lendingProgram";
          type: "pubkey";
        }
      ];
    },
    {
      name: "initLendingRewardsRateModel";
      discriminator: [117, 123, 196, 52, 246, 90, 168, 0];
      accounts: [
        {
          name: "authority";
          writable: true;
          signer: true;
        },
        {
          name: "lendingRewardsAdmin";
        },
        {
          name: "mint";
        },
        {
          name: "lendingRewardsRateModel";
          writable: true;
          pda: {
            seeds: [
              {
                kind: "const";
                value: [
                  108,
                  101,
                  110,
                  100,
                  105,
                  110,
                  103,
                  95,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115,
                  95,
                  114,
                  97,
                  116,
                  101,
                  95,
                  109,
                  111,
                  100,
                  101,
                  108
                ];
              },
              {
                kind: "account";
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
      args: [];
    },
    {
      name: "queueNextRewards";
      discriminator: [12, 38, 248, 80, 128, 76, 155, 210];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "lendingRewardsAdmin";
        },
        {
          name: "lendingAccount";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "fTokenMint";
        },
        {
          name: "supplyTokenReservesLiquidity";
        },
        {
          name: "lendingRewardsRateModel";
          writable: true;
        },
        {
          name: "lendingProgram";
        }
      ];
      args: [
        {
          name: "rewardAmount";
          type: "u64";
        },
        {
          name: "duration";
          type: "u64";
        }
      ];
    },
    {
      name: "startRewards";
      discriminator: [62, 183, 108, 14, 161, 145, 121, 115];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "lendingRewardsAdmin";
        },
        {
          name: "lendingAccount";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "fTokenMint";
        },
        {
          name: "supplyTokenReservesLiquidity";
        },
        {
          name: "lendingRewardsRateModel";
          writable: true;
        },
        {
          name: "lendingProgram";
        }
      ];
      args: [
        {
          name: "rewardAmount";
          type: "u64";
        },
        {
          name: "duration";
          type: "u64";
        },
        {
          name: "startTime";
          type: "u64";
        },
        {
          name: "startTvl";
          type: "u64";
        }
      ];
    },
    {
      name: "stopRewards";
      discriminator: [39, 231, 201, 99, 230, 105, 100, 76];
      accounts: [
        {
          name: "authority";
          signer: true;
        },
        {
          name: "lendingRewardsAdmin";
        },
        {
          name: "lendingAccount";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "fTokenMint";
        },
        {
          name: "supplyTokenReservesLiquidity";
        },
        {
          name: "lendingRewardsRateModel";
          writable: true;
        },
        {
          name: "lendingProgram";
        }
      ];
      args: [];
    },
    {
      name: "transitionToNextRewards";
      discriminator: [167, 50, 233, 93, 0, 178, 154, 247];
      accounts: [
        {
          name: "lendingRewardsAdmin";
        },
        {
          name: "lendingAccount";
          writable: true;
        },
        {
          name: "mint";
          relations: ["lendingRewardsRateModel"];
        },
        {
          name: "fTokenMint";
        },
        {
          name: "supplyTokenReservesLiquidity";
        },
        {
          name: "lendingRewardsRateModel";
          writable: true;
        },
        {
          name: "lendingProgram";
        }
      ];
      args: [];
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
          name: "lendingRewardsAdmin";
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
          name: "lendingRewardsAdmin";
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
    }
  ];
  accounts: [
    {
      name: "lendingRewardsAdmin";
      discriminator: [68, 18, 109, 18, 2, 9, 174, 101];
    },
    {
      name: "lendingRewardsRateModel";
      discriminator: [166, 72, 71, 131, 172, 74, 166, 181];
    }
  ];
  events: [
    {
      name: "logCancelQueuedRewards";
      discriminator: [177, 173, 63, 139, 228, 173, 187, 204];
    },
    {
      name: "logQueueNextRewards";
      discriminator: [50, 129, 214, 126, 39, 205, 209, 116];
    },
    {
      name: "logStartRewards";
      discriminator: [30, 243, 168, 45, 233, 150, 101, 238];
    },
    {
      name: "logStopRewards";
      discriminator: [37, 218, 239, 232, 21, 149, 99, 31];
    },
    {
      name: "logTransitionedToNextRewards";
      discriminator: [177, 232, 239, 222, 224, 61, 9, 101];
    },
    {
      name: "logUpdateAuthority";
      discriminator: [150, 152, 157, 143, 6, 135, 193, 101];
    },
    {
      name: "logUpdateAuths";
      discriminator: [88, 80, 109, 48, 111, 203, 76, 251];
    }
  ];
  errors: [
    {
      code: 6000;
      name: "invalidParams";
      msg: "lendingRewardRateModelInvalidParams";
    },
    {
      code: 6001;
      name: "alreadyStopped";
      msg: "lendingRewardRateModelAlreadyStopped";
    },
    {
      code: 6002;
      name: "nextRewardsQueued";
      msg: "lendingRewardRateModelNextRewardsQueued";
    },
    {
      code: 6003;
      name: "notEnded";
      msg: "lendingRewardRateModelNotEnded";
    },
    {
      code: 6004;
      name: "noQueuedRewards";
      msg: "lendingRewardRateModelNoQueuedRewards";
    },
    {
      code: 6005;
      name: "mustTransitionToNext";
      msg: "lendingRewardRateModelMustTransitionToNext";
    },
    {
      code: 6006;
      name: "noRewardsStarted";
      msg: "lendingRewardRateModelNoRewardsStarted";
    },
    {
      code: 6007;
      name: "maxAuthCountReached";
      msg: "lendingRewardRateModelMaxAuthCountReached";
    },
    {
      code: 6008;
      name: "onlyAuthority";
      msg: "lendingRewardRateModelOnlyAuthority";
    },
    {
      code: 6009;
      name: "onlyAuths";
      msg: "lendingRewardRateModelOnlyAuth";
    },
    {
      code: 6010;
      name: "cpiToLendingProgramFailed";
      msg: "lendingRewardRateModelCpiToLendingProgramFailed";
    },
    {
      code: 6011;
      name: "invalidLendingProgram";
      msg: "lendingRewardRateModelInvalidLendingProgram";
    },
    {
      code: 6012;
      name: "invalidMint";
      msg: "lendingRewardRateModelInvalidMint";
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
      name: "lendingRewardsAdmin";
      type: {
        kind: "struct";
        fields: [
          {
            name: "authority";
            type: "pubkey";
          },
          {
            name: "lendingProgram";
            type: "pubkey";
          },
          {
            name: "auths";
            type: {
              vec: "pubkey";
            };
          },
          {
            name: "bump";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "lendingRewardsRateModel";
      type: {
        kind: "struct";
        fields: [
          {
            name: "mint";
            docs: ["@dev mint address"];
            type: "pubkey";
          },
          {
            name: "startTvl";
            docs: [
              "@dev tvl below which rewards rate is 0. If current TVL is below this value, triggering `update_rate()` on the fToken",
              "might bring the total TVL above this cut-off."
            ];
            type: "u64";
          },
          {
            name: "duration";
            docs: ["@dev for how long current rewards should run"];
            type: "u64";
          },
          {
            name: "startTime";
            docs: ["@dev when current rewards got started"];
            type: "u64";
          },
          {
            name: "yearlyReward";
            docs: [
              "@dev current annualized reward based on input params (duration, rewardAmount)"
            ];
            type: "u64";
          },
          {
            name: "nextDuration";
            docs: ["@dev Duration for the next rewards phase"];
            type: "u64";
          },
          {
            name: "nextRewardAmount";
            docs: ["@dev Amount of rewards for the next phase"];
            type: "u64";
          },
          {
            name: "bump";
            type: "u8";
          }
        ];
      };
    },
    {
      name: "logCancelQueuedRewards";
      type: {
        kind: "struct";
        fields: [
          {
            name: "mint";
            type: "pubkey";
          }
        ];
      };
    },
    {
      name: "logQueueNextRewards";
      type: {
        kind: "struct";
        fields: [
          {
            name: "rewardAmount";
            type: "u64";
          },
          {
            name: "duration";
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
      name: "logStartRewards";
      type: {
        kind: "struct";
        fields: [
          {
            name: "rewardAmount";
            type: "u64";
          },
          {
            name: "duration";
            type: "u64";
          },
          {
            name: "startTime";
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
      name: "logStopRewards";
      type: {
        kind: "struct";
        fields: [
          {
            name: "mint";
            type: "pubkey";
          }
        ];
      };
    },
    {
      name: "logTransitionedToNextRewards";
      type: {
        kind: "struct";
        fields: [
          {
            name: "startTime";
            type: "u64";
          },
          {
            name: "endTime";
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
    }
  ];
};
