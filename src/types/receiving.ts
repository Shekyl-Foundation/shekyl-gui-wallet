/**
 * The wallet contract's receiving vocabulary (`docs/api/wallet_rpc.yaml`):
 * payment requests and the `shekyl:` URI. Shekyl has no subaddresses; a
 * request's opaque `rid` on the URI is how a receive is attributed.
 * Atomic amounts are decimal strings; format with `formatSkl`.
 */

export type PaymentRequestState = "PENDING" | "MATCHED" | "EXPIRED" | "CANCELLED";

export type PaymentRequestFilter = "ALL" | "PENDING" | "MATCHED";

export interface PaymentRequest {
  /** Opaque request id (`rid`), decimal string. */
  id: string;
  label: string;
  amount: string;
  /** Unix seconds (UTC). */
  created_at: number;
  /** Unix seconds (UTC); absent when the request never expires. */
  expiry?: number;
  state: PaymentRequestState;
  matched_tx_hash?: string;
  matched_output_index?: number;
}

/** `create_payment_request` result: the `rid` and the URI composed from the stored request. */
export interface CreatedPaymentRequest {
  id: string;
  uri: string;
}

export interface PaymentRequests {
  payment_requests: PaymentRequest[];
}

export interface PaymentUriResult {
  uri: string;
}

/** `parse_uri` result: the link's components, untrusted text to prefill and show. */
export interface ParsedPaymentUri {
  address: string;
  amount?: string;
  label?: string;
  rid?: string;
  expiry?: number;
}

/** The scheme every payment link starts with. */
export const PAYMENT_URI_SCHEME = "shekyl:";
