//! Persistent server-wide command storage, compatible with Vanilla's
//! `world/data/command_storage_<namespace>.dat` files.

use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
};

use pumpkin_nbt::{
    NbtCompound,
    nbt_compress::{read_gzip_compound_tag, write_gzip_compound_tag},
};
use pumpkin_util::identifier::Identifier;
use tokio::sync::RwLock;
use tracing::warn;

const FILE_PREFIX: &str = "command_storage_";
const FILE_SUFFIX: &str = ".dat";
// Minecraft Java 1.21.4's world data version. Command-storage files written
// for this parity target must remain consumable by an unmodified 1.21.4 server.
const VANILLA_1_21_4_DATA_VERSION: i32 = 4189;

pub struct CommandStorage {
    data_dir: PathBuf,
    namespaces: RwLock<HashMap<String, HashMap<String, NbtCompound>>>,
}

impl CommandStorage {
    #[must_use]
    pub fn load(world_path: &Path) -> Self {
        let data_dir = world_path.join("data");
        let mut namespaces = HashMap::new();

        if let Ok(entries) = fs::read_dir(&data_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let Some(file_name) = file_name.to_str() else {
                    continue;
                };
                let Some(namespace) = file_name
                    .strip_prefix(FILE_PREFIX)
                    .and_then(|name| name.strip_suffix(FILE_SUFFIX))
                else {
                    continue;
                };
                if Identifier::new(namespace.to_owned(), "probe").is_err() {
                    continue;
                }
                match load_namespace(&entry.path()) {
                    Ok(contents) if !contents.is_empty() => {
                        namespaces.insert(namespace.to_owned(), contents);
                    }
                    Ok(_) => {}
                    Err(error) => warn!(
                        "Failed to load command storage {}: {error}",
                        entry.path().display()
                    ),
                }
            }
        }

        Self {
            data_dir,
            namespaces: RwLock::new(namespaces),
        }
    }

    pub async fn get(&self, id: &Identifier) -> NbtCompound {
        self.namespaces
            .read()
            .await
            .get(id.namespace())
            .and_then(|namespace| namespace.get(id.path()))
            .cloned()
            .unwrap_or_default()
    }

    pub async fn set(&self, id: &Identifier, value: NbtCompound) {
        let mut namespaces = self.namespaces.write().await;
        if value.is_empty() {
            if let Some(namespace) = namespaces.get_mut(id.namespace()) {
                namespace.remove(id.path());
                if namespace.is_empty() {
                    namespaces.remove(id.namespace());
                }
            }
        } else {
            namespaces
                .entry(id.namespace().to_owned())
                .or_default()
                .insert(id.path().to_owned(), value);
        }
    }

    pub async fn keys(&self) -> Vec<Identifier> {
        self.namespaces
            .read()
            .await
            .iter()
            .flat_map(|(namespace, values)| {
                values
                    .keys()
                    .filter_map(|path| Identifier::new(namespace.to_owned(), path.to_owned()).ok())
            })
            .collect()
    }

    pub async fn save(&self) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir)
            .map_err(|error| format!("creating {}: {error}", self.data_dir.display()))?;

        // Do not hold the async lock while doing blocking filesystem I/O.
        let namespaces = self.namespaces.read().await.clone();
        let existing = existing_namespace_files(&self.data_dir);
        for (namespace, values) in namespaces.iter() {
            save_namespace(&self.data_dir, namespace, values)?;
        }
        for (namespace, path) in existing {
            if !namespaces.contains_key(&namespace) {
                fs::remove_file(&path)
                    .map_err(|error| format!("removing {}: {error}", path.display()))?;
            }
        }
        Ok(())
    }
}

fn load_namespace(path: &Path) -> Result<HashMap<String, NbtCompound>, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let root = read_gzip_compound_tag(file).map_err(|error| error.to_string())?;
    let data = root.get_compound("data").unwrap_or(&root);
    let Some(contents) = data.get_compound("contents") else {
        return Ok(HashMap::new());
    };
    Ok(contents
        .child_tags
        .iter()
        .filter_map(|(key, value)| match value {
            pumpkin_nbt::tag::NbtTag::Compound(value) => Some((key.to_string(), value.clone())),
            _ => None,
        })
        .collect())
}

