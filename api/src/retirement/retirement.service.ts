import { Injectable, Logger, NotFoundException } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { StellarService } from '../stellar/stellar.service';
import { StellarKeypairService } from '../stellar/stellar-keypair.service';
import { nativeToScVal, scValToNative } from '@stellar/stellar-sdk';
import { RetirementRecord } from '../../../shared';

export class RetireDto {
  buyerPublicKey: string;
  creditId: string;       // hex-encoded BytesN<32>
  tonnes: string;         // i128 as string
  reason: string;
}

@Injectable()
export class RetirementService {
  private readonly logger = new Logger(RetirementService.name);
  private readonly retirementContractId: string;
  private readonly registryContractId: string;

  constructor(
    private readonly stellarService: StellarService,
    private readonly keypairService: StellarKeypairService,
    private readonly configService: ConfigService,
  ) {
    this.retirementContractId = this.configService.get<string>('RETIREMENT_CONTRACT_ID', '');
    this.registryContractId = this.configService.get<string>('CREDIT_REGISTRY_CONTRACT_ID', '');
  }

  async retire(dto: RetireDto): Promise<{ retirementId: string }> {
    this.logger.log(`Retiring credit ${dto.creditId} for ${dto.buyerPublicKey}`);

    const args = [
      nativeToScVal(dto.buyerPublicKey, { type: 'address' }),
      nativeToScVal(Buffer.from(dto.creditId, 'hex'), { type: 'bytes' }),
      nativeToScVal(BigInt(dto.tonnes), { type: 'i128' }),
      nativeToScVal(dto.reason, { type: 'string' }),
      nativeToScVal(this.registryContractId, { type: 'address' }),
    ];

    const signer = this.keypairService.getAdminKeypair();
    const response = await this.stellarService.invokeContract(
      this.retirementContractId,
      'retire',
      args,
      signer,
    );

    // Extract retirement ID (BytesN<32>) from the transaction result
    const retirementId = (response as any).returnValue
      ? Buffer.from(scValToNative((response as any).returnValue) as Uint8Array).toString('hex')
      : 'unknown';

    return { retirementId };
  }

  async getRetirement(retirementId: string): Promise<RetirementRecord> {
    const args = [nativeToScVal(Buffer.from(retirementId, 'hex'), { type: 'bytes' })];
    const retval = await this.stellarService.readContract(
      this.retirementContractId,
      'get_retirement',
      args,
    );

    if (!retval) throw new NotFoundException(`Retirement ${retirementId} not found`);

    const n = scValToNative(retval) as any;
    return {
      id: retirementId,
      credit_id: Buffer.from(n.credit_id as Uint8Array).toString('hex'),
      buyer: n.buyer.toString(),
      tonnes_retired: n.tonnes_retired.toString(),
      reason: n.reason.toString(),
      retired_at: Number(n.retired_at),
      tx_hash: '',
    };
  }

  async getRetirementsByAccount(account: string): Promise<string[]> {
    const args = [nativeToScVal(account, { type: 'address' })];
    const retval = await this.stellarService.readContract(
      this.retirementContractId,
      'get_retirements_by_account',
      args,
    );
    if (!retval) return [];
    const native = scValToNative(retval) as Uint8Array[];
    return native.map((b) => Buffer.from(b).toString('hex'));
  }
}
