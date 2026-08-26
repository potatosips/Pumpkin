use crate::entity::{
    Entity, EntityBaseFuture, NBTStorage, NbtFuture,
    mob::{Mob, MobEntity, skeleton::SkeletonEntityBase},
};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::version::JavaMinecraftVersion;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct BoggedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
    sheared: AtomicBool,
}

impl BoggedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let bogged = Self {
            entity,
            sheared: AtomicBool::new(false),
        };
        Arc::new(bogged)
    }

    pub fn is_sheared(&self) -> bool {
        self.sheared.load(Ordering::Relaxed)
    }

    pub fn set_sheared(&self, sheared: bool) {
        self.sheared.store(sheared, Ordering::Relaxed);
        self.get_mob_entity().living_entity.entity.send_meta_data(
            &[Metadata::new(tracked_data::bogged::SHEARED, sheared)],
            None,
        );
    }
}

impl NBTStorage for BoggedSkeletonEntity {
    fn write_nbt<'a>(&'a self, nbt: &'a mut NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.entity.mob_entity.write_nbt(nbt).await;
            nbt.put_bool("sheared", self.is_sheared());
        })
    }
    fn read_nbt_non_mut<'a>(&'a self, nbt: &'a NbtCompound) -> NbtFuture<'a, ()> {
        Box::pin(async move {
            self.entity
                .mob_entity
                .living_entity
                .read_nbt_non_mut(nbt)
                .await;
            if let Some(value) = nbt.get_bool("sheared") {
                self.sheared.store(value, Ordering::Relaxed);
            }
        })
    }
}

impl Mob for BoggedSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn get_bogged(&self) -> Option<&BoggedSkeletonEntity> {
        Some(self)
    }

    fn mob_java_spawn_metadata(
        &self,
        version: JavaMinecraftVersion,
    ) -> EntityBaseFuture<'_, Option<Box<[u8]>>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            Metadata::new(tracked_data::bogged::SHEARED, self.is_sheared())
                .write(&mut bytes, &version)
                .ok()?;
            bytes.push(255);
            Some(bytes.into_boxed_slice())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bogged_sheared_metadata_key() {
        assert_eq!(tracked_data::bogged::SHEARED.id.v1_21_4, 16);
    }
}
