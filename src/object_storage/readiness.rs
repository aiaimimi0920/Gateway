use std::path::Path;
use std::time::Duration;

use tokio::fs;
use tokio::time::timeout;
use uuid::Uuid;

use super::local_paths::local_operation_guard;
use super::{GatewayObjectStorage, ObjectStorageDriver, ObjectStorageProbeOutcome};

impl GatewayObjectStorage {
    pub async fn probe_readiness(&self, deadline: Duration) -> ObjectStorageProbeOutcome {
        match &self.driver {
            ObjectStorageDriver::Local { root } => {
                let _operation_guard = local_operation_guard(root).await;
                probe_local_object_storage(root.as_path(), deadline).await
            }
            ObjectStorageDriver::S3Compatible { client, bucket, .. } => {
                match timeout(deadline, client.head_bucket().bucket(bucket).send()).await {
                    Ok(Ok(_)) => ObjectStorageProbeOutcome {
                        ready: true,
                        timed_out: false,
                    },
                    Ok(Err(_)) => ObjectStorageProbeOutcome {
                        ready: false,
                        timed_out: false,
                    },
                    Err(_) => ObjectStorageProbeOutcome {
                        ready: false,
                        timed_out: true,
                    },
                }
            }
        }
    }
}

async fn probe_local_object_storage(root: &Path, deadline: Duration) -> ObjectStorageProbeOutcome {
    match timeout(deadline, fs::create_dir_all(root)).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            return ObjectStorageProbeOutcome {
                ready: false,
                timed_out: false,
            }
        }
        Err(_) => {
            return ObjectStorageProbeOutcome {
                ready: false,
                timed_out: true,
            }
        }
    }

    let probe_path = root.join(format!(".neuro-gateway-readiness-{}.tmp", Uuid::new_v4()));
    let probe_payload = b"neuro-gateway-object-storage-readiness-v1";

    let write = timeout(deadline, fs::write(&probe_path, probe_payload)).await;
    let write_ready = matches!(write, Ok(Ok(())));
    let mut timed_out = write.is_err();

    let read_ready = if write_ready {
        match timeout(deadline, fs::read(&probe_path)).await {
            Ok(Ok(payload)) => payload == probe_payload,
            Ok(Err(_)) => false,
            Err(_) => {
                timed_out = true;
                false
            }
        }
    } else {
        false
    };

    let cleanup_ready = match timeout(deadline, fs::remove_file(&probe_path)).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => true,
        Ok(Err(_)) => false,
        Err(_) => {
            timed_out = true;
            false
        }
    };

    ObjectStorageProbeOutcome {
        ready: write_ready && read_ready && cleanup_ready,
        timed_out,
    }
}
