use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use tracing::{debug, error, info, warn};

use crate::{
    app_state::AppState,
    federation::client::FederationClient,
    middlewares::node_auth::AuthenticatedNode,
    models::federation::{NodeInfo, S2sAck, S2sMessagePayload, S2sSessionPayload},
    repository::{
        device_repository, federation_repository, message_repository, session_repository,
    },
};

// ─── GET /s2s/info ───────────────────────────────────────────────────────────

pub async fn node_info(State(state): State<AppState>) -> impl IntoResponse {
    info!(node_id = %state.this_node_id, "GET /s2s/info");
    let info = NodeInfo {
        node_id: state.this_node_id.clone(),
        api_url: state.this_api_url.clone(),
        public_key_b64: state.node_keys.public_b64.clone(),
        protocol_version: "0.0.2",
    };
    (StatusCode::OK, Json(info))
}

// ─── GET /s2s/users/:username/devices ────────────────────────────────────────

pub async fn get_user_devices(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Path(username): Path<String>,
) -> impl IntoResponse {
    info!(peer = %peer.node_id, %username, "GET /s2s/users/:username/devices");

    let user_id =
        match federation_repository::get_local_user_id_by_username(&state.pool, &username).await {
            Ok(Some(id)) => {
                debug!(%username, %id, "local user found");
                id
            }
            Ok(None) => {
                warn!(%username, "user not found or is a shadow record");
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "user not found or not local to this node"})),
                )
                    .into_response();
            }
            Err(e) => {
                error!(%username, err = %e, "db error resolving user");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response();
            }
        };

    match device_repository::get_devices_by_user_id(&state.pool, &user_id).await {
        Ok(devices) => {
            debug!(%username, count = devices.len(), "returning devices");
            (StatusCode::OK, Json(devices)).into_response()
        }
        Err(e) => {
            error!(%username, err = %e, "db error fetching devices");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response()
        }
    }
}

// ─── GET /s2s/users/:username/keys ───────────────────────────────────────────

pub async fn get_user_keys(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Path(username): Path<String>,
) -> impl IntoResponse {
    info!(peer = %peer.node_id, %username, "GET /s2s/users/:username/keys");

    let user_id =
        match federation_repository::get_local_user_id_by_username(&state.pool, &username).await {
            Ok(Some(id)) => {
                debug!(%username, %id, "local user found for key fetch");
                id
            }
            Ok(None) => {
                warn!(%username, "user not found or is a shadow record (key fetch)");
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "user not found or not local to this node"})),
                )
                    .into_response();
            }
            Err(e) => {
                error!(%username, err = %e, "db error resolving user for key fetch");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response();
            }
        };

    match device_repository::get_device_bundle(&state.pool, &user_id).await {
        Ok(bundle) => {
            debug!(%username, devices = bundle.len(), "returning key bundle");
            (StatusCode::OK, Json(bundle)).into_response()
        }
        Err(e) => {
            error!(%username, err = %e, "db error fetching key bundle");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response()
        }
    }
}

// ─── POST /s2s/sessions ──────────────────────────────────────────────────────

