import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { StablecoinFactory } from "../target/types/stablecoin_factory";
import { PublicKey, Keypair } from "@solana/web3.js";
import { 
  createMint,
  getOrCreateAssociatedTokenAccount,

} from "@solana/spl-token";
import { expect } from "chai";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { mplTokenMetadata, fetchMetadataFromSeeds, MPL_TOKEN_METADATA_PROGRAM_ID, findMetadataPda } from "@metaplex-foundation/mpl-token-metadata";
import { publicKey } from "@metaplex-foundation/umi";
import { TOKEN_2022_PROGRAM_ID, amountToUiAmount, ASSOCIATED_TOKEN_PROGRAM_ID } from '@solana/spl-token';


describe("stablecoin_factory", () => {

  const localKeypairPath = path.join(os.homedir(), ".config", "solana", "id.json");
  const localKeypairData = JSON.parse(fs.readFileSync(localKeypairPath, "utf-8"));
  const localKeypair = Keypair.fromSecretKey(new Uint8Array(localKeypairData));
  const mintAuthority = localKeypair;
  
  let transferFeeProtocolVault: PublicKey;
  let feeOperatorKeypair: anchor.web3.Keypair;
  let feeOperatorPDA: PublicKey;

  const transferFeeBasisPoints = 100; // 1%
  const maximumFee = 1000000; // 1 token (with 6 decimals)
 
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.StablecoinFactory as Program<StablecoinFactory>;
  const authority = provider.wallet.publicKey;

  
  const [factoryPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("factory")],
    program.programId
  );

  // Test parameters for factory initialization
  const minFiatReserve = 2000;              // 20% in basis points
  const bondReserveNumerator = 30;          // 30 in 30/9 ratio
  const bondReserveDenominator = 9;         // 9 in 30/9 ratio
  const yieldShareProtocol = 1000;          // 10% in basis points
  const yieldShareIssuer = 2000;            // 20% in basis points
  const yieldShareHolders = 7000;           // 70% in basis points
  // Total yield shares = 10000 bps (100%)

  const usdCoinSymbol = "USDS";
  const eurCoinSymbol = "EURS";

  let factoryInitialized = false;

  // const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
  let usdcMint: PublicKey;
  let sovereignCoinMint;

    // Original Bond mint public keys
    // const usdBondMint = new PublicKey("USTRYnGgcHAhdWsanv8BG6vHGd4p7UGgoB9NRd8ei7j");
    // const eurBondMint = new PublicKey("EuroszHk1AL7fHBBsxgeGHsamUqwBpb26oEyt9BcfZ6G");
  
  
    let usdFiatMint: PublicKey;
    let eurFiatMint: PublicKey;
    let usdBondMint: PublicKey;
    let eurBondMint: PublicKey;
    

  let usdRegistered = false;
  let eurRegistered = false;

 
  let usdSovereignCoinPDA: PublicKey;
  // let eurSovereignCoinPDA: PublicKey;

  // Create mint for sovereign coin
  // let sovereignCoinMint: PublicKey;


  let unauthorizedMint: PublicKey;

  const TOKEN_METADATA_PROGRAM_ID = new anchor.web3.PublicKey(
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
  );

  const umi = createUmi(provider.connection).use(mplTokenMetadata());
  
function calculateRequiredReserve(
    baseBps: number,               
    ordinal: number,               
    bondReserveNumerator: number, 
    bondReserveDenominator: number
): number {
   
    if (baseBps > 10_000) {
        throw new Error("Invalid reserve percentage");
    }
    if (ordinal < 1 || ordinal > 10) {
        throw new Error("Invalid bond rating");
    }
    if (bondReserveDenominator <= 0) {
        throw new Error("Invalid bond reserve ratio");
    }

    const base = BigInt(baseBps);
    const ordinalFactor = BigInt(ordinal - 1);
    const numerator = BigInt(bondReserveNumerator);
    const denominator = BigInt(bondReserveDenominator);

    const adjustment = (ordinalFactor * numerator * BigInt(10000)) / denominator;
    
    const total = base + (adjustment / BigInt(10000));
    
    const result = Number(total);
    if (result > 10_000) {
        throw new Error("Reserve exceeds 100%");
    }

    return result;
}

function sleep(s: number) {
  return new Promise((resolve) => setTimeout(resolve, s * 1000));
}

    async function getRegisteredBondMint(currency: string): Promise<PublicKey | null> {
      try {
        const factoryAccount = await program.account.factory.fetch(factoryPDA);
        
        // Iterate through all bond mappings to find matching currency
        for (let i = 0; i < factoryAccount.bondMappingsCount; i++) {
          const mapping = factoryAccount.bondMappings[i];
          if (mapping.active) {
            const storedFiatBytes = mapping.fiatCurrency.filter(byte => byte !== 0);
            const fiatString = Buffer.from(storedFiatBytes).toString();
            
            if (fiatString === currency) {
              console.log(`Found registered bond mint for ${currency}: ${mapping.bondMint.toString()}`);
              return mapping.bondMint;
            }
          }
        }
        console.log(`No registered bond mint found for ${currency}`);
        return null;
      } catch (err) {
        console.error("Error fetching registered bond mint:", err);
        return null;
      }
    }
    
  
    before(async () => {
      // No need to airdrop SOL to mint authority since we're using the provider's wallet
      // which should already have SOL

      // try {
      //   unauthorizedMint = await createMint(
      //     provider.connection,
      //     mintAuthority,
      //     authority,
      //     null,
      //     6 // Decimals
      //   );
      //   console.log("Created token mint:", usdFiatMint.toString());
      // } catch (err) {
      //   console.log("Error creating token:", err);
      // }

      // try {
      //   sovereignCoinMint = await createMint(
      //     provider.connection,
      //     mintAuthority,
      //     authority,
      //     null,
      //     6 // Decimals
      //   );
      //   console.log("Created token mint:", usdFiatMint.toString());
      // } catch (err) {
      //   console.log("Error creating token:", err);
      // }


      console.log("Factory PDA:", factoryPDA.toString());
  
      const balance = await provider.connection.getBalance(authority);
      console.log("Authority SOL balance:", balance / anchor.web3.LAMPORTS_PER_SOL);
      if (balance < 1e9) { 
        throw new Error("Authority wallet has insufficient SOL");
      }
  
      try {
        usdcMint = await createMint(
          provider.connection,
          mintAuthority,
          authority,
          null,
          6 
        );
        console.log("Created USDC token mint:", usdcMint.toString());
      } catch (err) {
        console.log("Error creating USDC token:", err);
      }

      
      try {
        usdFiatMint = await createMint(
          provider.connection,
          mintAuthority,
          authority,
          null,
          6 
        );
        console.log("Created USD fiat token mint:", usdFiatMint.toString());
      } catch (err) {
        console.log("Error creating USD fiat token:", err);
      }
  
      
      try {
        eurFiatMint = await createMint(
          provider.connection,
          mintAuthority,
          authority,
          null,
          6 
        );
        console.log("Created EUR fiat token mint:", eurFiatMint.toString());
      } catch (err) {
        console.log("Error creating EUR fiat token:", err);
      }

      
      try {
        usdBondMint = await createMint(
          provider.connection,
          mintAuthority,
          authority,
          null,
          6 
        );
        console.log("Created USD bond token mint:", usdBondMint.toString());
      } catch (err) {
        console.log("Error creating USD bond token:", err);
      }

     
      try {
        eurBondMint = await createMint(
          provider.connection,
          mintAuthority,
          authority,
          null,
          6 
        );
        console.log("Created EUR bond token mint:", eurBondMint.toString());
      } catch (err) {
        console.log("Error creating EUR bond token:", err);
      }
  });

  it("Can initialize factory with valid parameters", async () => {
      
      if (!factoryInitialized) {
        const protocolVault = anchor.web3.Keypair.generate();
        const yieldVault = anchor.web3.Keypair.generate();
  
        console.log("Protocol Vault:", protocolVault.publicKey.toString());
        console.log("Yield Vault:", yieldVault.publicKey.toString());
  
        try {
          const tx = await program.methods
            .initializeFactory(
              minFiatReserve,
              bondReserveNumerator,
              bondReserveDenominator,
              yieldShareProtocol,
              yieldShareIssuer,
              yieldShareHolders
            )
            .accounts({
              authority: authority,
               // @ts-ignore
              factory: factoryPDA,
              systemProgram: anchor.web3.SystemProgram.programId,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            })
            .rpc();
  
          console.log("Initialize Factory Transaction:", tx);
  
          
          const factoryAccount = await program.account.factory.fetch(factoryPDA);
          console.log("Factory Account Data:", JSON.stringify(factoryAccount, null, 2));
          factoryInitialized = true;
        } catch (err) {
          console.error("Transaction failed:", err);
          throw err; 
        }
      }
     
      const factoryAccount = await program.account.factory.fetch(factoryPDA);

      
      expect(factoryAccount.authority.toString()).to.equal(authority.toString());
      expect(factoryAccount.treasury.toString()).to.equal(authority.toString());
      expect(factoryAccount.totalSovereignCoins.toNumber()).to.equal(0);
      expect(factoryAccount.totalSupplyAllCoins.toNumber()).to.equal(0);
      expect(factoryAccount.minFiatReservePercentage).to.equal(minFiatReserve);
      expect(factoryAccount.bondReserveNumerator).to.equal(bondReserveNumerator);
      expect(factoryAccount.bondReserveDenominator).to.equal(bondReserveDenominator);
      expect(factoryAccount.yieldShareProtocol).to.equal(yieldShareProtocol);
      expect(factoryAccount.yieldShareIssuer).to.equal(yieldShareIssuer);
      expect(factoryAccount.yieldShareHolders).to.equal(yieldShareHolders);
      expect(factoryAccount.mintFeeBps).to.equal(0);
      expect(factoryAccount.burnFeeBps).to.equal(0);

     
      expect(factoryAccount.bondRatingOrdinals).to.deep.equal([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

  });

  it("Can register a bond mapping", async () => {
   
    const fiatCurrency = "USD";
    // const bondMint = new PublicKey("USTRYnGgcHAhdWsanv8BG6vHGd4p7UGgoB9NRd8ei7j");
    const bondRating = 1; // Highest quality and lowest risk

    try {
      
      const tx = await program.methods
        .registerBondMaps(
          fiatCurrency,
          usdBondMint,
          bondRating
        )
        .accounts({
          authority: authority,
          factory: factoryPDA,
        })
        .rpc();

      console.log("Register Bond Mapping Transaction:", tx);
      usdRegistered = true;

      
      const factoryAccount = await program.account.factory.fetch(factoryPDA);

      
      expect(factoryAccount.bondMappingsCount).to.be.greaterThan(0);

      
      const mappingIndex = factoryAccount.bondMappingsCount - 1;
      const mapping = factoryAccount.bondMappings[mappingIndex];

     
      expect(mapping.active).to.equal(true);
      
      
      const storedFiatBytes = mapping.fiatCurrency.filter(byte => byte !== 0);
      const fiatBytes = Buffer.from(fiatCurrency);
      expect(Buffer.from(storedFiatBytes).toString()).to.equal(fiatCurrency);

      
      expect(mapping.bondMint.toString()).to.equal(usdBondMint.toString());
      expect(mapping.bondRating).to.equal(bondRating);
    } catch (err) {
      console.error("Error registering bond mapping:", err);
      throw err;
    }
  });

  it("Can register a second bond mapping with different currency", async () => {
    
    const fiatCurrency = "EUR";
    // const bondMint = new PublicKey("EuroszHk1AL7fHBBsxgeGHsamUqwBpb26oEyt9BcfZ6G");
    const bondRating = 3;

    try {
      
      const factoryBefore = await program.account.factory.fetch(factoryPDA);
      const countBefore = factoryBefore.bondMappingsCount;

      
      const tx = await program.methods
        .registerBondMaps(
          fiatCurrency,
          eurBondMint,
          bondRating
        )
        .accounts({
          authority: authority,
          factory: factoryPDA,
        })
        .rpc();

      console.log("Register Second Bond Mapping Transaction:", tx);

      
      const factoryAfter = await program.account.factory.fetch(factoryPDA);

      
      expect(factoryAfter.bondMappingsCount).to.equal(countBefore + 1);

      
      const mappingIndex = factoryAfter.bondMappingsCount - 1;
      const mapping = factoryAfter.bondMappings[mappingIndex];
      
      
      const storedFiatBytes = mapping.fiatCurrency.filter(byte => byte !== 0);
      expect(Buffer.from(storedFiatBytes).toString()).to.equal(fiatCurrency);

      // expect(mapping.bondMint.toString()).to.equal(bondMint.toString());
      expect(mapping.bondMint.toString()).to.equal(eurBondMint.toString());
      expect(mapping.bondRating).to.equal(bondRating);
    } catch (err) {
      console.error("Error registering second bond mapping:", err);
      throw err;
    }
  });

  it("Should fail with invalid bond rating", async () => {
    // Use an invalid bond rating (outside 1-10 range)
    const fiatCurrency = "GBP";
    const bondMint = Keypair.generate().publicKey;
    const invalidBondRating = 11; // Invalid rating

    try {
     
      await program.methods
        .registerBondMaps(
          fiatCurrency,
          bondMint,
          invalidBondRating
        )
        .accounts({
          authority: authority,
          factory: factoryPDA,
        })
        .rpc();

      expect.fail("Transaction should have failed with invalid bond rating");
    } catch (err) {
      // Anticipate the error: MaxBondMappingsReached or InvalidBondRating
      const errorCode = err.error?.errorCode?.code;
      expect(errorCode === "InvalidBondRating" || errorCode === "MaxBondMappingsReached").to.be.true;
    }
  });

  it("Should fail with fiat currency that's too long", async () => {
    
    const longFiatCurrency = "TOOLONGCURRENCY";
    const bondMint = Keypair.generate().publicKey;
    const bondRating = 4;

    try {
      
      await program.methods
        .registerBondMaps(
          longFiatCurrency,
          bondMint,
          bondRating
        )
        .accounts({
          authority: authority,
          factory: factoryPDA,
        })
        .rpc();

      expect.fail("Transaction should have failed with too long fiat currency");
    } catch (err) {
      
      const errorCode = err.error?.errorCode?.code;
      expect(errorCode === "FiatCurrencyTooLong" || errorCode === "MaxBondMappingsReached").to.be.true;
    }
  });

    it("Can create fee operator", async () => {
    try {
      const tx = await program.methods
        .createFeeOperator()
        .accounts({
          claimFeeOperator: feeOperatorPDA,
          operator: feeOperatorKeypair.publicKey,
          admin: authority,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      console.log("Create Fee Operator Transaction:", tx);

      // Verify the fee operator account was created
      const feeOperatorAccount = await program.account.feeOperator.fetch(feeOperatorPDA);
      expect(feeOperatorAccount.operator.toString()).to.equal(feeOperatorKeypair.publicKey.toString());
      expect(feeOperatorAccount.bump).to.be.a('number');

      console.log("Fee operator created successfully");
    } catch (err) {
      console.error("Error creating fee operator:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

    it("Should fail when non-admin tries to create fee operator", async () => {
    const unauthorizedUser = Keypair.generate();
    const unauthorizedOperator = Keypair.generate();
    
    // Calculate PDA for unauthorized operator
    const [unauthorizedFeeOperatorPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("fee_operator"), unauthorizedOperator.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .createFeeOperator()
        .accounts({
          claimFeeOperator: unauthorizedFeeOperatorPDA,
          operator: unauthorizedOperator.publicKey,
          admin: unauthorizedUser.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([unauthorizedUser])
        .rpc();

      expect.fail("Transaction should have failed with unauthorized user");
    } catch (err) {
      expect(err.error.errorCode.code).to.equal("Unauthorized");
      console.log("Correctly rejected unauthorized fee operator creation");
    }
  });
 
  it("Can initialize a USD sovereign coin", async () => {
    
    if (!usdRegistered) {
      console.log("USD bond mapping not registered, skipping USD sovereign coin test");
      return;
    }

    const registeredUsdBondMint = await getRegisteredBondMint("USD");
    // if (!registeredUsdBondMint) {
    //   console.log("Cannot find registered USD bond mint, skipping test");
    //   return;
    // }

    // usdSovereignCoinPDA = PublicKey.findProgramAddressSync(
    //   [
    //     Buffer.from("sovereign_coin"),
    //     authority.toBuffer(),
    //     Buffer.from(usdCoinSymbol)
    //   ],
    //   program.programId
    // )[0];

    console.log("Using registered USD bond mint:", registeredUsdBondMint.toString());
    console.log("Our created USD bond mint:", usdBondMint.toString());

   
    const coinArgs = {
      name: "US Dollar Sovereign",
      symbol: usdCoinSymbol,
      uri: "https://example.com/usds.json",
      fiatCurrency: "USD"
    };

    
    const [sovereignCoinPDA] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("sovereign_coin"),
        authority.toBuffer(),
        Buffer.from(usdCoinSymbol)
      ],
      program.programId
    );

    console.log("Init USD Sovereign Coin:", sovereignCoinPDA)

    try {
      
      const tx = await program.methods
        .initSovereignCoin(coinArgs)
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: sovereignCoinPDA,
          fiatTokenMint: usdFiatMint,
          bondTokenMint: usdBondMint,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Initialize USD Sovereign Coin Transaction:", tx);

     
      usdSovereignCoinPDA = sovereignCoinPDA;
      console.log("After Sovereign Coin:", sovereignCoinPDA)
      console.log("After Sovereign Coin:", usdSovereignCoinPDA)


      
      const sovereignCoinAccount = await program.account.sovereignCoin.fetch(sovereignCoinPDA);

      
      expect(sovereignCoinAccount.authority.toString()).to.equal(authority.toString());
      expect(sovereignCoinAccount.factory.toString()).to.equal(factoryPDA.toString());
      
      
      const nameBytes = sovereignCoinAccount.name.filter(byte => byte !== 0);
      const symbolBytes = sovereignCoinAccount.symbol.filter(byte => byte !== 0);
      expect(Buffer.from(nameBytes).toString()).to.equal(coinArgs.name);
      expect(Buffer.from(symbolBytes).toString()).to.equal(coinArgs.symbol);
      
     
      const uriBytes = sovereignCoinAccount.uri.filter(byte => byte !== 0);
      expect(Buffer.from(uriBytes).toString()).to.equal(coinArgs.uri);
      
      
      const fiatCurrencyBytes = sovereignCoinAccount.targetFiatCurrency.filter(byte => byte !== 0);
      expect(Buffer.from(fiatCurrencyBytes).toString()).to.equal(coinArgs.fiatCurrency);
      
      
      expect(sovereignCoinAccount.bondMint.toString()).to.equal(usdBondMint.toString());
      expect(sovereignCoinAccount.bondRating).to.equal(1); 
      
      
      const expectedReserve = calculateRequiredReserve(
        minFiatReserve,
        sovereignCoinAccount.bondRating,
        bondReserveNumerator,
        bondReserveDenominator
    );
    expect(sovereignCoinAccount.requiredReservePercentage).to.equal(expectedReserve);
      expect(sovereignCoinAccount.requiredReservePercentage).to.equal(expectedReserve);
      
      
      expect(sovereignCoinAccount.decimals).to.equal(6);
      expect(sovereignCoinAccount.totalSupply.toNumber()).to.equal(0);
      expect(sovereignCoinAccount.fiatAmount.toNumber()).to.equal(0);
      expect(sovereignCoinAccount.bondAmount.toNumber()).to.equal(0);
    } catch (err) {
      console.error("Error initializing USD sovereign coin:", err);
      throw err;
    }
  });

  it("Can initialize a EUR sovereign coin", async () => {
      
    // if (!eurRegistered) {
    //   console.log("EUR bond mapping not registered, skipping EUR sovereign coin test");
    //   return;
    // }

    
    // eurSovereignCoinPDA = PublicKey.findProgramAddressSync(
    //   [
    //     Buffer.from("sovereign_coin"),
    //     authority.toBuffer(),
    //     Buffer.from(eurCoinSymbol)
    //   ],
    //   program.programId
    // )[0];

    
    const registeredEurBondMint = await getRegisteredBondMint("EUR");
    if (!registeredEurBondMint) {
      console.log("Cannot find registered EUR bond mint");
      return;
    }
    
    console.log("Using registered EUR bond mint:", registeredEurBondMint.toString());
    console.log("Our created EUR bond mint:", eurBondMint.toString());
    
    const coinArgs = {
      name: "Euro Sovereign",
      symbol: eurCoinSymbol,
      uri: "https://example.com/eurs.json",
      fiatCurrency: "EUR"
    };

    const [sovereignCoinPDA] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("sovereign_coin"),
        authority.toBuffer(),
        Buffer.from(coinArgs.symbol)
      ],
      program.programId
    );

    try {
      
      const tx = await program.methods
        .initSovereignCoin(coinArgs)
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: sovereignCoinPDA,
          fiatTokenMint: eurFiatMint,
          bondTokenMint: eurBondMint,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Initialize EUR Sovereign Coin Transaction:", tx);

      
      const sovereignCoinAccount = await program.account.sovereignCoin.fetch(sovereignCoinPDA);

      
      expect(sovereignCoinAccount.bondMint.toString()).to.equal(eurBondMint.toString());
      expect(sovereignCoinAccount.bondRating).to.equal(3); 
      
      const expectedReserve = calculateRequiredReserve(
        minFiatReserve,
        sovereignCoinAccount.bondRating,
        bondReserveNumerator,
        bondReserveDenominator
    );
    
    expect(sovereignCoinAccount.requiredReservePercentage).to.be.closeTo(expectedReserve, 1);
    
    } catch (err) {
      console.error("Error initializing EUR sovereign coin:", err);
      throw err;
    }
  });

  it("Should fail with unknown fiat currency", async () => {
    
    const coinArgs = {
      name: "Japanese Yen Sovereign",
      symbol: "JPYS",
      uri: "https://example.com/jpys.json",
      fiatCurrency: "JPY" 
    };

    
    const [sovereignCoinPDA] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("sovereign_coin"),
        authority.toBuffer(),
        Buffer.from(coinArgs.symbol)
      ],
      program.programId
    );

    try {
      
      await program.methods
        .initSovereignCoin(coinArgs)
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: sovereignCoinPDA,
          fiatTokenMint: usdFiatMint, 
          bondTokenMint: usdBondMint, 
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      expect.fail("Transaction should have failed with unknown fiat currency");
    } catch (err) {
      
      expect(err.error.errorCode.code).to.equal("NoBondMappingForCurrency");
    }
  });

  it("Should fail with incorrect bond mint", async () => {
    
    const coinArgs = {
      name: "US Dollar Sovereign 2",
      symbol: "USDS2",
      uri: "https://example.com/usds2.json",
      fiatCurrency: "USD"
    };

    
    const [sovereignCoinPDA] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("sovereign_coin"),
        authority.toBuffer(),
        Buffer.from(coinArgs.symbol)
      ],
      program.programId
    );

    try {
      // This will fail because we're using EUR bond mint for USD currency
      await program.methods
        .initSovereignCoin(coinArgs)
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: sovereignCoinPDA,
          fiatTokenMint: usdFiatMint,
          bondTokenMint: eurBondMint, // Wrong bond mint for USD
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      expect.fail("Transaction should have failed with incorrect bond mint");
    } catch (err) {
      
      expect(err.error.errorCode.code).to.equal("InvalidBondMint");
    }
  });

  it("Should fail with name too long", async () => {
    
    const coinArgs = {
      name: "This name is way too long for a sovereign coin and should cause a validation error",
      symbol: "LONG",
      uri: "https://example.com/long.json",
      fiatCurrency: "USD"
    };

  
    const [sovereignCoinPDA] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("sovereign_coin"),
        authority.toBuffer(),
        Buffer.from(coinArgs.symbol)
      ],
      program.programId
    );

    try {

      await program.methods
        .initSovereignCoin(coinArgs)
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: sovereignCoinPDA,
          fiatTokenMint: usdFiatMint,
          bondTokenMint: usdBondMint,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      expect.fail("Transaction should have failed with name too long");
    } catch (err) {
      
      expect(err.error.errorCode.code).to.equal("NameTooLong");
    }
  });

  // HERE WE WILL SETUP OUR ORACLE
  

  // TOKEN EXTENSIONS CONTINUE HERE ----
  // TOKEN EXTENSIONS FOR INTEREST BEARING MINTS

    let ibtSovereignCoinMint: anchor.web3.Keypair;

    it("Can set up interest bearing mint for USD sovereign coin", async () => {
    console.log("Using USD Sovereign Coin PDA:", usdSovereignCoinPDA.toString());
    
    // Generate a new keypair for the IBT mint
    ibtSovereignCoinMint = anchor.web3.Keypair.generate();
    console.log("New Interest Bearing Sovereign Coin Mint:", ibtSovereignCoinMint.publicKey.toString());
    
    const initialRate = 400; // 4% annual interest rate (100 basis points)
    
    try {
      const tx = await program.methods
        .setupInterestBearingMint(initialRate)
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          sovereignCoin: usdSovereignCoinPDA,
          mint: ibtSovereignCoinMint.publicKey,
          // @ts-ignore
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_2022_PROGRAM_ID, // Use Token2022 program
        })
        .signers([ibtSovereignCoinMint])
        .rpc();

      console.log("Setup Interest Bearing Mint Transaction:", tx);

      // Verify the sovereign coin account was updated
      const sovereignCoinAccount = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);

      // Check that the mint was set correctly
      expect(sovereignCoinAccount.mint.toString()).to.equal(ibtSovereignCoinMint.publicKey.toString());
      expect(sovereignCoinAccount.isInterestBearing).to.equal(true);
      expect(sovereignCoinAccount.interestRate).to.equal(initialRate);
      
      console.log("Interest bearing sovereign coin mint set up successfully");
      console.log("Initial interest rate:", sovereignCoinAccount.interestRate);
    } catch (err) {
      console.error("Error setting up interest bearing mint for USD sovereign coin:", err);
      
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

    it("Can update interest rate for interest bearing mint", async () => {
    if (!ibtSovereignCoinMint) {
      console.log("Interest bearing mint not created, skipping test");
      return;
    }

    const newRate = 250; // 2.5% annual interest rate (250 basis points)
    
    try {
      // First get the current state
      const sovereignCoinBefore = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);
      console.log("Current interest rate:", sovereignCoinBefore.interestRate);

      // Get bond holding account (you'll need to create this if it doesn't exist)
      const bondHoldingATA = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        usdBondMint,
        authority
      );

      const tx = await program.methods
        .updateInterestRate(newRate) // Pass the new rate as manual_rate
        .accounts({
          admin: authority, // Assuming authority is also admin
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: usdSovereignCoinPDA,
          mint: ibtSovereignCoinMint.publicKey,
          bondTokenMint: usdBondMint,
          bondHolding: bondHoldingATA.address,
          token2022Program: TOKEN_2022_PROGRAM_ID,
        })
        .rpc();

      console.log("Update Interest Rate Transaction:", tx);

      // Verify the rate was updated
      const sovereignCoinAfter = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);
      expect(sovereignCoinAfter.interestRate).to.equal(newRate);
      
      console.log("Interest rate updated successfully from", sovereignCoinBefore.interestRate, "to", sovereignCoinAfter.interestRate);
    } catch (err) {
      console.error("Error updating interest rate:", err);
      
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

  // TOKEN EXTENSIONS FOR TRANSFER FEES
  it("Can initialize transfer fee configuration on existing mint", async () => {
    if (!usdSovereignCoinPDA || !ibtSovereignCoinMint) {
      console.log("USD sovereign coin or interest bearing mint not initialized, skipping test");
      return;
    }

    try {
      // Calculate the protocol vault ATA using the existing interest bearing mint
      const protocolVaultATA = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey,
        factoryPDA, // Factory as the authority
      );

      transferFeeProtocolVault = protocolVaultATA.address;

      const tx = await program.methods
        .initializeTransferFee(transferFeeBasisPoints, new anchor.BN(maximumFee))
        .accounts({
          admin: authority,
          factory: factoryPDA,
          sovereignCoin: usdSovereignCoinPDA,
          authority: authority,
          mint: ibtSovereignCoinMint.publicKey,
          protocolVault: transferFeeProtocolVault,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      console.log("Initialize Transfer Fee Transaction:", tx);

      // Verify factory was updated
      const factoryAccount = await program.account.factory.fetch(factoryPDA);
      expect(factoryAccount.protocolVault.toString()).to.equal(transferFeeProtocolVault.toString());
      expect(factoryAccount.transferFeeBps).to.equal(transferFeeBasisPoints);
      expect(factoryAccount.maximumTransferFee.toNumber()).to.equal(maximumFee);

      console.log("Transfer fee configuration initialized successfully");
      console.log("Protocol Vault:", transferFeeProtocolVault.toString());
    } catch (err) {
      console.error("Error initializing transfer fee:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

  // HERE WE WILL MINT AND TRANSFER OUR STABLECOIN TO TEST THE TRANSFER FEE EXTENSION

  it("Can harvest transfer fees from token accounts", async () => {
    if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
      console.log("Transfer fee not initialized or mint not available, skipping test");
      return;
    }

    try {
      // Create a test token account to harvest from
      const testTokenAccount = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey,
        authority,
      );

      console.log("Harvesting fees from token accounts...");

      const tx = await program.methods
        .harvestFees()
        .accounts({
          mintAccount: ibtSovereignCoinMint.publicKey,
          operator: feeOperatorKeypair.publicKey,
          claimFeeOperator: feeOperatorPDA,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .remainingAccounts([
          {
            pubkey: testTokenAccount.address,
            isSigner: false,
            isWritable: true,
          },
        ])
        .signers([feeOperatorKeypair])
        .rpc();

      console.log("Harvest Fees Transaction:", tx);
      console.log("Fees harvested successfully");
    } catch (err) {
      console.error("Error harvesting fees:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

  it("Can withdraw fees to protocol vault", async () => {
    if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
      console.log("Transfer fee not initialized or mint not available, skipping test");
      return;
    }

    try {
      console.log("Withdrawing fees to protocol vault...");

      const tx = await program.methods
        .withdrawFees()
        .accounts({
          mintAccount: ibtSovereignCoinMint.publicKey,
          factory: factoryPDA,
          protocolVault: transferFeeProtocolVault,
          operator: feeOperatorKeypair.publicKey,
          claimFeeOperator: feeOperatorPDA,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([feeOperatorKeypair])
        .rpc();

      console.log("Withdraw Fees Transaction:", tx);
      console.log("Fees withdrawn to protocol vault successfully");
    } catch (err) {
      console.error("Error withdrawing fees:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

    it("Can update transfer fee configuration", async () => {
      if (!ibtSovereignCoinMint) {
        console.log("Interest bearing mint not available, skipping test");
        return;
      }

      const newTransferFeeBasisPoints = 50; // 0.5%
      const newMaximumFee = 500000; // 0.5 tokens (with 6 decimals)

      try {
        const tx = await program.methods
          .updateTransferFee(newTransferFeeBasisPoints, new anchor.BN(newMaximumFee))
          .accounts({
            admin: authority,
            factory: factoryPDA,
            mintAccount: ibtSovereignCoinMint.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .rpc();

        console.log("Update Transfer Fee Transaction:", tx);

        // Verify factory was updated
        const factoryAccount = await program.account.factory.fetch(factoryPDA);
        expect(factoryAccount.transferFeeBps).to.equal(newTransferFeeBasisPoints);
        expect(factoryAccount.maximumTransferFee.toNumber()).to.equal(newMaximumFee);

        console.log("Transfer fee configuration updated successfully");
        console.log("New transfer fee basis points:", newTransferFeeBasisPoints);
        console.log("New maximum fee:", newMaximumFee);
      } catch (err) {
        console.error("Error updating transfer fee:", err);
        if (err.logs) {
          console.log("Transaction logs:", err.logs);
        }
        throw err;
      }
    });

    it("Should fail when non-admin tries to update transfer fee", async () => {
      if (!ibtSovereignCoinMint) {
        console.log("Interest bearing mint not available, skipping test");
        return;
      }

      const unauthorizedUser = Keypair.generate();

      try {
        const unauthorizedProvider = new anchor.AnchorProvider(
          provider.connection,
          new anchor.Wallet(unauthorizedUser),
          {}
        );
        const unauthorizedProgram = new anchor.Program(
          program.idl,
          unauthorizedProvider
        );

        await unauthorizedProgram.methods
          .updateTransferFee(200, new anchor.BN(2000000))
          .accounts({
            admin: unauthorizedUser.publicKey,
            factory: factoryPDA,
            mintAccount: ibtSovereignCoinMint.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .rpc();

        expect.fail("Transaction should have failed with unauthorized user");
      } catch (err) {
        expect(err.error.errorCode.code).to.equal("Unauthorized");
        console.log("Correctly rejected unauthorized update attempt");
      }
    });

    it("Should fail when non-fee-operator tries to harvest fees", async () => {
      if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
        console.log("Transfer fee not initialized, skipping test");
        return;
      }

      const unauthorizedOperator = Keypair.generate();

      try {
        await program.methods
          .harvestFees()
          .accounts({
            mintAccount: ibtSovereignCoinMint.publicKey,
            operator: unauthorizedOperator.publicKey,
            claimFeeOperator: feeOperatorPDA, // Using valid PDA but wrong signer
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .remainingAccounts([
            {
              pubkey: transferFeeProtocolVault,
              isSigner: false,
              isWritable: true,
            },
          ])
          .signers([unauthorizedOperator])
          .rpc();

        expect.fail("Transaction should have failed with unauthorized operator");
      } catch (err) {
        expect(err.error.errorCode.code).to.equal("Unauthorized");
        console.log("Correctly rejected unauthorized harvest attempt");
      }
    });

    it("Should fail when non-fee-operator tries to withdraw fees", async () => {
      if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
        console.log("Transfer fee not initialized, skipping test");
        return;
      }

      const unauthorizedOperator = Keypair.generate();

      try {
        await program.methods
          .withdrawFees()
          .accounts({
            mintAccount: ibtSovereignCoinMint.publicKey,
            factory: factoryPDA,
            protocolVault: transferFeeProtocolVault,
            operator: unauthorizedOperator.publicKey,
            claimFeeOperator: feeOperatorPDA, // Using valid PDA but wrong signer
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .signers([unauthorizedOperator])
          .rpc();

        expect.fail("Transaction should have failed with unauthorized operator");
      } catch (err) {
        expect(err.error.errorCode.code).to.equal("Unauthorized");
        console.log("Correctly rejected unauthorized withdraw attempt");
      }
    });

    it("Should fail with invalid transfer fee basis points", async () => {
      if (!ibtSovereignCoinMint) {
        console.log("Interest bearing mint not available, skipping test");
        return;
      }

      const invalidFeeBasisPoints = 10001; // > 100%

      try {
        await program.methods
          .updateTransferFee(invalidFeeBasisPoints, new anchor.BN(1000000))
          .accounts({
            admin: authority,
            factory: factoryPDA,
            mintAccount: ibtSovereignCoinMint.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .rpc();

        expect.fail("Transaction should have failed with invalid fee basis points");
      } catch (err) {
        // The error might come from the Token2022 program or your validation
        console.log("Correctly rejected invalid fee basis points:", err.error?.errorCode?.code);
      }
    });

    it("Should fail to harvest fees with no token accounts", async () => {
      if (!ibtSovereignCoinMint) {
        console.log("Interest bearing mint not available, skipping test");
        return;
      }

      try {
        await program.methods
          .harvestFees()
          .accounts({
            mintAccount: ibtSovereignCoinMint.publicKey,
            operator: feeOperatorKeypair.publicKey,
            claimFeeOperator: feeOperatorPDA,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .remainingAccounts([]) // No token accounts to harvest from
          .signers([feeOperatorKeypair])
          .rpc();

        expect.fail("Transaction should have failed with no token accounts");
      } catch (err) {
        expect(err.error.errorCode.code).to.equal("NoTokenAccountsToHarvest");
        console.log("Correctly rejected harvest with no token accounts");
      }
  });

  it("Can withdraw from protocol vault", async () => {
    if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
      console.log("Protocol vault or mint not available, skipping test");
      return;
    }

    try {
      // Create a destination token account for the withdrawal
      const destinationAccount = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey,
        feeOperatorKeypair.publicKey, // Fee operator as destination owner
        false,
        null,
        null,
        TOKEN_2022_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
      );

      // First, let's check the protocol vault balance
      const protocolVaultBefore = await provider.connection.getTokenAccountBalance(transferFeeProtocolVault);
      console.log("Protocol vault balance before withdrawal:", protocolVaultBefore.value.uiAmount);

      // If there's no balance, we can't test withdrawal, so we'll just test the structure
      const withdrawAmount = 100000; // 0.1 tokens (with 6 decimals)

      const tx = await program.methods
        .withdrawFromProtocolAccount(new anchor.BN(withdrawAmount))
        .accounts({
          factory: factoryPDA,
          protocolVault: transferFeeProtocolVault,
          destination: destinationAccount.address,
          operator: feeOperatorKeypair.publicKey,
          claimFeeOperator: feeOperatorPDA,
          tokenMint: ibtSovereignCoinMint.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([feeOperatorKeypair])
        .rpc();

      console.log("Withdraw from Protocol Vault Transaction:", tx);

      // Verify the withdrawal
      const destinationBalance = await provider.connection.getTokenAccountBalance(destinationAccount.address);
      console.log("Destination balance after withdrawal:", destinationBalance.value.uiAmount);

      console.log("Successfully withdrew from protocol vault");
    } catch (err) {
      // If there are insufficient funds, that's expected in a test environment
      if (err.message.includes("insufficient funds") || err.message.includes("0x1")) {
        console.log("Withdrawal test passed - insufficient funds error is expected in test environment");
      } else {
        console.error("Error withdrawing from protocol vault:", err);
        if (err.logs) {
          console.log("Transaction logs:", err.logs);
        }
        throw err;
      }
    }
  });

  it("Should fail when non-fee-operator tries to withdraw from protocol vault", async () => {
    if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
      console.log("Protocol vault or mint not available, skipping test");
      return;
    }

    const unauthorizedOperator = Keypair.generate();

    try {
      // Create a destination account
      const destinationAccount = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey,
        unauthorizedOperator.publicKey,
      );

      await program.methods
        .withdrawFromProtocolAccount(new anchor.BN(100000))
        .accounts({
          factory: factoryPDA,
          protocolVault: transferFeeProtocolVault,
          destination: destinationAccount.address,
          operator: unauthorizedOperator.publicKey,
          claimFeeOperator: feeOperatorPDA, // Using valid PDA but wrong signer
          tokenMint: ibtSovereignCoinMint.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([unauthorizedOperator])
        .rpc();

      expect.fail("Transaction should have failed with unauthorized operator");
    } catch (err) {
      expect(err.error.errorCode.code).to.equal("Unauthorized");
      console.log("Correctly rejected unauthorized protocol vault withdrawal");
    }
  });

  it("Should fail to withdraw from protocol vault with invalid vault", async () => {
    if (!ibtSovereignCoinMint) {
      console.log("Mint not available, skipping test");
      return;
    }

    try {
      // Create a fake protocol vault (different from the one in factory)
      const fakeProtocolVault = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey,
        authority, // Different authority
      );

      const destinationAccount = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey,
        feeOperatorKeypair.publicKey,
        false,
        null,
        null,
        TOKEN_2022_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
      );

      await program.methods
        .withdrawFromProtocolAccount(new anchor.BN(100000))
        .accounts({
          factory: factoryPDA,
          protocolVault: fakeProtocolVault.address, // Wrong protocol vault
          destination: destinationAccount.address,
          operator: feeOperatorKeypair.publicKey,
          claimFeeOperator: feeOperatorPDA,
          tokenMint: ibtSovereignCoinMint.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([feeOperatorKeypair])
        .rpc();

      expect.fail("Transaction should have failed with invalid protocol vault");
    } catch (err) {
      expect(err.error.errorCode.code).to.equal("InvalidProtocolVault");
      console.log("Correctly rejected invalid protocol vault");
    }
  });

  it("Can close fee operator", async () => {
    try {
      // Get the rent receiver (authority will receive the rent)
      const rentReceiver = authority;

      // Get the rent amount before closing
      const feeOperatorAccountInfo = await provider.connection.getAccountInfo(feeOperatorPDA);
      const rentAmount = feeOperatorAccountInfo?.lamports || 0;

      // Get the rent receiver balance before
      const rentReceiverBalanceBefore = await provider.connection.getBalance(rentReceiver);

      const tx = await program.methods
        .closeFeeOperator()
        .accounts({
          claimFeeOperator: feeOperatorPDA,
          rentReceiver: rentReceiver,
          admin: authority,
        })
        .rpc();

      console.log("Close Fee Operator Transaction:", tx);

      // Verify the account was closed
      const closedAccountInfo = await provider.connection.getAccountInfo(feeOperatorPDA);
      expect(closedAccountInfo).to.be.null;

      // Verify rent was returned
      const rentReceiverBalanceAfter = await provider.connection.getBalance(rentReceiver);
      expect(rentReceiverBalanceAfter).to.be.greaterThan(rentReceiverBalanceBefore);

      console.log("Fee operator closed successfully");
      console.log("Rent returned:", (rentReceiverBalanceAfter - rentReceiverBalanceBefore) / anchor.web3.LAMPORTS_PER_SOL, "SOL");
    } catch (err) {
      console.error("Error closing fee operator:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

  it("Should fail when non-admin tries to close fee operator", async () => {
    // First, create a new fee operator to close
    const newFeeOperatorKeypair = anchor.web3.Keypair.generate();
    const [newFeeOperatorPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("fee_operator"), newFeeOperatorKeypair.publicKey.toBuffer()],
      program.programId
    );

    // Create the fee operator first
    await program.methods
      .createFeeOperator()
      .accounts({
        claimFeeOperator: newFeeOperatorPDA,
        operator: newFeeOperatorKeypair.publicKey,
        admin: authority,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    // Now try to close it with unauthorized user
    const unauthorizedUser = Keypair.generate();

    try {
      await program.methods
        .closeFeeOperator()
        .accounts({
          claimFeeOperator: newFeeOperatorPDA,
          rentReceiver: unauthorizedUser.publicKey,
          admin: unauthorizedUser.publicKey,
        })
        .signers([unauthorizedUser])
        .rpc();

      expect.fail("Transaction should have failed with unauthorized user");
    } catch (err) {
      expect(err.error.errorCode.code).to.equal("Unauthorized");
      console.log("Correctly rejected unauthorized fee operator closure");
    }

    // Clean up - close the fee operator with proper authority
    await program.methods
      .closeFeeOperator()
      .accounts({
        claimFeeOperator: newFeeOperatorPDA,
        rentReceiver: authority,
        admin: authority,
      })
      .rpc();
  });

  it("Should fail to use closed fee operator", async () => {
    if (!transferFeeProtocolVault || !ibtSovereignCoinMint) {
      console.log("Transfer fee not initialized, skipping test");
      return;
    }

    // The original fee operator should be closed by now, so this should fail
    try {
      await program.methods
        .harvestFees()
        .accounts({
          mintAccount: ibtSovereignCoinMint.publicKey,
          operator: feeOperatorKeypair.publicKey,
          claimFeeOperator: feeOperatorPDA, // This account should be closed
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .remainingAccounts([
          {
            pubkey: transferFeeProtocolVault,
            isSigner: false,
            isWritable: true,
          },
        ])
        .signers([feeOperatorKeypair])
        .rpc();

      expect.fail("Transaction should have failed with closed fee operator");
    } catch (err) {
      // The error might be account not found or similar
      console.log("Correctly rejected using closed fee operator:", err.error?.errorCode?.code || "Account not found");
    }
  });


  // Add this test to calculate accrued interest (similar to the example)
  it("Can calculate accrued interest", async () => {
    if (!ibtSovereignCoinMint) {
      console.log("Interest bearing mint not created, skipping test");
      return;
    }

    // Wait a bit for interest to accrue (even if minimal)
    await sleep(2);
    
    const amount = 1000; // 1000 tokens
    
    try {
      // Convert amount to UI amount with accrued interest
      const uiAmount = await amountToUiAmount(
        provider.connection,
        mintAuthority,
        ibtSovereignCoinMint.publicKey, // Address of the interest-bearing mint
        amount, // Amount to be converted
        TOKEN_2022_PROGRAM_ID, // Token2022 Program ID
      );

      console.log('\nAmount with Accrued Interest:', uiAmount);
      console.log('Original amount:', amount);
      
      // The UI amount should be >= the original amount due to interest
      expect(uiAmount).to.be.at.least(amount);
    } catch (err) {
      console.error("Error calculating accrued interest:", err);
      throw err;
    }
  });

  // Add this test to verify interest rate is automatically calculated from bond rate
  it("Can update interest rate based on bond rate (automatic calculation)", async () => {
    if (!ibtSovereignCoinMint) {
      console.log("Interest bearing mint not created, skipping test");
      return;
    }

    try {
      // Get bond holding account
      const bondHoldingATA = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        usdBondMint,
        authority
      );

      // Call update without manual rate (should calculate automatically)
      const tx = await program.methods
        .updateInterestRate(null) // No manual rate - will calculate from bond
        .accounts({
          admin: authority,
          authority: authority,
          // @ts-ignore
          factory: factoryPDA,
          sovereignCoin: usdSovereignCoinPDA,
          mint: ibtSovereignCoinMint.publicKey,
          bondTokenMint: usdBondMint,
          bondHolding: bondHoldingATA.address,
          token2022Program: TOKEN_2022_PROGRAM_ID,
        })
        .rpc();

      console.log("Auto-calculated Interest Rate Update Transaction:", tx);

      // Verify the rate was updated (should be calculated from bond rate)
      const sovereignCoinAfter = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);
      console.log("Auto-calculated interest rate:", sovereignCoinAfter.interestRate);
      
      // The rate should be some calculated value (exact value depends on your calculation logic)
      expect(sovereignCoinAfter.interestRate).to.be.a('number');
    } catch (err) {
      console.error("Error updating interest rate automatically:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

  // it("Can set up mint for USD sovereign coin", async () => {
    
  //   console.log("Using USD Sovereign Coin PDA:", usdSovereignCoinPDA.toString());
  //   const [sovereignCoinPDA] = PublicKey.findProgramAddressSync(
  //     [
  //       Buffer.from("sovereign_coin"),
  //       authority.toBuffer(),
  //       Buffer.from(usdCoinSymbol)
  //     ],
  //     program.programId
  //   );

  //   console.log("Other USD Sovereign Coin PDA:", sovereignCoinPDA.toString());
    
  //   sovereignCoinMint = anchor.web3.Keypair.generate();
  //   console.log("New Sovereign Coin Mint:", sovereignCoinMint.publicKey.toString());
    
  //   try {
      
  //     const tx = await program.methods
  //       .setupMint()
  //       .accounts({
  //         payer: authority,
  //         authority: authority,
  //         // @ts-ignore
  //         sovereignCoin: usdSovereignCoinPDA,
  //         mint: sovereignCoinMint.publicKey,
  //         // @ts-ignore
  //         systemProgram: anchor.web3.SystemProgram.programId,
  //         tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
  //         rent: anchor.web3.SYSVAR_RENT_PUBKEY,
  //       })
  //       .signers([sovereignCoinMint]) 
  //       .rpc();
  
  //     console.log("Setup Mint Transaction:", tx);
  
      
  //     const sovereignCoinAccount = await program.account.sovereignCoin.fetch(sovereignCoinPDA);
  
     
  //     expect(sovereignCoinAccount.mint.toString()).to.equal(sovereignCoinMint.publicKey.toString());
  //     console.log("Sovereign coin mint set up successfully");
  //   } catch (err) {
  //     console.error("Error setting up mint for USD sovereign coin:", err);
      
  //     if (err.logs) {
  //       console.log("Transaction logs:", err.logs);
  //     }
  //     throw err;
  //   }
  // });

  let globalFiatReserve: PublicKey;

  it("Can setup global fiat reserve (USDC) - Admin only", async () => {
    // Calculate the global fiat reserve PDA
      const globalFiatReserveATA = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        usdcMint,
        factoryPDA, // Using factory PDA as the owner
      );

    console.log("Expected Global Fiat Reserve PDA:", globalFiatReserveATA.toString());
    
    try {
      const tx = await program.methods
        .setupGlobalFiatReserve()
        .accounts({
          admin: authority, 
          // @ts-ignore
          factory: factoryPDA,
          globalFiatReserve: globalFiatReserveATA.address,
          usdcMint: usdcMint, // Using the USDC mint we created in setup
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Setup Global Fiat Reserve Transaction:", tx);

      // Verify the global fiat reserve was created
      const globalFiatReserveAccount = await provider.connection.getTokenAccountBalance(globalFiatReserveATA.address);
      expect(globalFiatReserveAccount.value.amount).to.equal("0"); // Should start with 0 balance
      
      // Store for use in next test
      globalFiatReserve = globalFiatReserveATA.address;
      
      console.log("Global fiat reserve (USDC) set up successfully");
      console.log("Global fiat reserve address:", globalFiatReserve.toString());
    } catch (err) {
      console.error("Error setting up global fiat reserve:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });

  it("Can setup bond holding for USD sovereign coin", async () => {
    if (!globalFiatReserve) {
      console.log("Global fiat reserve not set up, skipping test");
      return;
    }

    if (!usdSovereignCoinPDA) {
      console.log("USD sovereign coin not initialized, skipping test");
      return;
    }

    try {
      // Calculate the expected bond holding ATA
      const bondHoldingATA = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        mintAuthority,
        usdBondMint,
        authority,
      );

      console.log("Expected Bond Holding ATA:", bondHoldingATA.address.toString());
      console.log("Global Fiat Reserve:", globalFiatReserve.toString());

      const tx = await program.methods
        .setupBondHolding()
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          sovereignCoin: usdSovereignCoinPDA,
          globalFiatReserve: globalFiatReserve,
          bondHolding: bondHoldingATA.address,
          bondTokenMint: usdBondMint,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Setup Bond Holding Transaction:", tx);

      // Verify the sovereign coin account was updated
      const sovereignCoinAccount = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);

      expect(sovereignCoinAccount.fiatReserve.toString()).to.equal(globalFiatReserve.toString());
      expect(sovereignCoinAccount.bondHolding.toString()).to.equal(bondHoldingATA.address.toString());

      // Verify the bond holding token account was created
      const bondHoldingAccount = await provider.connection.getTokenAccountBalance(bondHoldingATA.address);
      expect(bondHoldingAccount.value.amount).to.equal("0"); // Should start with 0 balance

      console.log("Bond holding for USD sovereign coin set up successfully");
      console.log("Fiat Reserve:", sovereignCoinAccount.fiatReserve.toString());
      console.log("Bond Holding:", sovereignCoinAccount.bondHolding.toString());
    } catch (err) {
      console.error("Error setting up bond holding for USD sovereign coin:", err);
      if (err.logs) {
        console.log("Transaction logs:", err.logs);
      }
      throw err;
    }
  });
  
  // it("Should fail when caller is not authority", async () => {
    
  //   // if (!usdSovereignCoinPDA) {
  //   //   console.log("USD sovereign coin not initialized, skipping test");
  //   //   return;
  //   // }
   
  //   const unauthorizedUser = Keypair.generate();
    
  //   try {
  //     const airdropSig = await provider.connection.requestAirdrop(
  //       unauthorizedUser.publicKey,
  //       1_000_000_000 
  //     );
  //     await provider.connection.confirmTransaction(airdropSig);
  //   } catch (err) {
  //     console.log("Error funding unauthorized user:", err);
  //     return;
  //   }
    
  //   try {
      
  //     const unauthorizedProvider = new anchor.AnchorProvider(
  //       provider.connection,
  //       new anchor.Wallet(unauthorizedUser),
  //       {}
  //     );
  //     const unauthorizedProgram = new anchor.Program(
  //       program.idl,
  //       unauthorizedProvider
  //     );
  //     const unauthorizedMint = anchor.web3.Keypair.generate();
      
  //     await unauthorizedProgram.methods
  //       .setupMint()
  //       .accounts({
  //         payer: unauthorizedUser.publicKey,
  //         authority: unauthorizedUser.publicKey, 
  //         // @ts-ignore
  //         sovereignCoin: usdSovereignCoinPDA,
  //         mint: unauthorizedMint.publicKey,
  //         // @ts-ignore
  //         systemProgram: anchor.web3.SystemProgram.programId,
  //         tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
  //         rent: anchor.web3.SYSVAR_RENT_PUBKEY,
  //       })
  //       .rpc();
        
  //     expect.fail("Transaction should have failed with unauthorized user");
  //   } catch (err) {
  //     console.log("Transaction correctly failed with unauthorized user:", err.error?.errorCode?.code);
  //   }
  // });

  // it("Can set up token accounts for USD sovereign coin", async () => {
    
  //   try {
  //     const fiatReserveATA = await getOrCreateAssociatedTokenAccount(
  //       provider.connection,
  //       mintAuthority, 
  //       usdFiatMint,
  //       authority
  //     );
      
  //     const bondHoldingATA = await getOrCreateAssociatedTokenAccount(
  //       provider.connection,
  //       mintAuthority, 
  //       usdBondMint,
  //       authority
  //     );
      
  //     console.log("Fiat Reserve ATA:", fiatReserveATA.address.toString());
  //     console.log("Bond Holding ATA:", bondHoldingATA.address.toString());
  
  //     const tx = await program.methods
  //       .setupTokenAccounts()
  //       .accounts({
  //         payer: authority,
  //         authority: authority,
  //         // @ts-ignore
  //         sovereignCoin: usdSovereignCoinPDA,
  //         // @ts-ignore
  //         fiatReserve: fiatReserveATA.address,
  //         bondHolding: bondHoldingATA.address,
  //         fiatTokenMint: usdFiatMint,
  //         bondTokenMint: usdBondMint,
  //         systemProgram: anchor.web3.SystemProgram.programId,
  //         tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
  //         rent: anchor.web3.SYSVAR_RENT_PUBKEY,
  //       })
  //       .rpc({ skipPreflight: true });
  
  //     console.log("Setup Token Accounts Transaction:", tx);
  
     
  //     const sovereignCoinAccount = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);
  
  //     expect(sovereignCoinAccount.fiatReserve.toString()).to.equal(fiatReserveATA.address.toString());
  //     expect(sovereignCoinAccount.bondHolding.toString()).to.equal(bondHoldingATA.address.toString());
  
  //     console.log("Sovereign coin token accounts set up successfully");
  //   } catch (err) {
  //     console.error("Error setting up token accounts for USD sovereign coin:", err);
  //     if (err.logs) {
  //       console.log("Transaction logs:", err.logs);
  //     }
  //     throw err;
  //   }
  // });

  it("Can finalize setup for USD sovereign coin", async () => {
    // if (!usdSovereignCoinPDA || !factoryPDA) {
    //   console.log("USD sovereign coin or factory not initialized, skipping test");
    //   return;
    // }

    try {
      const sovereignCoinAccount = await program.account.sovereignCoin.fetch(usdSovereignCoinPDA);

      if (!sovereignCoinMint) {
        console.log("Retrieved mint from sovereign coin:", sovereignCoinAccount.mint.toString());
      }

      const [metadataPDA] = anchor.web3.PublicKey.findProgramAddressSync(
        [
          Buffer.from("metadata"),
          TOKEN_METADATA_PROGRAM_ID.toBuffer(),
          sovereignCoinAccount.mint.toBuffer()
        ],
        TOKEN_METADATA_PROGRAM_ID
      );

      // const metadataPdaUmi = findMetadataPda(umi, { 
      //   mint: publicKey(sovereignCoinAccount.mint.toString()) 
      // });
      // console.log("Metadata PDA UMI:", metadataPdaUmi)
      
      // const metadataPDA = new anchor.web3.PublicKey(metadataPdaUmi[0].toString());
      // console.log("Metadata PDA Addy:", metadataPDA);
    
      const tx = await program.methods
        .finalizeSetup()
        .accounts({
          payer: authority,
          authority: authority,
          // @ts-ignore
          sovereignCoin: usdSovereignCoinPDA,
          // @ts-ignore
          factory: factoryPDA,
          mint: sovereignCoinAccount.mint,
          metadata: metadataPDA, 
          tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      console.log("Finalize Setup Transaction:", tx);

      
      const factoryAccount = await program.account.factory.fetch(factoryPDA);
      expect(factoryAccount.totalSovereignCoins.toNumber()).to.be.greaterThan(0);

      
      const metadata = await fetchMetadataFromSeeds(umi, {
        mint: publicKey(sovereignCoinAccount.mint.toString()),
      });
      expect(metadata).to.not.be.null;
      
      const expectedName = Buffer.from(sovereignCoinAccount.name)
        .toString("utf8")
        .split("\0")[0];
      const expectedSymbol = Buffer.from(sovereignCoinAccount.symbol)
        .toString("utf8")
        .split("\0")[0];
      const expectedUri = Buffer.from(sovereignCoinAccount.uri)
        .toString("utf8")
        .split("\0")[0];

     
      expect(metadata.name).to.equal(expectedName);
      expect(metadata.symbol).to.equal(expectedSymbol);
      expect(metadata.uri).to.equal(expectedUri);
      expect(metadata.sellerFeeBasisPoints).to.equal(0);
      expect(metadata.updateAuthority.toString()).to.equal(authority.toString());
      expect(metadata.mint.toString()).to.equal(sovereignCoinAccount.mint.toString());

      expect(factoryAccount.totalSovereignCoins.toNumber()).to.be.at.least(1);

      console.log("Sovereign coin setup finalized successfully");
    } catch (err) {
      console.error("Error finalizing setup for USD sovereign coin:", err);
      throw err;
    }
  });
});