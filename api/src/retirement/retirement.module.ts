import { Module } from '@nestjs/common';
import { ConfigModule } from '@nestjs/config';
import { RetirementService } from './retirement.service';
import { RetirementController } from './retirement.controller';
import { StellarModule } from '../stellar/stellar.module';

@Module({
  imports: [ConfigModule, StellarModule],
  controllers: [RetirementController],
  providers: [RetirementService],
  exports: [RetirementService],
})
export class RetirementModule {}
