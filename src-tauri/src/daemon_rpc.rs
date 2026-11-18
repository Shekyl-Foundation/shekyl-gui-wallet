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

//! The daemon's wire, read through core's client.
//!
//! Two legs, one target (shekyl-core `RPC_CHANNEL.md` RT-15: one target per
//! client, never a ladder). The wallet's own node, the default, is called
//! on its owner's leg ([`node_call`]): "this computer", peer-checked, with
//! every grant, since the wallet started that node and is its host's
//! administrator. A node address a person typed is called over HTTP
//! ([`call`]), with whatever that node grants.
//!
//! Every call is a row of `shekyl_rpc_types::wire`, made with
//! [`Rpc::call`] over [`HttpRpc`], and every reply is core's own type
//! (shekyl-core RK-D1). The wallet keeps no reply type of its own, so a
//! wire change is a compile error here, never a field that silently reads
//! zero.

use std::time::Duration;

use shekyl_rpc_client::{ErrorClass, Rpc, RpcError, RpcMethod};
use shekyl_rpc_transport::local::LocalFault;
use shekyl_rpc_transport::{HttpRpc, JsonRpcError};

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
    call_within::<M>(url, request, what, CALL_TIMEOUT).await
}

async fn call_within<M: RpcMethod>(
    url: &str,
    request: M::Request,
    what: &str,
    deadline: Duration,
) -> Result<M::Reply, String>
where
    M::Request: Send,
{
    // One deadline for the whole exchange: with credentials in the URL,
    // `HttpRpc::new` already talks to the node (its authentication
    // challenge), and a node that stalls there must not hold the poll.
    let exchange = async {
        let rpc = HttpRpc::new(base(url).to_owned())
            .await
            .map_err(|e| format!("Could not reach the node for {what}: {e}"))?;
        rpc.call::<M>(request).await.map_err(|e| said(&e, what))
    };
    tokio::time::timeout(deadline, exchange)
        .await
        .unwrap_or_else(|_| Err(format!("The node did not answer in time for {what}.")))
}

/// Call `M` on the wallet's node: over the owner's leg of the node this
/// wallet started, or over HTTP at the node address a person typed. The
/// target decides the leg; a failure on one is never retried on the other.
pub(crate) async fn node_call<M: RpcMethod>(
    state: &crate::state::AppState,
    node: &crate::daemon_manager::DaemonManager,
    request: M::Request,
    what: &str,
) -> Result<M::Reply, String>
where
    M::Request: Send + 'static,
    M::Reply: Send + 'static,
{
    if !state.targets_this_computer().await {
        return call::<M>(&state.url().await, request, what).await;
    }
    let target = node.own_target().await?;
    // The owner's leg is a blocking dial and exchange, under its own
    // deadline (`shekyl_rpc_transport::local::CALL_WITHIN`).
    let reply = tokio::task::spawn_blocking(move || target.call::<M>(&request))
        .await
        .map_err(|e| format!("Could not read {what} from the node: {e}"))?;
    reply.map_err(|e| said_local(&e, what))
}

/// A failed call on the owner's leg in the person's words. A failed peer
/// check is said as core says it, whole: it is a security failure, not a
/// node that is away.
fn said_local(e: &JsonRpcError<LocalFault>, what: &str) -> String {
    match e {
        JsonRpcError::Transport(fault) => fault.to_string(),
        JsonRpcError::Refusal(refusal) if refusal.class == ErrorClass::OutsideTheGrant => {
            format!("This node does not let this wallet see or change {what}.")
        }
        JsonRpcError::Refusal(refusal) => refusal.diagnostic.message.clone(),
        JsonRpcError::NoSuchMethod(_) => different_version(what),
        JsonRpcError::Malformed(detail) => {
            format!("Could not read {what} from the node: {detail}")
        }
    }
}

/// The words for a node that serves no such method: another RPC version.
fn different_version(what: &str) -> String {
    format!(
        "This node runs a different version of the daemon than this wallet, so it cannot \
         answer for {what}. Update the node or the wallet so they match."
    )
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
        RpcError::NoSuchMethod(_) => different_version(what),
        // Core's own sentences (`shekyl_rpc_client`): a failed peer check is
        // a security failure, said whole; a service node says where it
        // listens.
        RpcError::PeerCheck(_) => e.to_string(),
        RpcError::Service(view) => shekyl_rpc_client::service_refusal(*view),
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

    /// With credentials in the URL the client's construction already talks
    /// to the node; a node that accepts and then never answers is still
    /// bounded by the call's deadline.
    #[tokio::test]
    async fn a_node_that_stalls_the_authentication_challenge_times_out() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let held = tokio::spawn(async move {
            let mut open = Vec::new();
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                open.push(socket);
            }
        });
        let url = format!("http://wallet:secret@127.0.0.1:{port}/json_rpc");
        let started = std::time::Instant::now();
        let err = call_within::<shekyl_rpc_client::wire::Version>(
            &url,
            shekyl_rpc_types::kinds::NoParams,
            "the node's version",
            Duration::from_millis(300),
        )
        .await
        .expect_err("a silent node is not an answer");
        held.abort();
        assert_eq!(
            err,
            "The node did not answer in time for the node's version."
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
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
