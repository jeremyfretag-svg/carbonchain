import { Injectable, inject } from '@angular/core';
import { HttpClient, HttpHeaders } from '@angular/common/http';
import { Observable } from 'rxjs';
import { CreditMetadata, ProjectProfile, Offer } from '@shared';

// ---------------------------------------------------------------------------
// Response types mirroring the NestJS controllers
// ---------------------------------------------------------------------------

export interface ChallengeResponse {
  transaction: string;
  network_passphrase: string;
}

export interface TokenResponse {
  access_token: string;
}

export interface MeResponse {
  account: string;
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

@Injectable({ providedIn: 'root' })
export class ApiService {
  private readonly http = inject(HttpClient);

  /** Base URL — override via environment files as needed. */
  private readonly baseUrl = '/api';

  // ── Auth ──────────────────────────────────────────────────────────────────

  /** GET /auth/challenge?account=G... */
  getChallenge(account: string): Observable<ChallengeResponse> {
    return this.http.get<ChallengeResponse>(`${this.baseUrl}/auth/challenge`, {
      params: { account },
    });
  }

  /** POST /auth/token — exchange signed XDR for a JWT. */
  getToken(signedTransaction: string): Observable<TokenResponse> {
    return this.http.post<TokenResponse>(`${this.baseUrl}/auth/token`, {
      transaction: signedTransaction,
    });
  }

  /** GET /auth/me — returns the authenticated account (requires JWT). */
  getMe(token: string): Observable<MeResponse> {
    return this.http.get<MeResponse>(`${this.baseUrl}/auth/me`, {
      headers: this.authHeaders(token),
    });
  }

  // ── Projects ──────────────────────────────────────────────────────────────

  /** GET /projects */
  listProjects(): Observable<ProjectProfile[]> {
    return this.http.get<ProjectProfile[]>(`${this.baseUrl}/projects`);
  }

  /** GET /projects/:id */
  getProject(id: string): Observable<ProjectProfile> {
    return this.http.get<ProjectProfile>(`${this.baseUrl}/projects/${id}`);
  }

  /** POST /projects */
  createProject(data: Omit<ProjectProfile, 'id'>, token: string): Observable<ProjectProfile> {
    return this.http.post<ProjectProfile>(`${this.baseUrl}/projects`, data, {
      headers: this.authHeaders(token),
    });
  }

  // ── Credits ───────────────────────────────────────────────────────────────

  /** GET /credits/:id */
  getCredit(id: string): Observable<CreditMetadata> {
    return this.http.get<CreditMetadata>(`${this.baseUrl}/credits/${id}`);
  }

  /** GET /credits/project/:projectId */
  listCreditsByProject(projectId: string): Observable<string[]> {
    return this.http.get<string[]>(`${this.baseUrl}/credits/project/${projectId}`);
  }

  // ── Marketplace ───────────────────────────────────────────────────────────

  /** GET /marketplace/offer/:id */
  getOffer(id: number): Observable<Offer> {
    return this.http.get<Offer>(`${this.baseUrl}/marketplace/offer/${id}`);
  }

  /** GET /marketplace/seller/:address */
  getOffersBySeller(address: string): Observable<string[]> {
    return this.http.get<string[]>(`${this.baseUrl}/marketplace/seller/${address}`);
  }

  /** POST /marketplace/offer */
  createOffer(
    body: { sellerPublicKey: string; creditId: string; priceXlm: string; tonnes: string },
    token: string,
  ): Observable<{ offerId: string }> {
    return this.http.post<{ offerId: string }>(`${this.baseUrl}/marketplace/offer`, body, {
      headers: this.authHeaders(token),
    });
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  private authHeaders(token: string): HttpHeaders {
    return new HttpHeaders({ Authorization: `Bearer ${token}` });
  }
}
