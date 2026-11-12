// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! The daemon's wire (`CORE_RPC_VERSION` 4.0), read through core's client.
//!
//! Every call is a row of `shekyl_rpc_types::wire`, made with
//! [`Rpc::call`] over [`HttpRpc`], and every reply is core's own type
//! (shekyl-core RK-D1). The wallet keeps no reply type of its own, so a
//! wire change is a compile error here, never a field that silently reads
//! zero.

use std::time::Duration;

use shekyl_rpc_client::{ErrorClass, Rpc, RpcError, RpcMethod};
use shekyl_rpc_transport::HttpRpc;

/// How long one read may take before the wallet treats the node as gone.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// The node's base URL: the configured daemon URL without `/json_rpc`,
/// which the client adds itself.
fn base(url: &str) -> &str {
    let trimmed = url.trim_end_matches('/');
    trimmed
        .strip_suffix("/json_rpc")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
}

/// Call `M` on the node at `url`. `what` names, in the person's words,
/// what the call was for; a refusal or failure is said in terms of it.
pub(crate) async fn call<M: RpcMethod>(
    url: &str,
    request: M::Request,
    what: &str,
) -> Result<M::Reply, String>
where
    M::Request: Send,
{
    let rpc = HttpRpc::new(base(url).to_owned())
        .await
        .map_err(|e| format!("Could not reach the node for {what}: {e}"))?;
    match tokio::time::timeout(CALL_TIMEOUT, rpc.call::<M>(request)).await {
        Ok(Ok(reply)) => Ok(reply),
        Ok(Err(e)) => Err(said(&e, what)),
        Err(_) => Err(format!("The node did not answer in time for {what}.")),
    }
}

/// A failed call in the person's words, read from the error's type: a
/// refusal by its class (`RPC_WIRE.md` RW-Q3), never by its message.
pub(crate) fn said(e: &RpcError, what: &str) -> String {
    match e {
        // The connection does not hold the method's grant (`RPC_CHANNEL.md`
        // §6.1): the wallet's own node gives it the view grants only.
        RpcError::Refused(refusal) if refusal.class == ErrorClass::OutsideTheGrant => {
            format!("This node does not let this wallet see or change {what}.")
        }
        RpcError::Refused(refusal) => refusal.diagnostic.message.clone(),
        RpcError::NoSuchMethod(_) => format!(
            "This node runs a different version of the daemon than this wallet, so it cannot \
             answer for {what}. Update the node or the wallet so they match."
        ),
        other => format!("Could not read {what} from the node: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_rpc_client::{Diagnostic, Refusal};

    fn refused(class: ErrorClass, code: &str, message: &str) -> RpcError {
        RpcError::Refused(Refusal::new(
            class,
            Diagnostic {
                code: code.into(),
                parameter: Default::default(),
                message: message.into(),
            },
        ))
    }

    #[test]
    fn the_base_drops_json_rpc_and_slashes() {
        assert_eq!(
            base("http://127.0.0.1:12029/json_rpc"),
            "http://127.0.0.1:12029"
        );
        assert_eq!(
            base("http://127.0.0.1:12029/json_rpc/"),
            "http://127.0.0.1:12029"
        );
        assert_eq!(base("http://127.0.0.1:12029/"), "http://127.0.0.1:12029");
        assert_eq!(base("http://127.0.0.1:12029"), "http://127.0.0.1:12029");
    }

    #[test]
    fn a_grant_refusal_is_read_by_its_class_not_its_message() {
        let e = refused(
            ErrorClass::OutsideTheGrant,
            "grant.outside",
            "anything at all",
        );
        assert_eq!(
            said(&e, "mining"),
            "This node does not let this wallet see or change mining."
        );
    }

    #[test]
    fn another_refusal_carries_the_node_s_own_sentence() {
        let e = refused(
            ErrorClass::RefusedByMethod,
            "mine.address.unset",
            "the node has no mining address",
        );
        assert_eq!(said(&e, "mining"), "the node has no mining address");
    }

    #[test]
    fn a_method_the_node_lacks_names_the_version_gap() {
        let msg = said(
            &RpcError::NoSuchMethod("daemon_status"),
            "the node's status",
        );
        assert!(msg.contains("different version"), "{msg}");
        assert!(!msg.contains("daemon_status"), "no protocol names: {msg}");
    }
}
