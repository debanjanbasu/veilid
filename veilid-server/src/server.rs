use crate::client_api;
use crate::settings::*;
use crate::tools::*;
use flume::{unbounded, Receiver, Sender};
use lazy_static::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::*;
use veilid_core::xx::SingleShotEventual;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ServerMode {
    Normal,
    ShutdownImmediate,
    DumpTXTRecord,
}

lazy_static! {
    static ref SHUTDOWN_SWITCH: Mutex<Option<SingleShotEventual<()>>> =
        Mutex::new(Some(SingleShotEventual::new(Some(()))));
}

#[instrument]
pub fn shutdown() {
    let shutdown_switch = SHUTDOWN_SWITCH.lock().take();
    if let Some(shutdown_switch) = shutdown_switch {
        shutdown_switch.resolve(());
    }
}

pub async fn run_veilid_server(settings: Settings, server_mode: ServerMode) -> Result<(), String> {
    run_veilid_server_internal(settings, server_mode).await
}

#[instrument(err, skip_all)]
pub async fn run_veilid_server_internal(
    settings: Settings,
    server_mode: ServerMode,
) -> Result<(), String> {
    trace!(?settings, ?server_mode);

    let settingsr = settings.read();

    // Create client api state change pipe
    let (sender, receiver): (
        Sender<veilid_core::VeilidUpdate>,
        Receiver<veilid_core::VeilidUpdate>,
    ) = unbounded();

    // Create VeilidCore setup
    let update_callback = Arc::new(move |change: veilid_core::VeilidUpdate| {
        if sender.send(change).is_err() {
            error!("error sending veilid update callback");
        }
    });
    let config_callback = settings.get_core_config_callback();

    // Start Veilid Core and get API
    let veilid_api = veilid_core::api_startup(update_callback, config_callback)
        .await
        .map_err(|e| format!("VeilidCore startup failed: {}", e))?;

    // Start client api if one is requested
    let mut capi = if settingsr.client_api.enabled && matches!(server_mode, ServerMode::Normal) {
        let some_capi = client_api::ClientApi::new(veilid_api.clone());
        some_capi
            .clone()
            .run(settingsr.client_api.listen_address.addrs.clone());
        Some(some_capi)
    } else {
        None
    };

    // Drop rwlock on settings
    let auto_attach = settingsr.auto_attach || !matches!(server_mode, ServerMode::Normal);
    drop(settingsr);

    // Process all updates
    let capi2 = capi.clone();
    let update_receiver_jh = spawn_local(async move {
        while let Ok(change) = receiver.recv_async().await {
            if let Some(capi) = &capi2 {
                // Handle state changes on main thread for capnproto rpc
                capi.clone().handle_update(change);
            }
        }
    });

    // Auto-attach if desired
    let mut out = Ok(());
    if auto_attach {
        info!("Auto-attach to the Veilid network");
        if let Err(e) = veilid_api.attach().await {
            let outerr = format!("Auto-attaching to the Veilid network failed: {:?}", e);
            error!("{}", outerr);
            out = Err(outerr);
            shutdown();
        }
    }

    // Process dump-txt-record
    if matches!(server_mode, ServerMode::DumpTXTRecord) {
        let start_time = Instant::now();
        while Instant::now().duration_since(start_time) < Duration::from_secs(10) {
            match veilid_api.get_state().await {
                Ok(vs) => {
                    if vs.network.started {
                        break;
                    }
                }
                Err(e) => {
                    let outerr = format!("Getting state failed: {:?}", e);
                    error!("{}", outerr);
                    out = Err(outerr);
                    break;
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
        match veilid_api.debug("txtrecord".to_string()).await {
            Ok(v) => {
                print!("{}", v);
            }
            Err(e) => {
                let outerr = format!("Getting TXT record failed: {:?}", e);
                error!("{}", outerr);
                out = Err(outerr);
            }
        };
        shutdown();
    }

    // Process shutdown-immediate
    if matches!(server_mode, ServerMode::ShutdownImmediate) {
        shutdown();
    }

    // Idle while waiting to exit
    let shutdown_switch = {
        let shutdown_switch_locked = SHUTDOWN_SWITCH.lock();
        (*shutdown_switch_locked).as_ref().map(|ss| ss.instance())
    };
    if let Some(shutdown_switch) = shutdown_switch {
        shutdown_switch.await;
    }

    // Stop the client api if we have one
    if let Some(c) = capi.as_mut().cloned() {
        c.stop().await;
    }

    // Shut down Veilid API to release state change sender
    veilid_api.shutdown().await;

    // Wait for update receiver to exit
    let _ = update_receiver_jh.await;

    out
}