pub async fn receive_session(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Json(payload): Json<S2sSessionPayload>,
) -> impl IntoResponse {
    info!(
        peer = %peer.node_id,
        from = %payload.from_federated_address,
        to   = %payload.to_user,
        sessions = payload.sessions_init.len(),
        "POST /s2s/sessions"
    );

    let sender_username = payload
        .from_federated_address
        .split('@')
        .next()
        .unwrap_or("unknown");

    let sender_local_id = match federation_repository::upsert_shadow_user(
        &state.pool,
        sender_username,
        &payload.from_federated_address,
        peer.id,
    )
    .await
    {
        Ok(id) => {
            debug!(federated = %payload.from_federated_address, local_id = %id, "shadow user upserted");
            id
        }
        Err(e) => {
            error!(err = %e, "shadow user upsert failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    if let Err(e) = federation_repository::upsert_shadow_device(
        &state.pool,
        payload.from_device_id,
        sender_local_id,
        &payload.from_identity_pubkey,
    )
    .await
    {
        error!(device_id = %payload.from_device_id, err = %e, "shadow device upsert failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
            .into_response();
    }

    for init in &payload.sessions_init {
        debug!(recipient_device = %init.recipient_device_id, "inserting pending session");
        if let Err(e) = session_repository::create_pending_session(
            &state.pool,
            &payload.from_device_id,
            &init.recipient_device_id,
            &init.ephemeral_pubkey,
            &init.sender_prekey_pub,
            &init.otpk_used,
            &init.ciphertext,
        )
        .await
        {
            error!(recipient_device = %init.recipient_device_id, err = %e, "pending session insert failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to store pending session"})),
            )
                .into_response();
        }
    }

    info!(from = %payload.from_federated_address, to = %payload.to_user, "sessions stored ok");
    (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
}

// ─── POST /s2s/messages ──────────────────────────────────────────────────────

pub async fn receive_messages(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Json(payload): Json<S2sMessagePayload>,
) -> impl IntoResponse {
    info!(
        peer        = %peer.node_id,
        logical_id  = %payload.logical_msg_id,
        from        = %payload.from_federated_address,
        to_user     = %payload.to_user,
        device_count = payload.payloads.len(),
        "POST /s2s/messages"
    );

    let recipient_id =
        match federation_repository::get_local_user_id_by_username(&state.pool, &payload.to_user)
            .await
        {
            Ok(Some(id)) => {
                debug!(username = %payload.to_user, local_id = %id, "recipient resolved");
                id
            }
            Ok(None) => {
                warn!(username = %payload.to_user, "recipient not found or is a shadow record");
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "recipient not found or not local to this node"})),
                )
                    .into_response();
            }
            Err(e) => {
                error!(username = %payload.to_user, err = %e, "db error resolving recipient");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response();
            }
        };

    let sender_username = payload
        .from_federated_address
        .split('@')
        .next()
        .unwrap_or("unknown");

    let sender_local_id = match federation_repository::upsert_shadow_user(
        &state.pool,
        sender_username,
        &payload.from_federated_address,
        peer.id,
    )
    .await
    {
        Ok(id) => {
            debug!(federated = %payload.from_federated_address, local_id = %id, "shadow user upserted");
            id
        }
        Err(e) => {
            error!(err = %e, "shadow user upsert failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    if let Err(e) = federation_repository::upsert_shadow_device(
        &state.pool,
        payload.from_device_id,
        sender_local_id,
        &payload.from_identity_pubkey,
    )
    .await
    {
        error!(device_id = %payload.from_device_id, err = %e, "shadow device upsert failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
            .into_response();
    }

    let chat_id = match federation_repository::get_or_create_direct_chat(
        &state.pool,
        sender_local_id,
        recipient_id,
    )
    .await
    {
        Ok(id) => {
            debug!(chat_id = %id, "chat resolved");
            id
        }
        Err(e) => {
            error!(err = %e, "get_or_create_direct_chat failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    let mut any_new = false;
    for dev in &payload.payloads {
        debug!(to_device = %dev.to_device_id, "inserting device payload");
        match message_repository::insert_federated_message(
            &state.pool,
            &payload.logical_msg_id,
            chat_id,
            sender_local_id,
            payload.from_device_id,
            recipient_id,
            dev.to_device_id,
            &dev.header,
            &dev.ciphertext,
        )
        .await
        {
            Ok(true) => {
                debug!(to_device = %dev.to_device_id, "message inserted");
                any_new = true;
            }
            Ok(false) => {
                debug!(to_device = %dev.to_device_id, "duplicate, skipped");
            }
            Err(e) => {
                error!(to_device = %dev.to_device_id, err = %e, "message insert failed");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response();
            }
        }
    }

    let status = if any_new { "delivered" } else { "duplicate" };
    info!(logical_id = %payload.logical_msg_id, %status, "messages processed");

    let ack = S2sAck {
        logical_msg_id: payload.logical_msg_id,
        status: status.into(),
    };
    (StatusCode::OK, Json(ack)).into_response()
}

// ─── POST /s2s/ack ───────────────────────────────────────────────────────────

pub async fn receive_ack(
    State(state): State<AppState>,
    AuthenticatedNode(peer): AuthenticatedNode,
    Json(ack): Json<S2sAck>,
) -> impl IntoResponse {
    info!(peer = %peer.node_id, logical_id = %ack.logical_msg_id, status = %ack.status, "POST /s2s/ack");

    if let Err(e) =
        federation_repository::mark_outbox_delivered_by_logical_id(&state.pool, &ack.logical_msg_id)
            .await
    {
        error!(logical_id = %ack.logical_msg_id, err = %e, "ack db update failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
            .into_response();
    }
    (StatusCode::OK, Json(json!({"status": "ack received"}))).into_response()
}

// ─── GET /users/federated/:address/keys ──────────────────────────────────────

pub async fn federated_keys(
    State(state): State<AppState>,
    crate::middlewares::auth::AuthenticatedDevice(_device): crate::middlewares::auth::AuthenticatedDevice,
    Path((username, node_id)): Path<(String, String)>,
) -> impl IntoResponse {
    info!(%username, %node_id, "GET /users/federated/:username/:node_id/keys");
    let (username, node_id) = (username.as_str(), node_id.as_str());

    // Local shortcut: address points to this node.
    if node_id == state.this_node_id {
        debug!(%username, "address is local, serving directly");
        let user_id =
            match federation_repository::get_local_user_id_by_username(&state.pool, username).await
            {
                Ok(Some(id)) => id,
                Ok(None) => {
                    warn!(%username, "local user not found");
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "user not found"})),
                    )
                        .into_response();
                }
                Err(e) => {
                    error!(%username, err = %e, "db error on local key fetch");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "internal error"})),
                    )
                        .into_response();
                }
            };
        return match device_repository::get_device_bundle(&state.pool, &user_id).await {
            Ok(bundle) => {
                debug!(%username, devices = bundle.len(), "local bundle returned");
                (StatusCode::OK, Json(bundle)).into_response()
            }
            Err(e) => {
                error!(%username, err = %e, "db error fetching local bundle");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal error"})),
                )
                    .into_response()
            }
        };
    }

    // Remote: resolve the target node.
    debug!(%node_id, "resolving remote node");
    let node = match federation_repository::get_federation_node(&state.pool, node_id).await {
        Ok(Some(n)) => {
            debug!(%node_id, api_url = %n.api_url, "node found in local cache");
            n
        }
        Ok(None) => {
            info!(%node_id, "node not in cache, querying registry");
            let url = format!("{}/api/registry/nodes/{}", state.registry_url, node_id);
            debug!(registry_url = %url, "registry lookup");
            match state.http_client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(body) => {
                            let api_url = body["api_url"].as_str().unwrap_or("");
                            let pubkey = body["public_key_b64"].as_str().unwrap_or("");
                            debug!(%node_id, %api_url, "registry returned node info");
                            match federation_repository::upsert_federation_node(
                                &state.pool,
                                node_id,
                                api_url,
                                pubkey,
                            )
                            .await
                            {
                                Ok(n) => n,
                                Err(e) => {
                                    error!(%node_id, err = %e, "failed to cache node from registry");
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": "internal error"})),
                                    )
                                        .into_response();
                                }
                            }
                        }
                        Err(e) => {
                            error!(%node_id, err = %e, "malformed registry response");
                            return (
                                StatusCode::BAD_GATEWAY,
                                Json(json!({"error": "malformed registry response"})),
                            )
                                .into_response();
                        }
                    }
                }
                Ok(resp) => {
                    warn!(%node_id, status = %resp.status(), "registry returned non-200");
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "target node not found in registry"})),
                    )
                        .into_response();
                }
                Err(e) => {
                    error!(%node_id, err = %e, "registry request failed");
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({"error": "registry unreachable"})),
                    )
                        .into_response();
                }
            }
        }
        Err(e) => {
            error!(%node_id, err = %e, "db error looking up federation node");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            )
                .into_response();
        }
    };

    if node.is_blocked {
        warn!(%node_id, "node is blocked");
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "target node is blocked"})),
        )
            .into_response();
    }

    info!(%node_id, api_url = %node.api_url, %username, "proxying key fetch to remote node");

    let fed_client = FederationClient::new(
        state.http_client.clone(),
        state.node_keys.clone(),
        state.this_node_id.clone(),
    );

    match fed_client.fetch_peer_keys(&node.api_url, username).await {
        Ok(bundle) => {
            info!(%node_id, %username, devices = bundle.len(), "remote key fetch succeeded");
            (StatusCode::OK, Json(bundle)).into_response()
        }
        Err(e) => {
            error!(%node_id, %username, err = %e, "remote key fetch failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("peer returned error: {e}")})),
            )
                .into_response()
        }
    }
}
