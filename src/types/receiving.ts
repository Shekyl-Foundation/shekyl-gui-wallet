/**
 * The wallet contract's receiving vocabulary (`docs/api/wallet_rpc.yaml`):
 * payment requests and the `shekyl:` URI. Shekyl has no subaddresses. A
 * request's opaque `rid` rides the link; a payment built from that link
 * echoes it in its encrypted label, so the payee's wallet can attribute the
 * receive. Atomic amounts are decimal strings; format with `formatSkl`.
 * `PaymentRequest.uri` is the stored link, not a contract field.
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
  /**
   * `shekyl:` link composed from this stored request. The same string
   * `create_payment_request` returns. The label, when the request has one,
   * is on this link.
   */
  uri: string;
}

/** `create_payment_request` result: the `rid` and the URI composed from the stored request. */
export interface CreatedPaymentRequest {
  id: string;
  uri: string;
}

export interface PaymentRequests {
  payment_requests: PaymentRequest[];
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
