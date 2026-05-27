import { Component, inject, OnInit } from '@angular/core';
import { CommonModule } from '@angular/common';
import { CreditStore } from '../core/store/credit.store';
import { StellarWalletService } from '../core/services/stellar-wallet.service';
import { CreditStatus } from '@shared';

@Component({
  selector: 'app-dashboard',
  standalone: true,
  imports: [CommonModule],
  templateUrl: './dashboard.component.html',
  styleUrl: './dashboard.component.css',
})
export class DashboardComponent implements OnInit {
  protected readonly store = inject(CreditStore);
  protected readonly wallet = inject(StellarWalletService);

  // Expose enum to template
  protected readonly CreditStatus = CreditStatus;

  ngOnInit(): void {
    // If a wallet is already connected, load credits for that account
    const key = this.wallet.publicKey();
    if (key) {
      this.store.loadByProject(key);
    }
  }

  async connectWallet(): Promise<void> {
    const publicKey = await this.wallet.connect();
    await this.store.loadByProject(publicKey);
  }

  selectCredit(id: string): void {
    this.store.select(id);
  }

  formatTonnes(raw: string): string {
    // tonnes stored as BigInt string — divide by TONNES_SCALE (1_000_000) to display in tonnes
    return (Number(BigInt(raw)) / 1_000_000).toLocaleString(undefined, { maximumFractionDigits: 4 }) + ' t';
  }
}
