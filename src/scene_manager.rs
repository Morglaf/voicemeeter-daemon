use crate::scenes::ScenesConfig;
use std::sync::{Arc, RwLock};

pub type SharedScenes = Arc<RwLock<ScenesConfig>>;

pub fn new_shared_scenes(config: &ScenesConfig) -> SharedScenes {
    Arc::new(RwLock::new(config.clone()))
}

pub struct SceneManager {
    inner: SharedScenes,
}

impl SceneManager {
    pub fn new(config: &ScenesConfig) -> Self {
        Self {
            inner: new_shared_scenes(config),
        }
    }

    pub fn shared(&self) -> SharedScenes {
        self.inner.clone()
    }

    #[allow(dead_code)]
    pub fn read<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&ScenesConfig) -> T,
    {
        let guard = self.inner.read().expect("scenes lock");
        f(&guard)
    }

    #[allow(dead_code)]
    pub fn write<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut ScenesConfig) -> T,
    {
        let mut guard = self.inner.write().expect("scenes lock");
        f(&mut guard)
    }

    pub fn sync_from_config(&self, config: &ScenesConfig) {
        *self.inner.write().expect("scenes lock") = config.clone();
    }

    #[allow(dead_code)]
    pub fn active_scene_name(&self) -> String {
        self.read(|s| s.active_scene_name().to_string())
    }
}
