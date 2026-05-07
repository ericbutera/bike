pub use sea_orm_migration::prelude::*;
mod m20260506_000001_create_activity_imports;
mod m20260506_000002_create_activities;
mod m20260506_000003_add_activity_derived_data;
mod m20260506_000004_create_segments;
mod m20260507_000005_create_user_preferences;
mod m20260507_000006_add_training_profile_fields;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = kaleido_migrations::external_migrations();
        migrations.sort_by_key(|m| m.name().to_string());

        let mut locals: Vec<Box<dyn MigrationTrait>> = Vec::new();
        locals.push(Box::new(
            m20260506_000001_create_activity_imports::Migration,
        ));
        locals.push(Box::new(m20260506_000002_create_activities::Migration));
        locals.push(Box::new(
            m20260506_000003_add_activity_derived_data::Migration,
        ));
        locals.push(Box::new(m20260506_000004_create_segments::Migration));
        locals.push(Box::new(
            m20260507_000005_create_user_preferences::Migration,
        ));
        locals.push(Box::new(
            m20260507_000006_add_training_profile_fields::Migration,
        ));
        locals.sort_by_key(|m| m.name().to_string());

        migrations.extend(locals);
        migrations
    }
}
