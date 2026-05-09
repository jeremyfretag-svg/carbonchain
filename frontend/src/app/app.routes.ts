import { Routes } from '@angular/router';
import { DashboardComponent } from './dashboard/dashboard.component';
import { MarketplaceComponent } from './marketplace/marketplace.component';
import { RetireComponent } from './retire/retire.component';

export const routes: Routes = [
  { path: '', redirectTo: 'dashboard', pathMatch: 'full' },
  { path: 'dashboard', component: DashboardComponent },
  { path: 'marketplace', component: MarketplaceComponent },
  { path: 'retire', component: RetireComponent },
];
