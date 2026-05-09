import { Component } from '@angular/core';
import { RouterOutlet, RouterLink } from '@angular/router';
import { ConnectWalletComponent } from './core/components/connect-wallet.component';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet, RouterLink, ConnectWalletComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {}