fn save_namespace(
    data_dir: &Path,
    namespace: &str,
    values: &HashMap<String, NbtCompound>,
) -> Result<(), String> {
    let mut contents = NbtCompound::new();
    for (path, value) in values {
        contents.put_compound(path, value.clone());
    }
    let mut data = NbtCompound::new();
    data.put_compound("contents", contents);
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", VANILLA_1_21_4_DATA_VERSION);
    root.put_compound("data", data);

    let final_path = data_dir.join(format!("{FILE_PREFIX}{namespace}{FILE_SUFFIX}"));
    let temporary_path = data_dir.join(format!("{FILE_PREFIX}{namespace}{FILE_SUFFIX}.tmp"));
    let file = File::create(&temporary_path)
        .map_err(|error| format!("creating {}: {error}", temporary_path.display()))?;
    write_gzip_compound_tag(root, file)
        .map_err(|error| format!("writing {}: {error}", temporary_path.display()))?;
    fs::copy(&temporary_path, &final_path)
        .map_err(|error| format!("replacing {}: {error}", final_path.display()))?;
    fs::remove_file(&temporary_path)
        .map_err(|error| format!("removing {}: {error}", temporary_path.display()))?;
    Ok(())
}

fn existing_namespace_files(data_dir: &Path) -> HashMap<String, PathBuf> {
    fs::read_dir(data_dir).map_or_else(
        |_| HashMap::new(),
        |entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let file_name = entry.file_name();
                    let file_name = file_name.to_str()?;
                    let namespace = file_name
                        .strip_prefix(FILE_PREFIX)?
                        .strip_suffix(FILE_SUFFIX)?;
                    Some((namespace.to_owned(), entry.path()))
                })
                .collect()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn persists_vanilla_namespace_files_and_removes_empty_values() {
        let temp = tempfile::tempdir().unwrap();
        let storage = CommandStorage::load(temp.path());
        let first = Identifier::parse("demo:first").unwrap();
        let second = Identifier::parse("demo:nested/second").unwrap();
        let mut first_value = NbtCompound::new();
        first_value.put_string("message", "hello".to_owned());
        let mut second_value = NbtCompound::new();
        second_value.put_int("count", 7);
        storage.set(&first, first_value.clone()).await;
        storage.set(&second, second_value.clone()).await;
        storage.save().await.unwrap();

        let saved_path = temp.path().join("data/command_storage_demo.dat");
        let saved = read_gzip_compound_tag(File::open(&saved_path).unwrap()).unwrap();
        assert_eq!(
            saved.get_int("DataVersion"),
            Some(VANILLA_1_21_4_DATA_VERSION)
        );
        let contents = saved
            .get_compound("data")
            .and_then(|data| data.get_compound("contents"))
            .expect("Vanilla SavedData wrapper must contain data.contents");
        assert_eq!(contents.get_compound("first"), Some(&first_value));
        assert_eq!(contents.get_compound("nested/second"), Some(&second_value));

        let reloaded = CommandStorage::load(temp.path());
        assert_eq!(reloaded.get(&first).await, first_value);
        assert_eq!(reloaded.get(&second).await, second_value);
        assert_eq!(reloaded.keys().await.len(), 2);

        reloaded.set(&first, NbtCompound::new()).await;
        reloaded.set(&second, NbtCompound::new()).await;
        reloaded.save().await.unwrap();
        assert!(!temp.path().join("data/command_storage_demo.dat").exists());
    }

    #[tokio::test]
    async fn loads_vanilla_data_wrapper_and_defaults_missing_keys_to_empty() {
        let temp = tempfile::tempdir().unwrap();
        let storage = CommandStorage::load(temp.path());
        let missing = Identifier::parse("minecraft:missing").unwrap();
        assert!(storage.get(&missing).await.is_empty());
    }
}
