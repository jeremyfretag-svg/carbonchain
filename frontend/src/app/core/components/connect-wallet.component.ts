import { Component, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { AuthService } from '../services/auth.service';
import { StellarWalletService } from '../services/stellar-wallet.service';

@Component({
  selector: 'app-connect-wallet',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="wallet-connect">
      @if (auth.isAuthenticated()) {
        <div class="wallet-info">
          <span class="pubkey" [title]="wallet.publicKey()!">
            {{ wallet.publicKey()! | slice:0:6 }}…{{ wallet.publicKey()! | slice:-4 }}
          </span>
          <button class="btn btn-outline" (click)="auth.logout()">Disconnect</button>
        </div>
      } @else {
        <button
          class="btn btn-primary"
          [disabled]="auth.authState() === 'authenticating'"
          (click)="login()"
        >
          @if (auth.authState() === 'authenticating') {
            Connecting…
          } @else {
            Connect Wallet
          }
        </button>
        @if (auth.authError()) {
          <p class="error">{{ auth.authError() }}</p>
        }
      }
    </div>
  `,
  styles: [`
    .wallet-connect { display: flex; align-items: center; gap: 0.5rem; }
    .wallet-info { display: flex; align-items: center; gap: 0.75rem; }
    .pubkey { font-family: monospace; font-size: 0.85rem; }
    .btn { padding: 0.4rem 1rem; border-radius: 6px; cursor: pointer; border: none; font-size: 0.9rem; }
    .btn-primary { background: #4caf50; color: #fff; }
    .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
    .btn-outline { background: transparent; border: 1px solid #ccc; }
    .error { color: #e53935; font-size: 0.8rem; margin: 0; }
  `],
})
export class ConnectWalletComponent {
  protected readonly auth = inject(AuthService);
  protected readonly wallet = inject(StellarWalletService);

  async login(): Promise<void> {
    try {
      await this.auth.login();
    } catch {
      // error already stored in auth.authError signal
    }
  }
}
