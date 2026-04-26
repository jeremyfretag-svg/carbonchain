export enum CreditStatus {
  Pending = 'Pending',
  Active = 'Active',
  Retired = 'Retired',
  Flagged = 'Flagged',
}

export interface CreditMetadata {
  id: string;
  project_id: string;
  issuer: string;
  vintage_year: number;
  methodology: string;
  geography: string;
  tonnes: string; // BigInt as string for precision
  ipfs_hash: string;
  status: CreditStatus;
  issued_at: number;
}

export interface ProjectProfile {
  id: string;
  name: string;
  developer: string;
  description: string;
  location: string;
  methodology: string;
  documents_cid: string;
}

export interface InteractionSession {
  session_id: string;
  initiator: string;
  created_at: number;
  operation_count: number;
  status: 'active' | 'completed';
}
