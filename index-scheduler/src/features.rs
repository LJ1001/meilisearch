use std::sync::{Arc, RwLock};

use meilisearch_types::features::{InstanceTogglableFeatures, RuntimeTogglableFeatures};
use meilisearch_types::heed::types::{SerdeJson, Str};
use meilisearch_types::heed::{Database, Env, RwTxn};

use crate::error::FeatureNotEnabledError;
use crate::Result;

const EXPERIMENTAL_FEATURES: &str = "experimental-features";

#[derive(Clone)]
pub(crate) struct FeatureData {
    persisted: Database<Str, SerdeJson<RuntimeTogglableFeatures>>,
    runtime: Arc<RwLock<RuntimeTogglableFeatures>>,
}

#[derive(Debug, Clone, Copy)]
pub struct RoFeatures {
    runtime: RuntimeTogglableFeatures,
}

impl RoFeatures {
    fn new(data: &FeatureData) -> Self {
        let runtime = data.runtime_features();
        Self { runtime }
    }

    pub fn runtime_features(&self) -> RuntimeTogglableFeatures {
        self.runtime
    }

    pub fn check_metrics(&self) -> Result<()> {
        if self.runtime.metrics {
            Ok(())
        } else {
            Err(FeatureNotEnabledError {
                disabled_action: "Getting metrics",
                feature: "metrics",
                issue_link: "https://github.com/meilisearch/product/discussions/625",
            }
            .into())
        }
    }

    pub fn check_logs_route(&self) -> Result<()> {
        if self.runtime.logs_route {
            Ok(())
        } else {
            Err(FeatureNotEnabledError {
                disabled_action: "getting logs through the `/logs/stream` route",
                feature: "logs route",
                issue_link: "https://github.com/orgs/meilisearch/discussions/721",
            }
            .into())
        }
    }

    pub fn check_vector(&self, disabled_action: &'static str) -> Result<()> {
        if self.runtime.vector_store {
            Ok(())
        } else {
            Err(FeatureNotEnabledError {
                disabled_action,
                feature: "vector store",
                issue_link: "https://github.com/meilisearch/product/discussions/677",
            }
            .into())
        }
    }

    pub fn check_puffin(&self) -> Result<()> {
        if self.runtime.export_puffin_reports {
            Ok(())
        } else {
            Err(FeatureNotEnabledError {
                disabled_action: "Outputting Puffin reports to disk",
                feature: "export puffin reports",
                issue_link: "https://github.com/meilisearch/product/discussions/693",
            }
            .into())
        }
    }
}

impl FeatureData {
    pub fn new(env: &Env, instance_features: InstanceTogglableFeatures) -> Result<Self> {
        let mut wtxn = env.write_txn()?;
        let runtime_features_db = env.create_database(&mut wtxn, Some(EXPERIMENTAL_FEATURES))?;
        wtxn.commit()?;

        let txn = env.read_txn()?;
        let persisted_features: RuntimeTogglableFeatures =
            runtime_features_db.get(&txn, EXPERIMENTAL_FEATURES)?.unwrap_or_default();
        let runtime = Arc::new(RwLock::new(RuntimeTogglableFeatures {
            metrics: instance_features.metrics || persisted_features.metrics,
            logs_route: instance_features.logs_route || persisted_features.logs_route,
            ..persisted_features
        }));

        Ok(Self { persisted: runtime_features_db, runtime })
    }

    pub fn put_runtime_features(
        &self,
        mut wtxn: RwTxn,
        features: RuntimeTogglableFeatures,
    ) -> Result<()> {
        self.persisted.put(&mut wtxn, EXPERIMENTAL_FEATURES, &features)?;
        wtxn.commit()?;

        // safe to unwrap, the lock will only fail if:
        // 1. requested by the same thread concurrently -> it is called and released in methods that don't call each other
        // 2. there's a panic while the thread is held -> it is only used for an assignment here.
        let mut toggled_features = self.runtime.write().unwrap();
        *toggled_features = features;
        Ok(())
    }

    fn runtime_features(&self) -> RuntimeTogglableFeatures {
        // sound to unwrap, the lock will only fail if:
        // 1. requested by the same thread concurrently -> it is called and released in methods that don't call each other
        // 2. there's a panic while the thread is held -> it is only used for copying the data here
        *self.runtime.read().unwrap()
    }

    pub fn features(&self) -> RoFeatures {
        RoFeatures::new(self)
    }
}
