import { Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { firstValueFrom } from 'rxjs';
import { AuthService } from '../core/services/auth.service';
import { StellarWalletService } from '../core/services/stellar-wallet.service';
import { ApiService } from '../core/services/api.service';
import { ConnectWalletComponent } from '../core/components/connect-wallet.component';

type Step = 'form' | 'confirm' | 'success' | 'error';

@Component({
  selector: 'app-retire',
  standalone: true,
  imports: [CommonModule, FormsModule, ConnectWalletComponent],
  template: `
    <div class="retire-wizard">
      <h1>Retire a Credit</h1>

      @if (!auth.isAuthenticated()) {
        <div class="auth-prompt">
          <p>Connect your wallet to retire a credit.</p>
          <app-connect-wallet />
        </div>
      } @else {

        @if (step() === 'form') {
          <form class="wizard-form" (ngSubmit)="goConfirm()" #f="ngForm">
            <label>
              Credit ID (hex)
              <input name="creditId" [(ngModel)]="creditId" required placeholder="037176a1…" />
            </label>
            <label>
              Tonnes (in kg units, e.g. 1000000 = 1 tonne)
              <input name="tonnes" [(ngModel)]="tonnes" required type="number" min="1" placeholder="1000000" />
            </label>
            <label>
              Reason
              <input name="reason" [(ngModel)]="reason" required placeholder="2024 Scope 3 offset" />
            </label>
            <button class="btn btn-primary" type="submit" [disabled]="f.invalid">
              Review →
            </button>
          </form>
        }

        @if (step() === 'confirm') {
          <div class="confirm-box">
            <h2>Confirm Retirement</h2>
            <dl>
              <dt>Credit ID</dt><dd class="mono">{{ creditId }}</dd>
              <dt>Tonnes</dt><dd>{{ formatTonnes(tonnes) }}</dd>
              <dt>Reason</dt><dd>{{ reason }}</dd>
              <dt>Wallet</dt><dd class="mono">{{ wallet.publicKey() }}</dd>
            </dl>
            <div class="actions">
              <button class="btn btn-outline" (click)="step.set('form')">← Back</button>
              <button class="btn btn-danger" [disabled]="submitting()" (click)="submit()">
                {{ submitting() ? 'Submitting…' : 'Confirm Retirement' }}
              </button>
            </div>
          </div>
        }

        @if (step() === 'success') {
          <div class="result success">
            <h2>✅ Credit Retired</h2>
            <p>Retirement ID:</p>
            <code>{{ retirementId() }}</code>
            <button class="btn btn-outline" (click)="reset()">Retire another</button>
          </div>
        }

        @if (step() === 'error') {
          <div class="result error-box">
            <h2>❌ Retirement Failed</h2>
            <p>{{ errorMsg() }}</p>
            <button class="btn btn-outline" (click)="step.set('confirm')">Try again</button>
          </div>
        }

      }
    </div>
  `,
  styles: [`
    .retire-wizard { max-width: 560px; margin: 0 auto; }
    h1 { margin-bottom: 1.5rem; }
    .auth-prompt { display: flex; flex-direction: column; gap: 0.75rem; align-items: flex-start; }
    .wizard-form { display: flex; flex-direction: column; gap: 1rem; }
    label { display: flex; flex-direction: column; gap: 0.3rem; font-size: 0.9rem; font-weight: 500; }
    input { padding: 0.5rem 0.75rem; border: 1px solid #ccc; border-radius: 6px; font-size: 0.95rem; }
    .confirm-box { background: #f9f9f9; border: 1px solid #ddd; border-radius: 8px; padding: 1.5rem; }
    dl { display: grid; grid-template-columns: 140px 1fr; gap: 0.5rem 1rem; margin: 1rem 0; font-size: 0.9rem; }
    dt { font-weight: 600; color: #555; }
    .mono { font-family: monospace; word-break: break-all; }
    .actions { display: flex; gap: 0.75rem; margin-top: 1rem; }
    .result { padding: 1.5rem; border-radius: 8px; }
    .success { background: #e8f5e9; }
    .error-box { background: #ffebee; }
    code { display: block; font-family: monospace; font-size: 0.85rem; word-break: break-all; margin: 0.5rem 0 1rem; }
    .btn { padding: 0.45rem 1.1rem; border-radius: 6px; cursor: pointer; border: none; font-size: 0.9rem; }
    .btn-primary { background: #4caf50; color: #fff; }
    .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
    .btn-danger { background: #e53935; color: #fff; }
    .btn-danger:disabled { opacity: 0.6; cursor: not-allowed; }
    .btn-outline { background: transparent; border: 1px solid #ccc; }
  `],
})
export class RetireComponent {
  protected readonly auth = inject(AuthService);
  protected readonly wallet = inject(StellarWalletService);
  private readonly api = inject(ApiService);

  // form fields
  creditId = '';
  tonnes = 1_000_000;
  reason = '';

  // wizard state
  readonly step = signal<Step>('form');
  readonly submitting = signal(false);
  readonly retirementId = signal<string | null>(null);
  readonly errorMsg = signal<string | null>(null);

  goConfirm(): void {
    this.step.set('confirm');
  }

  async submit(): Promise<void> {
    this.submitting.set(true);
    try {
      const token = this.auth.token()!;
      const { retirementId } = await firstValueFrom(
        this.api.retireCredit(
          {
            buyerPublicKey: this.wallet.publicKey()!,
            creditId: this.creditId,
            tonnes: String(this.tonnes),
            reason: this.reason,
          },
          token,
        ),
      );
      this.retirementId.set(retirementId);
      this.step.set('success');
    } catch (err) {
      this.errorMsg.set(err instanceof Error ? err.message : 'Unknown error.');
      this.step.set('error');
    } finally {
      this.submitting.set(false);
    }
  }

  reset(): void {
    this.creditId = '';
    this.tonnes = 1_000_000;
    this.reason = '';
    this.retirementId.set(null);
    this.errorMsg.set(null);
    this.step.set('form');
  }

  formatTonnes(kg: number): string {
    return (kg / 1_000_000).toLocaleString(undefined, { maximumFractionDigits: 4 }) + ' t';
  }
}
